//! The core [`Cache`] and [`KeyEncoder`] traits.

use std::borrow::Cow;
use std::time::Duration;

use async_trait::async_trait;

/// Errors returned by cache operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheError {
    /// The backend could not be reached (e.g. Redis connection lost).
    Io(String),
    /// A value could not be serialized / deserialized.
    Serialization(String),
    /// A key could not be encoded for the backend.
    Key(String),
}

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(s) => write!(f, "cache io: {s}"),
            Self::Serialization(s) => write!(f, "cache serialization: {s}"),
            Self::Key(s) => write!(f, "cache key: {s}"),
        }
    }
}

impl std::error::Error for CacheError {}

/// A generic byte-oriented cache.
///
/// Real caches operate on bytes or strings; typed convenience is layered on
/// top (see [`crate::memory::typed::TypedCache`], behind `cache-aside`).
/// Implementors control the value format.
#[async_trait]
pub trait Cache: Send + Sync {
    /// Fetches a value by key. `None` indicates a miss.
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError>;
    /// Stores a value under `key`, optionally expiring after `ttl`.
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>)
        -> Result<(), CacheError>;
    /// Removes a key.
    async fn invalidate(&self, key: &str) -> Result<(), CacheError>;
    /// Removes all entries.
    async fn clear(&self) -> Result<(), CacheError>;
}

/// Keys given to the byte-oriented [`Cache`] are `&str`, but concrete backends
/// may need richer keys. [`KeyEncoder`] turns typed keys into canonical strings.
pub trait KeyEncoder {
    /// The "shape" of a key, e.g. `"user:{id}:profile"`.
    fn encode<C: Into<Cow<'static, str>>, R: std::fmt::Display>(parts: (C, R)) -> String;
}

/// A blanket implementation that formats `{collection}:{id}`.
pub struct DefaultKeyEncoder;

impl KeyEncoder for DefaultKeyEncoder {
    fn encode<C: Into<Cow<'static, str>>, R: std::fmt::Display>(parts: (C, R)) -> String {
        format!("{}:{}", parts.0.into(), parts.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_key_encoder_formats() {
        assert_eq!(DefaultKeyEncoder::encode(("user", 42)), "user:42");
        assert_eq!(
            DefaultKeyEncoder::encode(("session", "abc-123")),
            "session:abc-123"
        );
    }
}
