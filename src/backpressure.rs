//! A bounded queue with configurable overflow handling (feature `traffic`).
//!
//! [`BackpressureQueue`] buffers items up to a fixed capacity. When full, the
//! configured [`OverflowPolicy`] decides the graceful-degradation behavior:
//! drop the oldest item to make room, reject the new item and hand it back to
//! the caller, or block the caller until a slot frees up.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::Notify;
use tracing::Instrument;

/// Behaviour applied when the queue is at capacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowPolicy {
    /// Discard the oldest item to free a slot, then accept the new one.
    DropOldest,
    /// Return the new item to the caller as an error without enqueueing it.
    Reject,
    /// Block the calling coroutine until a slot frees up.
    Block,
}

/// The error returned when an item cannot be enqueued.
#[derive(Debug)]
pub enum BackpressureError<T> {
    /// The queue was full and the [`OverflowPolicy::Reject`] policy declined
    /// the item. The rejected item is returned so the caller may handle it.
    QueueFull(T),
}

impl<T> std::fmt::Display for BackpressureError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "backpressure queue is full")
    }
}

impl<T: std::fmt::Debug> std::error::Error for BackpressureError<T> {}

struct Inner<T> {
    queue: Mutex<VecDeque<T>>,
    not_full: Notify,
    not_empty: Notify,
    capacity: usize,
    policy: OverflowPolicy,
    accepted: AtomicU64,
    dropped: AtomicU64,
    rejected: AtomicU64,
    closed: AtomicU64,
}

const CLOSED: u64 = 1;
const OPEN: u64 = 0;

/// A thread-safe, bounded work queue with graceful-degradation overflow
/// handling.
///
/// Construct with [`BackpressureQueue::new`]; await [`push`](Self::push) /
/// [`pop`](Self::pop) in async contexts, or use
/// [`try_push`](Self::try_push) to fail fast.
#[derive(Clone)]
pub struct BackpressureQueue<T> {
    inner: Arc<Inner<T>>,
}

impl<T> BackpressureQueue<T> {
    /// Builds a queue with `capacity` slots and the given overflow policy.
    ///
    /// # Panics
    ///
    /// Panics if `capacity` is zero.
    pub fn new(capacity: usize, policy: OverflowPolicy) -> Self {
        assert!(capacity > 0, "backpressure queue capacity must be > 0");
        Self {
            inner: Arc::new(Inner {
                queue: Mutex::new(VecDeque::with_capacity(capacity)),
                not_full: Notify::new(),
                not_empty: Notify::new(),
                capacity,
                policy,
                accepted: AtomicU64::new(0),
                dropped: AtomicU64::new(0),
                rejected: AtomicU64::new(0),
                closed: AtomicU64::new(OPEN),
            }),
        }
    }

    /// Enqueues `item`, applying the configured overflow policy when full.
    ///
    /// For [`OverflowPolicy::Block`], awaits a free slot first (in a
    /// `mytheclipse_backpressure_task` span); for [`OverflowPolicy::Reject`],
    /// returns the item back as [`BackpressureError::QueueFull`] without
    /// blocking; for [`OverflowPolicy::DropOldest`], never fails and evicts
    /// the oldest item.
    pub async fn push(&self, item: T) -> Result<(), BackpressureError<T>> {
        if self.inner.closed.load(Ordering::Acquire) == CLOSED {
            return Err(BackpressureError::QueueFull(item));
        }
        match self.inner.policy {
            OverflowPolicy::Block => {
                let span = tracing::info_span!("mytheclipse_backpressure_task");
                loop {
                    {
                        let mut queue = self.inner.queue.lock().unwrap();
                        if queue.len() < self.inner.capacity {
                            queue.push_back(item);
                            self.inner.accepted.fetch_add(1, Ordering::AcqRel);
                            drop(queue);
                            self.inner.not_empty.notify_one();
                            return Ok(());
                        }
                    }
                    let waiter = self.inner.not_full.notified();
                    tokio::pin!(waiter);
                    waiter.as_mut().instrument(span.clone()).await;
                }
            }
            OverflowPolicy::Reject => self.try_push(item),
            OverflowPolicy::DropOldest => {
                self.try_push_dropping(item);
                Ok(())
            }
        }
    }

    /// Non-blocking enqueue honoring [`OverflowPolicy::DropOldest`] and
    /// [`OverflowPolicy::Reject`]; for [`OverflowPolicy::Block`] it behaves
    /// like [`OverflowPolicy::Reject`] (never blocks).
    pub fn try_push(&self, item: T) -> Result<(), BackpressureError<T>> {
        let mut queue = self.inner.queue.lock().unwrap();
        if self.inner.closed.load(Ordering::Acquire) == CLOSED {
            return Err(BackpressureError::QueueFull(item));
        }
        if queue.len() >= self.inner.capacity {
            match self.inner.policy {
                OverflowPolicy::DropOldest => {
                    let _dropped = queue.pop_front();
                    self.inner.dropped.fetch_add(1, Ordering::AcqRel);
                }
                _ => {
                    self.inner.rejected.fetch_add(1, Ordering::AcqRel);
                    return Err(BackpressureError::QueueFull(item));
                }
            }
        }
        queue.push_back(item);
        self.inner.accepted.fetch_add(1, Ordering::AcqRel);
        drop(queue);
        self.inner.not_empty.notify_one();
        Ok(())
    }

