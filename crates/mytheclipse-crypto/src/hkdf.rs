//! HKDF-SHA256 key derivation (feature `derivation`).
//!
//! [`HkdfKeyDeriver`] wraps the HKDF construction (RFC 5869) to derive
//! domain-specific sub-keys from a single master secret. Each purpose
//! string acts as the `info` parameter for domain separation.

use sha2::Sha256;
use hkdf::Hkdf;

/// Derives sub-keys from a master secret using HKDF-SHA256.
pub struct HkdfKeyDeriver {
    hk: Hkdf<Sha256>,
}

impl HkdfKeyDeriver {
    /// Creates a deriver from the given master secret (IKM).
    pub fn new(master: &[u8]) -> Self {
        let hk = Hkdf::<Sha256>::new(None, master);
        Self { hk }
    }

    /// Derives a sub-key for the given `purpose` (used as the `info` parameter).
    ///
    /// Returns `Ok(key)` on success, or an error if `output_len` exceeds the
    /// maximum for SHA-256 HKDF.
    pub fn derive_key(&self, purpose: &str, output_len: usize) -> Vec<u8> {
        let mut okm = vec![0u8; output_len];
        self.hk
            .expand(purpose.as_bytes(), &mut okm)
            .expect("HKDF expand failed — output_len too large");
        okm
    }

    /// Convenience: derive a 32-byte AES-256 key for `purpose`.
    pub fn derive_aes256_key(&self, purpose: &str) -> [u8; 32] {
        let v = self.derive_key(purpose, 32);
        let mut key = [0u8; 32];
        key.copy_from_slice(&v);
        key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_key_is_deterministic() {
        let deriver = HkdfKeyDeriver::new(b"master-secret");
        let k1 = deriver.derive_key("encryption", 32);
        let k2 = deriver.derive_key("encryption", 32);
        assert_eq!(k1, k2);
        assert_eq!(k1.len(), 32);
    }

    #[test]
    fn derive_key_different_purposes_yield_different_keys() {
        let deriver = HkdfKeyDeriver::new(b"master-secret");
        let enc = deriver.derive_key("encryption", 32);
        let auth = deriver.derive_key("auth", 32);
        assert_ne!(enc, auth);
    }

    #[test]
    fn derive_aes256_key_length() {
        let deriver = HkdfKeyDeriver::new(b"master-secret");
        let key = deriver.derive_aes256_key("signing");
        assert_eq!(key.len(), 32);
    }
}
