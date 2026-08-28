//! Argon2id password hashing (RFC 9106).
//!
//! Hashes are stored as PHC strings, so they carry their parameters inline and
//! can be verified even if the recommended parameters change over time.

// Traits imported with `_` names so their methods resolve without colliding
// with the `PasswordHasher` struct defined below.
use argon2::password_hash::{PasswordHash, PasswordHasher as _, PasswordVerifier as _, SaltString};
use argon2::Argon2;

use crate::CryptoError;

/// Default memory cost (KiB) — 64 MiB.
const DEFAULT_MEM: u32 = 64 * 1024;
/// Default time cost.
const DEFAULT_TIME: u32 = 3;
/// Default parallelism (lanes).
const DEFAULT_PAR: u32 = 1;

/// An Argon2id password hasher with configurable parameters.
#[derive(Clone)]
pub struct PasswordHasher {
    mem: u32,
    time: u32,
    parallelism: u32,
}

impl Default for PasswordHasher {
    fn default() -> Self {
        Self {
            mem: DEFAULT_MEM,
            time: DEFAULT_TIME,
            parallelism: DEFAULT_PAR,
        }
    }
}

impl PasswordHasher {
    /// Builds a hasher with RFC-9106-style defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a hasher with custom Argon2 parameters.
    pub fn with_params(mem: u32, time: u32, parallelism: u32) -> Self {
        Self {
            mem,
            time,
            parallelism,
        }
    }

    /// The configured memory cost in KiB.
    pub fn memory_cost(&self) -> u32 {
        self.mem
    }

    /// Hashes `password` using Argon2id with a fresh random salt, returning a
    /// PHC-encoded string (`$argon2id$v=19$m=...,t=...,p=...$salt$hash`).
    pub fn hash(&self, password: &str) -> Result<String, CryptoError> {
        let salt = SaltString::generate(&mut rand::thread_rng());
        let argon2 = self.argon2();
        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| CryptoError::Password(e.to_string()))?;
        Ok(hash.to_string())
    }

    /// Verifies `password` against a previously computed PHC `hash`.
    ///
    /// Uses the parameters encoded in the hash (not our current defaults), so
    /// older hashes with different parameters still verify. Returns `false`
    /// on a mismatch or malformed hash.
    pub fn verify(&self, hash: &str, password: &str) -> bool {
        let parsed = match PasswordHash::new(hash) {
            Ok(p) => p,
            Err(_) => return false,
        };
        // Reconstruct Argon2 parameters from the PHC-serialized params string.
        let (mem, time, lanes) = match parse_params(parsed.params.as_str()) {
            Some(v) => v,
            None => (DEFAULT_MEM, DEFAULT_TIME, DEFAULT_PAR),
        };
        let params = match argon2::Params::new(mem, time, lanes, None) {
            Ok(p) => p,
            Err(_) => return false,
        };
        let version = parsed
            .version
            .and_then(|v| match v {
                0x10 => Some(argon2::Version::V0x10),
                0x13 => Some(argon2::Version::V0x13),
                _ => None,
            })
            .unwrap_or(argon2::Version::V0x13);
        let algorithm = match &*parsed.algorithm {
            "argon2d" => argon2::Algorithm::Argon2d,
            "argon2i" => argon2::Algorithm::Argon2i,
            _ => argon2::Algorithm::Argon2id,
        };
        let verifier = Argon2::new(algorithm, version, params);
        verifier
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    }

    fn argon2(&self) -> Argon2<'static> {
        let params = argon2::Params::new(self.mem, self.time, self.parallelism, None)
            .expect("valid argon2 parameters");
        Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params)
    }
}

/// Parses `m=65536,t=3,p=1` style params out of a PHC params string.
fn parse_params(params: &str) -> Option<(u32, u32, u32)> {
    let mut m = None;
    let mut t = None;
    let mut p = None;
    for part in params.split(',') {
        let (k, v) = part.split_once('=')?;
        let value = v.parse::<u32>().ok()?;
        match k {
            "m" => m = Some(value),
            "t" => t = Some(value),
            "p" => p = Some(value),
            _ => {}
        }
    }
    Some((m?, t?, p?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify_roundtrip() {
        let h = PasswordHasher::new();
        let encoded = h.hash("correct horse battery staple").unwrap();
        assert!(h.verify(&encoded, "correct horse battery staple"));
        assert!(!h.verify(&encoded, "wrong password"));
    }

    #[test]
    fn different_salts_yield_different_hashes() {
        let h = PasswordHasher::new();
        let a = h.hash("samepassword").unwrap();
        let b = h.hash("samepassword").unwrap();
        assert_ne!(a, b);
        assert!(h.verify(&a, "samepassword"));
        assert!(h.verify(&b, "samepassword"));
    }

    #[test]
    fn invalid_hash_verifies_false() {
        let h = PasswordHasher::new();
        assert!(!h.verify("not-a-real-hash", "anything"));
    }

    #[test]
    fn hash_string_is_phc_formatted() {
        let h = PasswordHasher::new();
        let encoded = h.hash("x").unwrap();
        assert!(encoded.starts_with("$argon2id$v=19$m=65536,t=3,p=1$"));
    }
}
