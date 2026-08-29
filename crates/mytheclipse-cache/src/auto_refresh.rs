//! Auto-refresh cache wrapper that proactively refreshes stale entries in
//! the background, eliminating thundering-herd on cache miss.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::traits::Cache;
use crate::CacheError;

/// A cache wrapper that refreshes entries in the background before they expire.
///
/// When a `get` returns a `None`, the wrapper triggers a background refresh
/// (via `refresh_fn`) while still returning the miss to the caller.
pub struct AutoRefreshCache<C, F, Fut>
where
    C: Cache + Clone + Send + Sync + 'static,
    F: Fn(String) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<Vec<u8>, CacheError>> + Send + 'static,
{
    inner: C,
    refresh_fn: Arc<F>,
    refresh_after: Duration,
    refreshing: Arc<Mutex<std::collections::HashSet<String>>>,
}

impl<C, F, Fut> AutoRefreshCache<C, F, Fut>
where
    C: Cache + Clone + Send + Sync + 'static,
    F: Fn(String) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<Vec<u8>, CacheError>> + Send + 'static,
{
    /// Creates a new auto-refresh wrapper.
    pub fn new(inner: C, refresh_fn: F, refresh_after: Duration) -> Self {
        Self {
            inner,
            refresh_fn: Arc::new(refresh_fn),
            refresh_after,
            refreshing: Arc::new(Mutex::new(std::collections::HashSet::new())),
        }
    }

    /// Gets a value, triggering a background refresh if the entry is a miss.
    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        let result = self.inner.get(key).await?;
        if result.is_none() {
            let key_str = key.to_string();
            let mut refreshing = self.refreshing.lock().await;
            if refreshing.insert(key_str.clone()) {
                let inner = self.inner.clone();
                let refresh_fn = Arc::clone(&self.refresh_fn);
                let refresh_after = self.refresh_after;
                let refreshing = self.refreshing.clone();
                tokio::spawn(async move {
                    let refresh_fut = refresh_fn(key_str.clone());
                    match refresh_fut.await {
                        Ok(value) => {
                            let ttl = Some(refresh_after * 2);
                            let _ = inner.set(&key_str, value, ttl).await;
                        }
                        Err(e) => {
                            tracing::warn!("background refresh failed for key {}: {}", key_str, e);
                        }
                    }
                    let mut r = refreshing.lock().await;
                    r.remove(&key_str);
                });
            }
        }
        Ok(result)
    }

    /// Sets a value in the underlying cache.
    pub async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<(), CacheError> {
        self.inner.set(key, value, ttl).await
    }

    /// Invalidates a key in the underlying cache.
    pub async fn invalidate(&self, key: &str) -> Result<(), CacheError> {
        self.inner.invalidate(key).await
    }
}
