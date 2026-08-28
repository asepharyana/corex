//! # mytheclipse-crypto
//!
//! Low-level security helpers that are easy to get wrong when hand-rolled:
//!
//! - **Argon2id** password hashing & verification (`password` feature), with
//!   RFC 9106-ish parameters.
//! - **AES-256-GCM** authenticated encryption with a fresh random nonce per
//!   operation (`encryption` feature).
//! - **JWT** (HS256) / **Paseto** v4 local token generation & validation (`tokens`
//!   feature).
//! - **Key rotation** via [`KeyRing`]: decryption/verification tries the current
//!   key then a list of previous keys.
//!
//! Nothing in the crate owns long-lived key material; keys are passed in as
//! bytes/keys by the caller and the caller is responsible for storage. Each
//! primitive is small enough to reason about in one screen.
//!
//! ## Example
//!
//! ```no_run
//! use mytheclipse_crypto::{PasswordHasher, Encryptor, TokenSigner};
//!
//! // Hash & verify a password.
//! let hasher = PasswordHasher::new();
//! let hash = hasher.hash("hunter2").unwrap();
//! assert!(hasher.verify(&hash, "hunter2"));
//!
//! // Encrypt & decrypt a blob.
//! let key = [0u8; 32];
//! let enc = Encryptor::new(&key);
//! let (nonce, ct) = enc.encrypt(b"secret message");
//! let plain = enc.decrypt(&nonce, &ct).unwrap();
//! assert_eq!(plain, b"secret message");
//!
//! // Sign & verify a JWT.
//! let signer = TokenSigner::new("super-secret-key");
//! let token = signer
//!     .sign(&serde_json::json!({ "sub": "u1" }), std::time::Duration::from_secs(3600))
//!     .unwrap();
//! let claims = signer.verify(&token).unwrap();
//! assert_eq!(claims["sub"], "u1");
//! ```

pub mod key_ring;

#[cfg(feature = "password")]
pub mod password;

#[cfg(feature = "encryption")]
pub mod encryption;

#[cfg(feature = "tokens")]
pub mod token;

#[cfg(feature = "password")]
pub use password::PasswordHasher;

#[cfg(feature = "encryption")]
pub use encryption::{AeadError, Encryptor};

#[cfg(feature = "tokens")]
pub use token::{Claims, TokenError, TokenSigner};

pub use key_ring::KeyRing;

/// Errors returned across mytheclipse-crypto primitives.
#[non_exhaustive]
#[derive(Debug)]
pub enum CryptoError {
    /// Password hashing or verification failed.
    Password(String),
    /// Authenticated encryption / decryption failed.
    Encryption(String),
    /// Token generation or validation failed.
    Token(String),
    /// Key material is invalid for the requested operation.
    Key(String),
}

impl std::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Password(msg) => write!(f, "password error: {msg}"),
            Self::Encryption(msg) => write!(f, "encryption error: {msg}"),
            Self::Token(msg) => write!(f, "token error: {msg}"),
            Self::Key(msg) => write!(f, "key error: {msg}"),
        }
    }
}

impl std::error::Error for CryptoError {}

/// The fixed AES-256-GCM nonce length (96 bits) in bytes.
pub const NONCE_LEN: usize = 12;
/// The fixed AES-256-GCM key length in bytes.
pub const KEY_LEN: usize = 32;
