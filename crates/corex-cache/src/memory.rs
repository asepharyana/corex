//! A simple, dependency-free in-process cache (L1, `l1-memory`).
//!
//! Backed by a `HashMap<String, (Vec<u8>, Instant)>` guarded by a `Mutex`.
//! Entries are lazily expired on access by comparing against `Instant`; a
//! monotonic clock keeps TTLs robust against wall-clock discontinuities.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;

use crate::traits::{Cache, CacheError};

/// A wrapping entry: `None` expiry means the value never expires.
type Entry = (Vec<u8>, Option<Instant>);

/// An in-process [`Cache`] implementation for L1 caching.
#[derive(Clone, Default)]
pub struct MemoryCache {
    inner: Arc<Mutex<HashMap<String, Entry>>>,
}

impl MemoryCache {
    /// Builds an empty in-memory cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Pre-allocates space for `capacity` entries to reduce reallocation.
    pub fn with_capacity(self, capacity: usize) -> Self {
        self.inner.lock().unwrap().reserve(capacity);
        self
    }
}

#[async_trait]
impl Cache for MemoryCache {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        let mut map = self.inner.lock().unwrap();
        match map.get(key) {
            Some((value, Some(expires))) if *expires <= Instant::now() => {
                map.remove(key);
                Ok(None)
            }
            Some((value, _)) => Ok(Some(value.clone())),
            None => Ok(None),
        }
    }

    async fn set(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> Result<(), CacheError> {
        let expires = ttl.map(|d| Instant::now() + d);
        self.inner
            .lock()
            .unwrap()
            .insert(key.to_string(), (value, expires));
        Ok(())
    }

    async fn invalidate(&self, key: &str) -> Result<(), CacheError> {
        self.inner.lock().unwrap().remove(key);
        Ok(())
    }

    async fn clear(&self) -> Result<(), CacheError> {
        self.inner.lock().unwrap().clear();
        Ok(())
    }
}

/// A typed view over a byte cache using `serde`-compatible (JSON) encoding.
///
/// Only enabled with the `cache-aside` feature, which pulls in `serde`.
#[cfg(feature = "cache-aside")]
pub mod typed {
    use serde::{de::DeserializeOwned, Serialize};

    use super::*;

    /// Wraps a [`Cache`] with JSON-based typed get/set.
    #[derive(Clone)]
    pub struct TypedCache<C> {
        inner: C,
    }

    impl<C: Cache> TypedCache<C> {
        /// Wraps `inner`.
        pub fn new(inner: C) -> Self {
            Self { inner }
        }

        /// Fetches and deserializes a value.
        pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, CacheError> {
            match self.inner.get(key).await? {
                Some(bytes) => serde_json::from_slice(&bytes)
                    .map(Some)
                    .map_err(|e| CacheError::Serialization(e.to_string())),
                None => Ok(None),
            }
        }

        /// Serializes and stores a value.
        pub async fn set<T: Serialize>(
            &self,
            key: &str,
            value: &T,
            ttl: Option<Duration>,
        ) -> Result<(), CacheError> {
            let bytes =
                serde_json::to_vec(value).map_err(|e| CacheError::Serialization(e.to_string()))?;
            self.inner.set(key, bytes, ttl).await
        }

        /// Returns the underlying byte cache.
        pub fn into_inner(self) -> C {
            self.inner
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn set_get_roundtrip() {
        let c = MemoryCache::new();
        c.set("k", b"v".to_vec(), None).await.unwrap();
        assert_eq!(c.get("k").await.unwrap(), Some(b"v".to_vec()));
        assert_eq!(c.get("missing").await.unwrap(), None);
    }

    #[tokio::test]
    async fn ttl_expires_entry() {
        let c = MemoryCache::new();
        c.set("k", b"v".to_vec(), Some(Duration::from_millis(30)))
            .await
            .unwrap();
        assert_eq!(c.get("k").await.unwrap(), Some(b"v".to_vec()));
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert_eq!(c.get("k").await.unwrap(), None);
    }

    #[tokio::test]
    async fn invalidate_and_clear() {
        let c = MemoryCache::new();
        c.set("a", b"1".to_vec(), None).await.unwrap();
        c.set("b", b"2".to_vec(), None).await.unwrap();
        c.invalidate("a").await.unwrap();
        assert_eq!(c.get("a").await.unwrap(), None);
        assert_eq!(c.get("b").await.unwrap(), Some(b"2".to_vec()));
        c.clear().await.unwrap();
        assert_eq!(c.get("b").await.unwrap(), None);
    }

    #[cfg(feature = "cache-aside")]
    #[tokio::test]
    async fn typed_cache_roundtrip() {
        use typed::TypedCache;
        #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
        struct User {
            id: u64,
            name: String,
        }
        let typed = TypedCache::new(MemoryCache::new());
        typed
            .set(
                "u",
                &User {
                    id: 1,
                    name: "alice".into(),
                },
                None,
            )
            .await
            .unwrap();
        let got: User = typed.get("u").await.unwrap().unwrap();
        assert_eq!(
            got,
            User {
                id: 1,
                name: "alice".into()
            }
        );
    }
}
