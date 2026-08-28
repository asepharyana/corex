//! Cache-aside with read-through (feature `cache-aside`).
//!
//! [`CacheAside`] wires a [`Cache`] to a data source: on a miss it invokes a
//! user-provided async fetcher, stores the result (with an optional TTL), and
//! returns it. This is the standard cache-aside pattern — reads bypass a cold
//! cache by falling back to the source of truth.

use std::time::Duration;

use crate::traits::{Cache, CacheError};

/// A generic read-through cache-aside helper.
///
/// `F` is the data source: an async closure `(owned key) -> Option<Vec<u8>>`.
/// The key is passed by value ([`String`]) so the returned future does not
/// borrow from the caller, which keeps the API simple and `'static`-friendly.
#[derive(Clone)]
pub struct CacheAside<C, F> {
    cache: C,
    fetcher: F,
    ttl: Option<Duration>,
}

impl<C, F, Fut> CacheAside<C, F>
where
    C: Cache,
    F: Fn(String) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Option<Vec<u8>>> + Send,
{
    /// Builds a cache-aside wrapper around `cache` using `fetcher` to fill
    /// misses. Entries are stored without expiry unless `with_ttl` is used.
    pub fn new(cache: C, fetcher: F) -> Self {
        Self {
            cache,
            fetcher,
            ttl: None,
        }
    }

    /// Applies a `ttl` to every entry written by this wrapper.
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// Returns a value for `key`, reading through to the fetcher on a miss and
    /// caching the result.
    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        if let Some(value) = self.cache.get(key).await? {
            return Ok(Some(value));
        }
        if let Some(value) = (self.fetcher)(key.to_string()).await {
            self.cache.set(key, value.clone(), self.ttl).await?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    /// Explicitly evicts `key`.
    pub async fn invalidate(&self, key: &str) -> Result<(), CacheError> {
        self.cache.invalidate(key).await
    }

    /// Returns a reference to the underlying cache.
    pub fn cache(&self) -> &C {
        &self.cache
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::MemoryCache;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    fn fetcher(
        hits: Arc<AtomicU64>,
    ) -> impl Fn(String) -> std::future::Ready<Option<Vec<u8>>> + Send + Sync {
        move |_key: String| {
            let n = hits.fetch_add(1, Ordering::SeqCst) + 1;
            std::future::ready(Some(format!("fetched-{n}").into_bytes()))
        }
    }

    #[tokio::test]
    async fn miss_reads_through_and_caches() {
        let hits = Arc::new(AtomicU64::new(0));
        let aside = CacheAside::new(MemoryCache::new(), fetcher(hits.clone()));

        let first = aside.get("k").await.unwrap().unwrap();
        let second = aside.get("k").await.unwrap().unwrap();
        assert_eq!(first, b"fetched-1");
        // Cache hit — fetcher not called again.
        assert_eq!(second, b"fetched-1");
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn invalidate_forces_refetch() {
        let hits = Arc::new(AtomicU64::new(0));
        let aside = CacheAside::new(MemoryCache::new(), fetcher(hits.clone()));
        let _ = aside.get("k").await.unwrap();
        aside.invalidate("k").await.unwrap();
        let again = aside.get("k").await.unwrap().unwrap();
        assert_eq!(again, b"fetched-2");
        assert_eq!(hits.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn ttl_applies_to_writes() {
        let aside = CacheAside::new(MemoryCache::new(), fetcher(Arc::new(AtomicU64::new(0))))
            .with_ttl(Duration::from_millis(30));
        let _ = aside.get("k").await.unwrap();
        assert_eq!(
            aside.cache().get("k").await.unwrap(),
            Some(b"fetched-1".to_vec())
        );
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert_eq!(aside.cache().get("k").await.unwrap(), None);
    }
}
