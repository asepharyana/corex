//! Shared error type for mytheclipse execution primitives.

/// Errors surfaced by mytheclipse's panic-isolated execution primitives.
///
/// Marked `#[non_exhaustive]` so new variants can be added without a
/// breaking change; downstream `match` expressions should include a
/// wildcard arm.
#[non_exhaustive]
#[derive(Debug)]
pub enum MytheclipseError {
    /// A closure submitted to [`crate::compute::compute`] panicked.
    ///
    /// The contained string is a best-effort rendering of the panic
    /// payload; the compute thread pool itself remains usable afterward.
    ComputePanic(String),
}

impl std::fmt::Display for MytheclipseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ComputePanic(message) => {
                write!(f, "compute closure panicked: {message}")
            }
        }
    }
}

impl std::error::Error for MytheclipseError {}
