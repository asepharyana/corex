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
//!
//! ## Quick Start
//!
//! ```toml
//! [dependencies]
//! mytheclipse-queue = "0.2"
//! ```
//!
//! ```ignore
//! use mytheclipse_queue::{InMemoryQueue, WorkerPool, JobHandler, Job};
//! ...
//! let queue = InMemoryQueue::new();
//! queue.enqueue("email", b"hello".to_vec()).await?;
//!
//! fn make_handler() -> impl JobHandler {
//!     struct PrintHandler;
//!     impl JobHandler for PrintHandler {
//!         fn handle(&self, job: Job) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), mytheclipse_queue::JobError>> + Send>> {
//!             Box::pin(async move {
//!                 println!("payload: {:?}", job.payload);
//!                 Ok(())
//!             })
//!         }
//!     }
//!     PrintHandler
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let queue = InMemoryQueue::new();
//! queue.enqueue("email", b"hello".to_vec()).await?;
//!
//! let pool = WorkerPool::new(queue, 4);
//! pool.start("email", make_handler());
//! # Ok(())
//! # }
//! ```

pub mod error;
pub mod job;
#[cfg(feature = "in-memory")]
pub mod in_memory;
pub mod traits;
pub mod worker;

#[cfg(feature = "in-memory")]
pub mod batch;
#[cfg(feature = "in-memory")]
pub mod pipeline;

#[cfg(feature = "in-memory")]
pub use in_memory::InMemoryQueue;

pub use traits::Queue;
pub use job::{Job, JobId};
pub use worker::{WorkerPool, WorkerConfig, JobHandler, JobFuture};
pub use error::{QueueError, JobError};

#[cfg(feature = "in-memory")]
pub use batch::{BatchConfig, BatchJobHandler, BatchProcessor, BatchFlush};
#[cfg(feature = "in-memory")]
pub use pipeline::{StageRunner, Stage, StageError};
