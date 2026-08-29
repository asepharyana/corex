//! Typed key registry with ID-based lookup (feature `password`).
//!
//! [`TypedKeyRegistry`] extends `KeyRing` semantics: instead of a single
//! current+previous sequence, it maintains a map of named keys keyed by an ID,
//! with one designated "current" ID. This is useful when keys are rotated by ID
//! (e.g. JWT `kid` header) and you need to look up a verification key by ID
//! while only accepting tokens signed by the current key.

use std::collections::HashMap;

use crate::CryptoError;

/// A registry of named keys with a single "current" key.
#[derive(Debug, Clone, Default)]
pub struct TypedKeyRegistry<T> {
    keys: HashMap<String, T>,
    current_id: Option<String>,
}

impl<T> TypedKeyRegistry<T> {
    /// Creates an empty registry (no current key).
    pub fn new() -> Self {
        Self {
            keys: HashMap::new(),
            current_id: None,
        }
    }

    /// Registers a key under `id`, making it the current key.
    pub fn register(&mut self, id: impl Into<String>, key: T) {
        let id = id.into();
        self.keys.insert(id.clone(), key);
        self.current_id = Some(id);
    }

    /// Looks up a key by ID (current or previous).
    pub fn lookup(&self, id: &str) -> Option<&T> {
        self.keys.get(id)
    }

    /// Returns the current key, if any.
    pub fn current(&self) -> Option<&T> {
        self.current_id.as_ref().and_then(|id| self.keys.get(id))
    }

    /// Returns the ID of the current key.
    pub fn current_id(&self) -> Option<&str> {
        self.current_id.as_deref()
    }

    /// Rotates to a new current key identified by `id`. The old current key
    /// remains accessible via `lookup` but is no longer the active signing key.
    pub fn rotate_current(&mut self, id: impl Into<String>, key: T) {
        let id = id.into();
        self.keys.insert(id.clone(), key);
        self.current_id = Some(id);
    }

    /// Number of keys in the registry.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Whether the registry has any keys.
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Returns an error if no current key is registered.
    pub fn require_current(&self) -> Result<&T, CryptoError> {
        self.current()
            .ok_or_else(|| CryptoError::Key("no current key registered".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_lookup() {
        let mut reg = TypedKeyRegistry::new();
        reg.register("k1", [1u8; 32]);
        assert_eq!(reg.current_id(), Some("k1"));
        assert!(reg.lookup("k1").is_some());
        assert_eq!(reg.lookup("k1"), Some(&[1u8; 32]));
    }

    #[test]
    fn lookup_unknown_returns_none() {
        let reg = TypedKeyRegistry::<[u8; 32]>::new();
        assert!(reg.lookup("nope").is_none());
    }

    #[test]
    fn rotate_preserves_previous() {
        let mut reg = TypedKeyRegistry::new();
        reg.register("k1", [1u8; 32]);
        reg.rotate_current("k2", [2u8; 32]);
        assert_eq!(reg.current_id(), Some("k2"));
        assert!(reg.lookup("k1").is_some());
        assert_eq!(reg.lookup("k1"), Some(&[1u8; 32]));
    }

    #[test]
    fn require_current_errors_when_empty() {
        let reg = TypedKeyRegistry::<[u8; 32]>::new();
        assert!(matches!(reg.require_current(), Err(CryptoError::Key(_))));
    }

    #[test]
    fn len_and_is_empty() {
        let mut reg = TypedKeyRegistry::new();
        assert!(reg.is_empty());
        reg.register("a", 0u32);
        assert_eq!(reg.len(), 1);
        assert!(!reg.is_empty());
    }
}
