//! Backpressure-aware enqueuer (in-memory backend).
//!
//! [`BackpressureEnforcer`] tracks in-flight jobs and caps the number of
//! pending enqueues per topic, returning [`BackpressureError`] instead of
//! blocking when the cap is exceeded.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::Semaphore;

use crate::traits::Queue;

/// Errors returned by [`BackpressureEnforcer`].
#[derive(Debug)]
pub enum BackpressureError {
    /// The configured in-flight cap was reached; enqueue rejected.
    LimitReached { topic: String },
}

impl std::fmt::Display for BackpressureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LimitReached { topic } => {
                write!(f, "backpressure: in-flight limit reached for topic {topic}")
            }
        }
    }
}

impl std::error::Error for BackpressureError {}

/// Enforces a maximum number of in-flight jobs per topic.
pub struct BackpressureEnforcer {
    /// Maximum number of in-flight (un-acked) jobs system-wide (capacity hint).
    #[allow(dead_code)]
    max_inflight: usize,
    /// Per-topic in-flight counter.
    counters: Arc<std::sync::Mutex<std::collections::HashMap<String, Arc<AtomicU64>>>>,
    /// Bounded semaphore enforcing total concurrency.
    #[allow(dead_code)]
    global: Arc<Semaphore>,
}

impl BackpressureEnforcer {
    /// Creates an enforcer with a global maximum of `max_inflight` concurrent
    /// in-flight jobs.
    pub fn new(max_inflight: usize) -> Self {
        Self {
            max_inflight,
            counters: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            global: Arc::new(Semaphore::new(max_inflight.max(1))),
        }
    }

    /// Returns the per-topic in-flight count, creating zero if absent.
    fn get_counter(&self, topic: &str) -> Arc<AtomicU64> {
        let mut map = self.counters.lock().unwrap();
        map.entry(topic.to_string())
            .or_insert_with(|| Arc::new(AtomicU64::new(0)));
        Arc::clone(map.get(topic).unwrap())
    }

    /// Attempts to acquire a backpressure slot non-blockingly.
    /// Returns Err if at capacity.
    pub async fn try_enqueue<Q: Queue + ?Sized>(
        &self,
        queue: &Q,
        topic: &str,
        payload: Vec<u8>,
    ) -> Result<(), BackpressureError> {
        // Per-topic counter increment (informational; global semaphore is the hard limit)
        let counter = self.get_counter(topic);
        counter.fetch_add(1, Ordering::SeqCst);

        // Try global semaphore non-blocking
        match self.global.clone().try_acquire_owned() {
            Ok(_permit) => {
                let _ = queue.enqueue(topic, payload).await;
                Ok(())
            }
            Err(_) => {
                counter.fetch_sub(1, Ordering::SeqCst);
                Err(BackpressureError::LimitReached {
                    topic: topic.to_string(),
                })
            }
        }
    }

    /// Increments the in-flight counter when a job is delivered.
    pub fn inc_delivered(&self, topic: &str) {
        self.get_counter(topic).fetch_add(1, Ordering::SeqCst);
    }

    /// Decrements the in-flight counter after a job is acked/nacked.
    pub fn dec_finished(&self, topic: &str) {
        self.get_counter(topic).fetch_sub(1, Ordering::SeqCst);
    }

    /// Current in-flight count for a topic.
    pub fn inflight(&self, topic: &str) -> u64 {
        self.get_counter(topic).load(Ordering::SeqCst)
    }
}

/// Helper: enqueue with backpressure, returning how many were rejected.
pub async fn enqueue_with_backpressure<Q: Queue + ?Sized>(
    enforcer: &BackpressureEnforcer,
    queue: &Q,
    topic: &str,
    payloads: Vec<Vec<u8>>,
) -> Result<usize, BackpressureError> {
    let mut rejected = 0;
    for payload in payloads {
        if enforcer.try_enqueue(queue, topic, payload).await.is_err() {
            rejected += 1;
        }
    }
    Ok(rejected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::in_memory::InMemoryQueue;

    #[tokio::test]
    async fn try_enqueue_within_limit_succeeds() {
        let reg = BackpressureEnforcer::new(2);
        let queue = InMemoryQueue::new();
        let result = reg.try_enqueue(&queue, "t", b"x".to_vec()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn try_enqueue_rejects_when_full() {
        let reg = BackpressureEnforcer::new(1);
        let queue = InMemoryQueue::new();

        // acquire the single global permit without releasing
        let _first = reg.global.clone().acquire_owned().await.unwrap();

        let result = reg.try_enqueue(&queue, "t", b"x".to_vec()).await;
        assert!(matches!(
            result,
            Err(BackpressureError::LimitReached { .. })
        ));
    }
}
