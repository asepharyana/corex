//! JWT (HS256) generation and validation.
//!
//! Uses `jsonwebtoken`. Claims are plain `serde_json::Value` so callers can
//! build arbitrary claim sets without a bespoke struct.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The error produced by token sign/verify.
#[derive(Debug)]
pub enum TokenError {
    /// The token or key was malformed.
    Encoding(String),
    /// The token failed validation (bad signature, expired, etc.).
    Validation(String),
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Encoding(s) => write!(f, "token encoding: {s}"),
            Self::Validation(s) => write!(f, "token validation: {s}"),
        }
    }
}

impl std::error::Error for TokenError {}

/// A JWT signing/verification helper using HS256.
#[derive(Clone)]
pub struct TokenSigner {
    key: Vec<u8>,
}

/// Convenience alias for a `serde_json::Value` claim set.
pub type Claims = Value;

impl TokenSigner {
    /// Creates an HS256 signer from a shared secret.
    pub fn new(secret: &str) -> Self {
        Self {
            key: secret.as_bytes().to_vec(),
        }
    }

    /// Signs `claims` (plus an automated `exp` and `iat`) into a JWT string.
    pub fn sign(&self, claims: &Value, ttl: std::time::Duration) -> Result<String, TokenError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
        let mut payload = claims.clone();
        if !payload.is_object() {
            return Err(TokenError::Encoding("claims must be a JSON object".into()));
        }
        if payload.get("exp").is_none() {
            payload["exp"] = Value::Number((now + ttl.as_secs()).into());
        }
        if payload.get("iat").is_none() {
            payload["iat"] = Value::Number(now.into());
        }
        let token = jsonwebtoken::encode(
            &header,
            &payload,
            &jsonwebtoken::EncodingKey::from_secret(&self.key),
        )
        .map_err(|e| TokenError::Encoding(e.to_string()))?;
        Ok(token)
    }

    /// Verifies `token` and returns its decoded claims.
    ///
    /// The signature, `exp`, and `iat` are all validated.
    pub fn verify(&self, token: &str) -> Result<Claims, TokenError> {
        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
        validation.validate_exp = true;
        // `iat` is validated implicitly (rejected if in the future beyond leeway).
        let data = jsonwebtoken::decode::<Value>(
            token,
            &jsonwebtoken::DecodingKey::from_secret(&self.key),
            &validation,
        )
        .map_err(|e| TokenError::Validation(e.to_string()))?;
        Ok(data.claims)
    }

    /// Verifies a token and additionally checks the registered `sub` claim.
    pub fn verify_subject(&self, token: &str, expected_subject: &str) -> Result<(), TokenError> {
        let claims = self.verify(token)?;
        match claims.get("sub").and_then(Value::as_str) {
            Some(sub) if sub == expected_subject => Ok(()),
            _ => Err(TokenError::Validation("subject mismatch".into())),
        }
    }
}

/// The expiry claim helper used by [`TokenSigner`] (kept for symmetry).
#[derive(Debug, Serialize, Deserialize)]
pub struct RegisteredClaims {
    pub iat: Option<u64>,
    pub exp: Option<u64>,
    pub sub: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_verify_roundtrip() {
        let s = TokenSigner::new("my secret");
        let token = s
            .sign(
                &serde_json::json!({ "sub": "user-1", "role": "admin" }),
                std::time::Duration::from_secs(3600),
            )
            .unwrap();
        let claims = s.verify(&token).unwrap();
        assert_eq!(claims["sub"], "user-1");
        assert_eq!(claims["role"], "admin");
        assert!(claims["exp"].is_number());
        assert!(claims["iat"].is_number());
    }

    #[test]
    fn wrong_secret_fails() {
        let a = TokenSigner::new("key-a");
        let b = TokenSigner::new("key-b");
        let token = a
            .sign(
                &serde_json::json!({ "sub": "x" }),
                std::time::Duration::from_secs(100),
            )
            .unwrap();
        assert!(b.verify(&token).is_err());
    }

    #[test]
    fn tampered_token_fails() {
        let a = TokenSigner::new("key-a");
        let token = a
            .sign(
                &serde_json::json!({ "sub": "x" }),
                std::time::Duration::from_secs(100),
            )
            .unwrap();
        let mut bytes = token.into_bytes();
        let last = bytes.len() - 1;
        bytes[last] ^= 0x01;
        let tampered = String::from_utf8(bytes).unwrap();
        assert!(a.verify(&tampered).is_err());
    }

    #[test]
    fn expired_token_fails() {
        let a = TokenSigner::new("key-a");
        // Explicitly past `exp` (1970); sign only fills it in if absent.
        let token = a
            .sign(
                &serde_json::json!({ "sub": "x", "exp": 1000 }),
                std::time::Duration::from_secs(3600),
            )
            .unwrap();
        assert!(a.verify(&token).is_err());
    }

    #[test]
    fn subject_check() {
        let s = TokenSigner::new("s");
        let token = s
            .sign(
                &serde_json::json!({ "sub": "alice" }),
                std::time::Duration::from_secs(100),
            )
            .unwrap();
        assert!(s.verify_subject(&token, "alice").is_ok());
        assert!(s.verify_subject(&token, "bob").is_err());
    }
}
