//! Key rotation support.
//!
//! [`KeyRing`] holds a "current" key plus a list of "previous" keys. Operations
//! that need to *decrypt* or *verify* (as opposed to sign/encrypt) try the
//! current key first and then fall back to the previous keys, which is exactly
//! what you want during a rotation window: new writes use the current key, old
//! data can still be read with the previous key.

/// A generic ring of keys: one current plus any number of previous.
///
/// `T` is typically `&[u8]` or a key handle. The type is `Clone`; rotation just
/// swaps the current key into the previous list.
#[derive(Debug, Clone)]
pub struct KeyRing<T> {
    current: T,
    previous: Vec<T>,
}

impl<T> KeyRing<T> {
    /// Builds a ring with a single current key.
    pub fn new(current: T) -> Self {
        Self {
            current,
            previous: Vec::new(),
        }
    }

    /// Returns the current key.
    pub fn current(&self) -> &T {
        &self.current
    }

    /// Returns all keys, current first, then previous oldest-first-in-insert
    /// order.
    pub fn all_keys(&self) -> impl Iterator<Item = &T> {
        std::iter::once(&self.current).chain(self.previous.iter())
    }

    /// Rotates in a new key, demoting the old current key to `previous`.
    ///
    /// Usually call this with the *new* key as `new_key`; the old current key
    /// remains usable for reads during the rotation window.
    pub fn rotate(&mut self, new_key: T) {
        let old = std::mem::replace(&mut self.current, new_key);
        self.previous.push(old);
    }

    /// The number of keys being tracked (current + previous).
    pub fn size(&self) -> usize {
        1 + self.previous.len()
    }
}

impl<T> KeyRing<T>
where
    T: PartialEq,
{
    /// Whether `key` is currently in the ring (current or previous).
    pub fn contains(&self, key: &T) -> bool {
        self.all_keys().any(|k| k == key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotate_preserves_previous() {
        let mut ring = KeyRing::new([1u8; 32]);
        assert_eq!(ring.current(), &[1u8; 32]);
        assert_eq!(ring.size(), 1);

        ring.rotate([2u8; 32]);
        assert_eq!(ring.current(), &[2u8; 32]);
        assert_eq!(ring.size(), 2);
        assert!(ring.contains(&[1u8; 32]));
        assert!(ring.contains(&[2u8; 32]));

        ring.rotate([3u8; 32]);
        assert_eq!(ring.size(), 3);
        assert!(ring.contains(&[1u8; 32]));
        assert!(ring.contains(&[2u8; 32]));
        assert!(ring.contains(&[3u8; 32]));
    }

    #[test]
    fn all_keys_iterates_current_first() {
        let mut ring = KeyRing::new("current");
        ring.rotate("prev1");
        ring.rotate("prev2");
        // `previous` is push-ordered, so iteration is current, then newest-old.
        let keys: Vec<&str> = ring.all_keys().copied().collect();
        assert_eq!(keys, vec!["prev2", "current", "prev1"]);
    }
}
