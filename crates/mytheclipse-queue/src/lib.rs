//! # mytheclipse-queue
//!
//! A unified job queue abstraction so your background work isn't locked to one
//! transport. Provides a single `Queue` trait, `Job` type, and `WorkerPool`
//! executor with configurable retry/backoff, concurrency, and a dead-letter
//! queue — behind pluggable backends:
//!
//! - **In-memory** (default) — `tokio::sync::mpsc` + task spawning, no external service.
//! - **Redis** (`redis`) — LIST-based queue with atomic moves.
//! - **NATS JetStream** (`nats`) — durable consumer with ACK/NACK.
//! - **PostgreSQL** (`postgres`) — `SKIP LOCKED` polling.

pub mod error;
pub mod job;
#[cfg(feature = "in-memory")]
pub mod in_memory;
pub mod traits;
pub mod worker;

#[cfg(feature = "in-memory")]
pub mod batch;
#[cfg(feature = "in-memory")]
pub mod backpressure_enqueue;
#[cfg(feature = "in-memory")]
pub mod rate_limited;
#[cfg(feature = "in-memory")]
pub mod worker_rate_limited;
#[cfg(feature = "in-memory")]
pub use backpressure_enqueue::{BackpressureEnforcer, BackpressureError, enqueue_with_backpressure};
#[cfg(feature = "in-memory")]
pub use rate_limited::{RateLimitedQueue, RateLimitQueueError};
#[cfg(feature = "in-memory")]
pub use worker_rate_limited::RateLimitedWorkerPool;

#[cfg(feature = "in-memory")]
pub mod pipeline;
#[cfg(feature = "in-memory")]
pub use pipeline::{StageRunner, Stage, StageError};
