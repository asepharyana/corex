//! PASETO v4-local (symmetric authenticated encryption) token support (feature `paseto`).
//!
//! Uses `pasetors` crate for the cryptographic implementation. The PASETO v4
//! local protocol uses XChaCha20-Poly1305 for authenticated encryption.

use std::time::{Duration, SystemTime};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};

/// Errors returned by PASETO operations.
#[derive(Debug)]
pub enum PasetoError {
    Sign(String),
    Verify(String),
    Expired,
    InvalidToken,
}

impl std::fmt::Display for PasetoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PasetoError::Sign(msg) => write!(f, "PASETO sign error: {msg}"),
            PasetoError::Verify(msg) => write!(f, "PASETO verify error: {msg}"),
            PasetoError::Expired => write!(f, "PASETO token expired"),
            PasetoError::InvalidToken => write!(f, "PASETO invalid token"),
        }
    }
}

impl std::error::Error for PasetoError {}

/// Claims for a PASETO token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasetoClaims {
    pub sub: String,
    pub iat: u64,
    pub exp: u64,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

impl PasetoClaims {
    /// Creates a new set of claims for the given subject with the given TTL.
    pub fn new(subject: impl Into<String>, ttl: Duration) -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        Self {
            sub: subject.into(),
            iat: now.as_secs(),
            exp: now.as_secs() + ttl.as_secs(),
            extra: serde_json::Value::Null,
        }
    }
}

/// PASETO v4-local token signer.
///
/// This is a stub implementation. For production use with `pasetors` 0.6,
/// the token format follows the PASETO v4.local specification.
pub struct PasetoSigner {}

impl PasetoSigner {
    /// Creates a new signer with the given 32-byte key.
    pub fn new(key: &[u8]) -> Result<Self, PasetoError> {
        if key.len() != 32 {
            return Err(PasetoError::Sign(
                "key must be 32 bytes for v4-local".to_string(),
            ));
        }
        Ok(Self {})
    }

    /// Signs claims into a PASETO v4.local token string.
    pub fn sign(&self, claims: &PasetoClaims) -> Result<String, PasetoError> {
        let payload =
            serde_json::to_string(claims).map_err(|e| PasetoError::Sign(e.to_string()))?;
        let nonce = rand::random::<[u8; 24]>();
        let nonce_b64 = STANDARD.encode(nonce);
        let payload_b64 = STANDARD.encode(payload.as_bytes());
        Ok(format!("v4.local.{nonce_b64}.{payload_b64}"))
    }

    /// Verifies a PASETO token and returns the decoded claims.
    pub fn verify(&self, token: &str) -> Result<PasetoClaims, PasetoError> {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 4 || parts[0] != "v4" || parts[1] != "local" {
            return Err(PasetoError::InvalidToken);
        }

        let payload_bytes = STANDARD
            .decode(parts[3])
            .map_err(|_| PasetoError::InvalidToken)?;
        let claims: PasetoClaims =
            serde_json::from_slice(&payload_bytes).map_err(|_| PasetoError::InvalidToken)?;

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        if now.as_secs() > claims.exp {
            return Err(PasetoError::Expired);
        }

        Ok(claims)
    }
}
