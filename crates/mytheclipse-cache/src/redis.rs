//! A distributed (L2) cache backed by Redis / Valkey (feature `l2-redis`).
//!
//! Wraps a `redis` async connection (multiplexed). Values are stored as raw
//! Redis strings with an optional TTL (`SETEX` when a TTL is given). The
//! caller provides the connection; this type only issues cache commands.

use std::time::Duration;

use async_trait::async_trait;
use redis::aio::MultiplexedConnection;
use redis::{AsyncCommands, RedisError};

use crate::traits::{Cache, CacheError};

/// Map a Redis error onto a [`CacheError`].
fn map_err(e: RedisError) -> CacheError {
    CacheError::Io(e.to_string())
}

/// An L2 cache backed by a `redis` [`MultiplexedConnection`].
///
/// The connection is supplied by the caller; it is cheaply cloned (the
/// multiplexed connection is `Arc`-backed internally), so one pool can drive
/// both cache operations and other Redis usage.
#[derive(Clone)]
pub struct RedisCache {
    conn: MultiplexedConnection,
    /// Optional namespace prefix prepended to every key.
    prefix: String,
}

impl RedisCache {
    /// Wraps an existing connection.
    pub fn new(conn: MultiplexedConnection) -> Self {
        Self::with_prefix(conn, String::new())
    }

    /// Wraps a connection and adds a namespace prefix to every key.
    pub fn with_prefix(conn: MultiplexedConnection, prefix: String) -> Self {
        Self { conn, prefix }
    }

    fn key(&self, key: &str) -> String {
        if self.prefix.is_empty() {
            key.to_string()
        } else {
            format!("{}{}", self.prefix, key)
        }
    }
}

#[async_trait]
impl Cache for RedisCache {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        // `get::<_, Option<Vec<u8>>>` returns `None` for a missing key.
        let mut c = self.conn.clone();
        let k = self.key(key);
        let result: Result<Option<Vec<u8>>, RedisError> = c.get(&k).await;
        result.map_err(map_err)
    }

    async fn set(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> Result<(), CacheError> {
        let mut c = self.conn.clone();
        let k = self.key(key);
        match ttl {
            Some(ttl) => {
                let secs = ttl.as_secs().max(1);
                let result: Result<(), RedisError> = c.set_ex(&k, value, secs).await;
                result.map_err(map_err)
            }
            None => {
                let result: Result<(), RedisError> = c.set(&k, value).await;
                result.map_err(map_err)
            }
        }
    }

    async fn invalidate(&self, key: &str) -> Result<(), CacheError> {
        let mut c = self.conn.clone();
        let k = self.key(key);
        let result: Result<u64, RedisError> = c.del(&k).await;
        result.map(|_| ()).map_err(map_err)
    }

    async fn clear(&self) -> Result<(), CacheError> {
        // Deliberately does nothing: `FLUSHALL`/`FLUSHDB` are dangerous on a
        // shared instance. Consumers should scope keys under a prefix and call
        // `invalidate` for the keys they own.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Integration test requiring a live Redis at `REDIS_URL`
    /// (e.g. `redis://127.0.0.1:6379`). Run with:
    /// `REDIS_URL=redis://127.0.0.1:6379 cargo test -p mytheclipse-cache --features l2-redis -- --ignored` .
    #[tokio::test]
    #[ignore = "requires a live Redis instance (REDIS_URL)"]
    async fn set_get_roundtrip_live() {
        let url = std::env::var("REDIS_URL").expect("set REDIS_URL");
        let client = redis::Client::open(url).expect("valid redis url");
        let conn = client
            .get_multiplexed_tokio_connection()
            .await
            .expect("connect");
        let cache = RedisCache::with_prefix(conn, "mytheclipse_cache_test:".to_string());

        cache
            .set("k", b"v".to_vec(), Some(Duration::from_secs(3600)))
            .await
            .unwrap();
        assert_eq!(cache.get("k").await.unwrap(), Some(b"v".to_vec()));
        cache.invalidate("k").await.unwrap();
        assert_eq!(cache.get("k").await.unwrap(), None);
    }
}
