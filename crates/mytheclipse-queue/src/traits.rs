//! The core `Queue` trait and supporting types.

use async_trait::async_trait;
use std::time::Duration;

use crate::job::Job;
use crate::error::{QueueError, JobError};

/// A handle to a single unit of queued work.
///
/// `Job` carries the raw payload (arbitrary bytes — caller decides encoding)
/// plus metadata the queue implementation fills in (ID, enqueue time, retry
/// count). `ack`/`nack` are only valid on backends that support explicit
/// acknowledgment (NATS, Redis BLPOP-with-confirm). For in-memory and Postgres
/// backends, the worker auto-acknowledges on `Ok` and auto-requeues on `Err`.
///
/// A trait for enqueueing and dequeueing jobs.
///
/// Implementations must be `Send + Sync`. Each backend provides its own factory
/// (e.g. `InMemoryQueue::new()`, `RedisQueue::connect(url)`).
#[async_trait]
pub trait Queue: Send + Sync {
    /// Enqueues `payload` onto `topic`.
    async fn enqueue(&self, topic: &str, payload: Vec<u8>) -> Result<(), QueueError>;

    /// Dequeues the next job from `topic`, waiting up to `timeout`.
    ///
    /// Returns `None` on timeout when the queue is empty and no job arrives
    /// within the window.
    async fn dequeue(&self, topic: &str, timeout: Duration) -> Result<Option<Job>, QueueError>;

    /// Acknowledges a job as successfully processed.
    async fn ack(&self, job: &Job) -> Result<(), JobError>;

    /// Negative-acknowledges a job. If `requeue` is true the job goes back
    /// onto the queue; if false it moves to the dead-letter queue (if
    /// configured).
    async fn nack(&self, job: &Job, requeue: bool) -> Result<(), JobError>;

    /// Moves a job to the dead-letter queue for `topic`.
    async fn dlq_move(&self, topic: &str, job: Job) -> Result<(), QueueError>;

    /// Number of messages currently waiting in `topic`.
    async fn len(&self, topic: &str) -> Result<u64, QueueError>;

    /// Whether the queue is empty.
    async fn is_empty(&self, topic: &str) -> Result<bool, QueueError> {
        Ok(self.len(topic).await? == 0)
    }
}
