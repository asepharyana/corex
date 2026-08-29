//! Config validation traits and built-in validators (feature `validation`).
//!
//! [`ConfigValidator`] lets application config types sanity-check themselves
//! after deserialization — e.g. ensuring a database URL parses, a port is in
//! range, or a required field is non-empty — and collect all failures into a
//! single report rather than failing one field at a time.

use std::fmt;

use crate::ConfigError;

/// A single validation failure with a human-readable path and message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationFailure {
    /// Dotted path to the offending field, e.g. `"database.url"`.
    pub path: String,
    /// What was wrong.
    pub message: String,
}

impl fmt::Display for ValidationFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

/// Errors produced by [`ConfigValidator::validate`].
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// All failures found in a single validation pass.
    pub failures: Vec<ValidationFailure>,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "config validation failed ({} issue(s)):",
            self.failures.len()
        )?;
        for failure in &self.failures {
            write!(f, "\n  - {failure}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ValidationError {}

impl From<ValidationError> for ConfigError {
    fn from(err: ValidationError) -> Self {
        ConfigError::Validation(err.to_string())
    }
}

/// Trait for types that can validate themselves after configuration loading.
///
/// Implementors collect field-level failures rather than returning on the
/// first error, so operators see the full problem set in one pass.
pub trait ConfigValidator {
    fn validate(&self) -> Result<(), ValidationError>;
}

/// Convenience blanket for any serializable config type that implements
/// [`ConfigValidator`]. Callers typically invoke this on the output of
/// [`ConfigLoader::build`](crate::loader::ConfigLoader::build).
///
/// ```no_run
/// # use mytheclipse_config::{ConfigLoader, ConfigValidator, ConfigValidatorExt};
/// # use serde::Deserialize;
/// # #[derive(Debug, Deserialize)]
/// # struct Cfg { port: u16 }
/// # impl ConfigValidator for Cfg {
/// #     fn validate(&self) -> Result<(), mytheclipse_config::ValidationError> { Ok(()) }
/// # }
/// let cfg: Cfg = ConfigLoader::new().build().unwrap();
/// cfg.validate_config().unwrap();
/// ```
pub trait ConfigValidatorExt: ConfigValidator {
    /// Validates `self`, returning `Ok(())` on success.
    fn validate_config(&self) -> Result<(), ConfigError> {
        self.validate().map_err(ConfigError::from)
    }
}

impl<T: ConfigValidator> ConfigValidatorExt for T {}

/// Validates that a string is a well-formed URL (http/https).
pub fn validate_url(path: &str, value: &str) -> Option<ValidationFailure> {
    if value.is_empty() {
        return Some(ValidationFailure {
            path: path.to_string(),
            message: "url must not be empty".into(),
        });
    }
    // Minimal heuristic: scheme + host. We avoid pulling in a full URL crate
    // to keep the dependency surface small.
    let scheme_len = if value.starts_with("http://") {
        7
    } else if value.starts_with("https://") {
        8
    } else {
        return Some(ValidationFailure {
            path: path.to_string(),
            message: format!("url must start with http:// or https:// (got {value:?})"),
        });
    };
    let host = &value[scheme_len..];
    if host.is_empty() {
        return Some(ValidationFailure {
            path: path.to_string(),
            message: format!("url has no host portion (got {value:?})"),
        });
    }
    None
}

/// Validates that a port number is in the valid range (1–65535).
pub fn validate_port(path: &str, port: u16) -> Option<ValidationFailure> {
    // u16 already ranges 0–65535; exclude 0 (reserved/unspecified).
    if port == 0 {
        Some(ValidationFailure {
            path: path.to_string(),
            message: "port must be > 0".into(),
        })
    } else {
        None
    }
}

/// Validates that a string is non-empty.
pub fn validate_non_empty(path: &str, value: &str) -> Option<ValidationFailure> {
    if value.trim().is_empty() {
        Some(ValidationFailure {
            path: path.to_string(),
            message: "value must not be empty".into(),
        })
    } else {
        None
    }
}

/// Validates that a numeric value falls within `[lo, hi]`.
pub fn validate_range<T>(path: &str, value: T, lo: T, hi: T) -> Option<ValidationFailure>
where
    T: PartialOrd + fmt::Display + Copy,
{
    if value < lo || value > hi {
        Some(ValidationFailure {
            path: path.to_string(),
            message: format!("value {value} is out of range [{lo}, {hi}]"),
        })
    } else {
        None
    }
}

/// Collects all failures from an iterator of `Option<ValidationFailure>`.
pub fn collect_failures(
    opts: impl IntoIterator<Item = Option<ValidationFailure>>,
) -> Result<(), ValidationError> {
    let failures: Vec<_> = opts.into_iter().flatten().collect();
    if failures.is_empty() {
        Ok(())
    } else {
        Err(ValidationError { failures })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_validator_pass_and_fail() {
        assert!(validate_url("db.url", "https://example.com").is_none());
        assert!(validate_url("db.url", "").is_some());
        assert!(validate_url("db.url", "ftp://bad").is_some());
        assert!(validate_url("db.url", "https://").is_some());
    }

    #[test]
    fn port_validator_rejects_zero() {
        assert!(validate_port("port", 0).is_some());
        assert!(validate_port("port", 1).is_none());
        assert!(validate_port("port", 65535).is_none());
    }

    #[test]
    fn range_validator_bounds() {
        assert!(validate_range("x", 5, 1, 10).is_none());
        assert!(validate_range("x", 10, 1, 10).is_none());
        assert!(validate_range("x", 0, 1, 10).is_some());
        assert!(validate_range("x", 11, 1, 10).is_some());
    }

    #[test]
    fn collect_failures_aggregates_all() {
        let opts = [
            validate_non_empty("a", ""),
            validate_non_empty("b", "ok"),
            validate_url("c.d", "bad://x"),
        ];
        let err = collect_failures(opts).unwrap_err();
        assert_eq!(err.failures.len(), 2);
        assert_eq!(err.failures[0].path, "a");
        assert_eq!(err.failures[1].path, "c.d");
    }

    #[test]
    fn collect_failures_ok_when_all_pass() {
        let opts = [
            validate_url("a", "https://ok.com"),
            validate_port("b", 8080),
        ];
        assert!(collect_failures(opts).is_ok());
    }

    #[test]
    fn blanket_ext_wrappers_validator() {
        struct Cfg;
        impl ConfigValidator for Cfg {
            fn validate(&self) -> Result<(), ValidationError> {
                Err(ValidationError {
                    failures: vec![ValidationFailure {
                        path: "x".into(),
                        message: "bad".into(),
                    }],
                })
            }
        }
        let c = Cfg;
        assert!(c.validate_config().is_err());
    }
}
