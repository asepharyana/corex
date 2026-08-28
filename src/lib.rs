//! # mytheclipse
//!
//! Resource-aware abstractions for async I/O, heavy compute, and
//! background queue management, built on a single lazily-initialized
//! global engine context.
//!
//! Call [`init`] once at startup (or simply let the first call to any
//! entry point below trigger it lazily) and then use whichever of the
//! feature-gated entry points your workload needs:
//!
//! - [`spawn_io`] (feature `io`) — spawn an async I/O task, tracing-instrumented.
//! - [`compute()`] (feature `compute`) — run CPU-bound work on a sized Rayon pool, panic-isolated.
//! - [`spawn_bg`] (feature `bg`) — spawn a background task under bounded concurrency.
//!
//! Enable the `full` feature to pull in all three at once.

pub mod context;
#[cfg(feature = "compute")]
pub mod error;

#[cfg(feature = "io")]
pub mod io;

#[cfg(feature = "compute")]
pub mod compute;

#[cfg(feature = "bg")]
pub mod bg;

pub use context::{context, EngineContext};

#[cfg(feature = "compute")]
pub use error::MytheclipseError;

#[cfg(feature = "io")]
pub use io::spawn_io;

#[cfg(feature = "compute")]
pub use compute::compute;

#[cfg(feature = "bg")]
pub use bg::spawn_bg;

/// Bootstraps the global [`EngineContext`].
///
/// See [`context::init`] for full semantics: this is safe to call any
/// number of times, from any thread, and is equivalent to letting the
/// first call to [`spawn_io`], [`compute()`], or [`spawn_bg`] trigger
/// initialization implicitly.
pub fn init() -> &'static EngineContext {
    context::init()
}
