//! Rate-limited worker pool (feature `in-memory`).
//!
//! [`RateLimitedWorkerPool`] wraps [`crate::worker::WorkerPool`] with a
//! [`crate::rate_limited::RateLimitedQueue`] to back-pressure dequeue when the
//! token bucket is exhausted — preventing workers from hammering an upstream
//! service faster than its rate limit allows.

use crate::rate_limited::RateLimitedQueue;
use crate::traits::Queue;
use crate::worker::{JobHandler, WorkerConfig, WorkerPool};

/// A `WorkerPool` whose dequeue is rate-limited via a token bucket.
pub struct RateLimitedWorkerPool<Q: Queue + 'static> {
    inner: WorkerPool<RateLimitedQueue<Q>>,
}

impl<Q: Queue + 'static> RateLimitedWorkerPool<Q> {
    /// Creates a rate-limited worker pool wrapping `queue` with the given
    /// token-bucket rate (tokens/sec) and burst capacity.
    pub fn new(queue: Q, worker_cfg: WorkerConfig, rate_per_sec: f64, burst: u32) -> Self {
        let limited = RateLimitedQueue::new(queue, rate_per_sec, burst);
        Self {
            inner: WorkerPool::with_config(limited, worker_cfg),
        }
    }

    /// Starts workers consuming from `topic` with the given handler.
    pub fn start<H>(&self, topic: &str, handler: H)
    where
        H: JobHandler + 'static,
    {
        self.inner.start(topic, handler);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_rate_limited_pool() {
        use crate::in_memory::InMemoryQueue;
        let _pool =
            RateLimitedWorkerPool::new(InMemoryQueue::new(), WorkerConfig::default(), 10.0, 5);
        // smoke: just verifies construction
    }
}
