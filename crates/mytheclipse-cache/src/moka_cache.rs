//! A high-performance in-process cache backed by Moka (L1, `l1-moka`).
//!
//! Moka provides automatic max-capacity and (optionally) TTL-based eviction,
//! so this L1 is well-suited to workloads where memory bounds matter.

use std::time::Duration;

use async_trait::async_trait;
use moka::future::Cache as MokaCache;

use crate::traits::{Cache, CacheError};

/// A Moka-backed [`Cache`] for L1 caching.
#[derive(Clone)]
pub struct MokaL1 {
    inner: MokaCache<String, Vec<u8>>,
}

impl MokaL1 {
    /// Builds a Moka cache with `max_capacity` entries and an optional default
    /// `ttl`.
    pub fn new(max_capacity: u64, ttl: Option<Duration>) -> Self {
        let mut builder = MokaCache::builder().max_capacity(max_capacity);
        if let Some(ttl) = ttl {
            builder = builder.time_to_live(ttl);
        }
        Self {
            inner: builder.build(),
        }
    }
}

#[async_trait]
impl Cache for MokaL1 {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        Ok(self.inner.get(key).await)
    }

    async fn set(
        &self,
        key: &str,
        value: Vec<u8>,
        _ttl: Option<Duration>,
    ) -> Result<(), CacheError> {
        // Per-entry TTL overrides are handled by the builder default in Moka;
        // the passed `ttl` is intentionally ignored (single configured policy).
        self.inner.insert(key.to_string(), value).await;
        Ok(())
    }

    async fn invalidate(&self, key: &str) -> Result<(), CacheError> {
        self.inner.invalidate(key).await;
        Ok(())
    }

    async fn clear(&self) -> Result<(), CacheError> {
        self.inner.invalidate_all();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn set_get_roundtrip() {
        let c = MokaL1::new(100, None);
        c.set("k", b"v".to_vec(), None).await.unwrap();
        assert_eq!(c.get("k").await.unwrap(), Some(b"v".to_vec()));
        assert_eq!(c.get("missing").await.unwrap(), None);
    }

    #[tokio::test]
    async fn invalidate_and_clear() {
        let c = MokaL1::new(100, None);
        c.set("a", b"1".to_vec(), None).await.unwrap();
        c.set("b", b"2".to_vec(), None).await.unwrap();
        c.invalidate("a").await.unwrap();
        assert_eq!(c.get("a").await.unwrap(), None);
        assert_eq!(c.get("b").await.unwrap(), Some(b"2".to_vec()));
        c.clear().await.unwrap();
        assert_eq!(c.get("b").await.unwrap(), None);
    }

    #[tokio::test]
    async fn ttl_does_expire() {
        // Keep a firm TTL assertion; sleep well past the expiry window.
        let c = MokaL1::new(100, Some(Duration::from_millis(40)));
        c.set("k", b"v".to_vec(), None).await.unwrap();
        assert_eq!(c.get("k").await.unwrap(), Some(b"v".to_vec()));
        tokio::time::sleep(Duration::from_millis(120)).await;
        let v = c.get("k").await.unwrap();
        assert!(matches!(v, None));
    }
}
