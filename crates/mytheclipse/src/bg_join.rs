//! Graceful task-joiner for background tasks (feature `lifecycle`).
//!
//! [`BgJoiner`] collects [`tokio::task::JoinHandle`]s returned by
//! [`crate::spawn_bg`] (or any manual `tokio::spawn`) and drains them in
//! aggregate on shutdown via [`BgJoiner::join_all`].
//!
//! This complements the bounded `spawn_bg` helper: while `spawn_bg` limits
//! *concurrency*, `BgJoiner` adds structured *lifetimes* so a service can wait
//! for all in-flight work to settle before terminating.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::Instant;

/// A joiner that tracks background task handles for ordered shutdown.
#[derive(Default, Clone)]
pub struct BgJoiner {
    inner: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

impl BgJoiner {
    /// Creates an empty joiner.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Spawns `future` as a background task and tracks its handle.
    pub fn spawn<F>(&self, future: F)
    where
        F: std::future::Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let handle: JoinHandle<()> = tokio::spawn(async move { let _ = future.await; });
        self.track(handle);
    }

    /// Registers an externally-created `JoinHandle` for tracking.
    pub fn track(&self, handle: JoinHandle<()>) {
        // can't lock synchronously; defer to a spawned task
        let inner = Arc::clone(&self.inner);
        tokio::spawn(async move {
            let mut set = inner.lock().await;
            set.push(handle);
        });
    }

    /// Number of currently-tracked tasks.
    pub async fn len(&self) -> usize {
        self.inner.lock().await.len()
    }

    /// Await every tracked task, dropping any that are still pending once
    /// `deadline` elapses. Returns the count of tasks that had not completed
    /// within the timeout.
    pub async fn join_all(&self, deadline: Duration) -> usize {
        let now = Instant::now();
        let handles: Vec<JoinHandle<()>> = {
            let mut guard = self.inner.lock().await;
            std::mem::take(&mut *guard)
        };

        let mut pending: Vec<JoinHandle<()>> = handles;
        let mut dropped = 0usize;

        loop {
            if pending.is_empty() {
                break 0;
            }

            if now.elapsed() >= deadline {
                dropped = pending.len();
                for h in pending.drain(..) {
                    h.abort();
                }
                return dropped;
            }

            let remaining = deadline.saturating_sub(now.elapsed());
            let mut still = Vec::with_capacity(pending.len());
            for mut handle in pending.drain(..) {
                match tokio::time::timeout(remaining, &mut handle).await {
                    Ok(Ok(_)) => {}
                    Ok(Err(_)) => {}
                    Err(_) => still.push(handle),
                }
            }
            pending = still;
        }
    }

    /// Drops (aborts) all tracked tasks immediately without awaiting.
    pub async fn abort_all(&self) {
        let handles: Vec<JoinHandle<()>> = {
            let mut guard = self.inner.lock().await;
            std::mem::take(&mut *guard)
        };
        for h in handles {
            h.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn spawn_and_join_completes() {
        let joiner = BgJoiner::new();
        joiner.spawn(async { tokio::task::yield_now().await });
        // give the task a moment to register
        tokio::time::sleep(Duration::from_millis(10)).await;
        let leftover = joiner.join_all(Duration::from_secs(1)).await;
        assert_eq!(leftover, 0);
    }

    #[tokio::test]
    async fn join_all_aborts_on_timeout() {
        let joiner = BgJoiner::new();
        joiner.spawn(async {
            tokio::time::sleep(Duration::from_secs(10)).await;
        });
        tokio::time::sleep(Duration::from_millis(10)).await;
        let leftover = joiner.join_all(Duration::from_millis(50)).await;
        assert!(leftover > 0);
    }
}
