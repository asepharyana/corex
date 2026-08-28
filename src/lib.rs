//! # mytheclipse
//!
//! Resource-aware abstractions for async I/O, heavy compute, background queue
//! management, resiliency, traffic control, lifecycle, and observability —
//! built on a single lazily-initialized global engine context plus a set of
//! self-contained, constructible utilities.
//!
//! Call [`init`] once at startup (or simply let the first call to any
//! entry point below trigger it lazily) and then use whichever of the
//! feature-gated entry points your workload needs:
//!
//! - [`spawn_io`] (feature `io`) — spawn an async I/O task, tracing-instrumented.
//! - [`compute()`] (feature `compute`) — run CPU-bound work on a sized Rayon pool, panic-isolated.
//! - [`spawn_bg`] (feature `bg`) — spawn a background task under bounded concurrency.
//! - [`retry()`] / [`circuit_breaker`] / [`timeout()`] (feature `resiliency`) — fault tolerance.
//! - [`ratelimit`] / [`backpressure`] / [`concurrency`] (feature `traffic`) — load control.
//! - [`shutdown`] / [`cron`] (feature `lifecycle`) — lifecycle management.
//! - [`metrics`] / [`panic_tracker`] (feature `observability`) — runtime visibility.
//!
//! Enable the `full` feature to pull in all of the above at once. The three
//! execution primitives are sized from the host's logical core count via the
//! engine context; the resiliency/traffic/lifecycle/observability utilities
//! are self-contained and constructed explicitly (e.g.
//! `RateLimiter::new(...)`, `CircuitBreaker::new(...)`).

pub mod context;
pub mod error;

#[cfg(feature = "io")]
pub mod io;

#[cfg(feature = "compute")]
pub mod compute;

#[cfg(feature = "bg")]
pub mod bg;

#[cfg(feature = "resiliency")]
pub mod retry;

#[cfg(feature = "resiliency")]
pub mod circuit_breaker;

#[cfg(feature = "resiliency")]
pub mod timeout;

#[cfg(feature = "traffic")]
pub mod ratelimit;

#[cfg(feature = "traffic")]
pub mod backpressure;

#[cfg(feature = "traffic")]
pub mod concurrency;

#[cfg(feature = "lifecycle")]
pub mod shutdown;

#[cfg(feature = "lifecycle")]
pub mod cron;

#[cfg(feature = "observability")]
pub mod metrics;

#[cfg(feature = "observability")]
pub mod panic_tracker;

pub use context::{context, EngineContext};
pub use error::MytheclipseError;

#[cfg(feature = "io")]
pub use io::spawn_io;

#[cfg(feature = "compute")]
pub use compute::compute;

#[cfg(feature = "bg")]
pub use bg::spawn_bg;

#[cfg(feature = "resiliency")]
pub use retry::{retry, JitterKind, RetryConfig, RetryError};

#[cfg(feature = "resiliency")]
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitError, CircuitState};

#[cfg(feature = "resiliency")]
pub use timeout::{timeout, with_timeout, Timeout, TimeoutError};

#[cfg(feature = "traffic")]
pub use ratelimit::{RateLimitError, RateLimiter};

#[cfg(feature = "traffic")]
pub use backpressure::{BackpressureError, BackpressureQueue, OverflowPolicy};

#[cfg(feature = "traffic")]
pub use concurrency::{ConcurrencyLimiter, ConcurrencyPermit};

#[cfg(feature = "lifecycle")]
pub use shutdown::{ShutdownManager, ShutdownSignal};

#[cfg(feature = "lifecycle")]
pub use cron::{schedule, CronError, CronJob, CronParseError, CronSchedule};

#[cfg(feature = "observability")]
pub use metrics::{MetricsCollector, MetricsSnapshot};

#[cfg(feature = "observability")]
pub use panic_tracker::{PanicGuard, PanicInfo, PanicTracker};

/// Bootstraps the global [`EngineContext`].
///
/// See [`context::init`] for full semantics: this is safe to call any
/// number of times, from any thread, and is equivalent to letting the
/// first call to [`spawn_io`], [`compute()`], or [`spawn_bg`] trigger
/// initialization implicitly.
pub fn init() -> &'static EngineContext {
    context::init()
}