    fn try_push_dropping(&self, item: T) {
        let mut queue = self.inner.queue.lock().unwrap();
        if queue.len() >= self.inner.capacity {
            let _dropped = queue.pop_front();
            self.inner.dropped.fetch_add(1, Ordering::AcqRel);
        }
        queue.push_back(item);
        self.inner.accepted.fetch_add(1, Ordering::AcqRel);
        drop(queue);
        self.inner.not_empty.notify_one();
    }

    /// Awaits the next item, blocking until one is available.
    pub async fn pop(&self) -> Option<T> {
        loop {
            {
                let mut queue = self.inner.queue.lock().unwrap();
                if let Some(item) = queue.pop_front() {
                    self.inner.not_full.notify_one();
                    return Some(item);
                }
                if self.inner.closed.load(Ordering::Acquire) == CLOSED && queue.is_empty() {
                    return None;
                }
            }
            self.inner.not_empty.notified().await;
        }
    }

    /// Attempts to pop an item without blocking.
    pub fn try_pop(&self) -> Option<T> {
        let mut queue = self.inner.queue.lock().unwrap();
        let item = queue.pop_front();
        if item.is_some() {
            self.inner.not_full.notify_one();
        }
        item
    }

    /// The number of items currently buffered.
    pub fn len(&self) -> usize {
        self.inner.queue.lock().unwrap().len()
    }

    /// Whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The queue's capacity.
    pub fn capacity(&self) -> usize {
        self.inner.capacity
    }

    /// How much room remains.
    pub fn remaining(&self) -> usize {
        self.inner.capacity - self.len()
    }

    /// Number of items successfully enqueued.
    pub fn accepted_count(&self) -> u64 {
        self.inner.accepted.load(Ordering::Acquire)
    }

    /// Number of items evicted by drop policies.
    pub fn dropped_count(&self) -> u64 {
        self.inner.dropped.load(Ordering::Acquire)
    }

    /// Number of items rejected by the reject/closed paths.
    pub fn rejected_count(&self) -> u64 {
        self.inner.rejected.load(Ordering::Acquire)
    }

    /// Closes the queue: no further items are accepted and `pop` drains the
    /// remaining items then returns `None`.
    pub fn close(&self) {
        self.inner.closed.store(CLOSED, Ordering::Release);
        self.inner.not_empty.notify_waiters();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn push_pop_roundtrip() {
        let q: BackpressureQueue<u32> = BackpressureQueue::new(4, OverflowPolicy::Reject);
        q.push(1).await.unwrap();
        q.push(2).await.unwrap();
        assert_eq!(q.pop().await, Some(1));
        assert_eq!(q.pop().await, Some(2));
        assert!(q.is_empty());
    }

    #[tokio::test]
    async fn reject_returns_item_when_full() {
        let q: BackpressureQueue<u32> = BackpressureQueue::new(2, OverflowPolicy::Reject);
        q.push(1).await.unwrap();
        q.push(2).await.unwrap();
        let err = q.push(3).await.unwrap_err();
        match err {
            BackpressureError::QueueFull(v) => assert_eq!(v, 3),
        }
        assert_eq!(q.len(), 2);
        assert_eq!(q.rejected_count(), 1);
    }

    #[tokio::test]
    async fn drop_oldest_evicts_front() {
        let q: BackpressureQueue<u32> = BackpressureQueue::new(2, OverflowPolicy::DropOldest);
        q.push(1).await.unwrap();
        q.push(2).await.unwrap();
        q.push(3).await.unwrap();
        assert_eq!(q.len(), 2);
        assert_eq!(q.pop().await, Some(2)); // 1 was dropped
        assert_eq!(q.dropped_count(), 1);
    }

    #[tokio::test]
    async fn block_waits_for_a_slot() {
        let q: BackpressureQueue<u32> = BackpressureQueue::new(1, OverflowPolicy::Block);
        q.push(1).await.unwrap();
        // A second push must block until pop frees a slot.
        let q2 = q.clone();
        let pusher = tokio::spawn(async move {
            q2.push(2).await.unwrap();
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(q.pop().await, Some(1));
        pusher.await.unwrap();
        assert_eq!(q.pop().await, Some(2));
        assert_eq!(q.accepted_count(), 2);
    }

    #[tokio::test]
    async fn close_drains_then_returns_none() {
        let q: BackpressureQueue<u32> = BackpressureQueue::new(2, OverflowPolicy::Reject);
        q.push(1).await.unwrap();
        q.push(2).await.unwrap();
        let q2 = q.clone();
        let drainer = tokio::spawn(async move {
            let mut seen = Vec::new();
            while let Some(v) = q2.pop().await {
                seen.push(v);
            }
            seen
        });
        q.close();
        assert_eq!(drainer.await.unwrap(), vec![1, 2]);
        assert!(q.push(3).await.is_err());
    }
}
