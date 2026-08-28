//! AES-256-GCM authenticated encryption with a random nonce per operation.
//!
//! The `nonce` is generated fresh for every [`Encryptor::encrypt`] call and
//! returned alongside the ciphertext so the caller can store/transmit it. The
//! caller must keep the plaintext length out of scope of concern; GCM provides
//! confidentiality and integrity.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};

use crate::{KEY_LEN, NONCE_LEN};

/// A thin wrapper around AES-256-GCM providing a safe one-line encrypt/decrypt.
pub struct Encryptor {
    cipher: Aes256Gcm,
}

/// The failure mode of an AEAD operation.
#[derive(Debug, PartialEq, Eq)]
pub enum AeadError {
    /// Decryption failed because the authentication tag did not match.
    AuthenticationFailed,
    /// Key or nonce material had an invalid length/format.
    InvalidInput,
}

impl std::fmt::Display for AeadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AuthenticationFailed => write!(f, "authentication failed"),
            Self::InvalidInput => write!(f, "invalid input"),
        }
    }
}

impl std::error::Error for AeadError {}

impl Encryptor {
    /// Builds a GCM encryptor from a 32-byte key.
    ///
    /// # Panics
    ///
    /// Panics if `key` is not exactly `KEY_LEN` (32) bytes.
    pub fn new(key: &[u8]) -> Self {
        assert_eq!(key.len(), KEY_LEN, "AES-256-GCM requires a 32-byte key");
        let mut kb = [0u8; KEY_LEN];
        kb.copy_from_slice(key);
        Self {
            cipher: Aes256Gcm::new((&kb).into()),
        }
    }

    /// Encrypts `plaintext`, returning `(nonce, ciphertext_with_tag)`.
    ///
    /// The nonce is 12 random bytes, unique per call.
    pub fn encrypt(&self, plaintext: &[u8]) -> (Vec<u8>, Vec<u8>) {
        let nonce_bytes = Self::random_nonce();
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ct = self
            .cipher
            .encrypt(nonce, plaintext)
            .expect("AES-256-GCM encryption is infallible for valid input");
        (nonce_bytes.to_vec(), ct)
    }

    /// Decrypts `nonce || ciphertext` produced by [`Encryptor::encrypt`].
    pub fn decrypt(&self, nonce: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, AeadError> {
        if nonce.len() != NONCE_LEN {
            return Err(AeadError::InvalidInput);
        }
        let nonce = Nonce::from_slice(nonce);
        self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| AeadError::AuthenticationFailed)
    }

    /// Deterministically encrypts with a caller-supplied nonce (for tests or
    /// for deriving per-record nonces from a counter). Prefer [`encrypt`].
    ///
    /// [`encrypt`]: Encryptor::encrypt
    pub fn encrypt_with_nonce(
        &self,
        nonce: &[u8; NONCE_LEN],
        plaintext: &[u8],
    ) -> Result<Vec<u8>, AeadError> {
        self.cipher
            .encrypt(Nonce::from_slice(nonce), plaintext)
            .map_err(|_| AeadError::InvalidInput)
    }

    fn random_nonce() -> [u8; NONCE_LEN] {
        let mut n = [0u8; NONCE_LEN];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut n);
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> [u8; KEY_LEN] {
        [0x42u8; KEY_LEN]
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let e = Encryptor::new(&key());
        let (nonce, ct) = e.encrypt(b"classified briefcase");
        let plain = e.decrypt(&nonce, &ct).unwrap();
        assert_eq!(plain, b"classified briefcase");
    }

    #[test]
    fn tampered_ciphertext_fails_auth() {
        let e = Encryptor::new(&key());
        let (nonce, mut ct) = e.encrypt(b"tamper me");
        ct[0] ^= 0xff;
        assert_eq!(e.decrypt(&nonce, &ct), Err(AeadError::AuthenticationFailed));
    }

    #[test]
    fn wrong_key_fails_auth() {
        let e = Encryptor::new(&key());
        let (nonce, ct) = e.encrypt(b"hi");
        let wrong = Encryptor::new(&[0x99u8; KEY_LEN]);
        assert_eq!(
            wrong.decrypt(&nonce, &ct),
            Err(AeadError::AuthenticationFailed)
        );
    }

    #[test]
    fn nonce_is_random_per_call() {
        let e = Encryptor::new(&key());
        let (n1, _) = e.encrypt(b"data");
        let (n2, _) = e.encrypt(b"data");
        assert_ne!(n1, n2);
    }

    #[test]
    fn wrong_key_length_panics() {
        let result = std::panic::catch_unwind(|| Encryptor::new(&[0u8; 16]));
        assert!(result.is_err());
    }
}
