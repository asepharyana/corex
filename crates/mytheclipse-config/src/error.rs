//! Shared error type for mytheclipse-config.

/// Errors surfaced while loading or reloading configuration.
#[non_exhaustive]
#[derive(Debug)]
pub enum ConfigError {
    /// An I/O error reading a config source (file not found, permissions, ...).
    Io(String),
    /// A source could not be parsed (malformed YAML/JSON/TOML).
    Parse(String),
    /// The merged configuration could not be deserialized into the target type.
    Deserialize(String),
    /// The requested file extension has no registered loader (feature not
    /// enabled, or unsupported format).
    UnsupportedFormat(String),
    /// Hot-reload setup failed (e.g. the file watcher could not be installed).
    Watch(String),
    /// Config validation failed after loading.
    Validation(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(s) => write!(f, "config io error: {s}"),
            Self::Parse(s) => write!(f, "config parse error: {s}"),
            Self::Deserialize(s) => write!(f, "config deserialize error: {s}"),
            Self::UnsupportedFormat(s) => write!(f, "unsupported config format: {s}"),
            Self::Watch(s) => write!(f, "config watch error: {s}"),
            Self::Validation(s) => write!(f, "config validation error: {s}"),
        }
    }
}

impl std::error::Error for ConfigError {}
