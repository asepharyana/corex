//! Graceful shutdown coordination (feature `lifecycle`).
//!
//! [`ShutdownManager`] watches for OS signals (SIGINT/SIGTERM on Unix, Ctrl-C
//! everywhere) and broadcasts a shutdown signal to every registered task, then
//! waits for those tasks to finish before the process exits. Tasks subscribe
//! via a cloneable [`ShutdownSignal`] and cooperatively stop when it fires.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;
use tracing::Instrument;

/// A cloneable token a task holds to observe whether shutdown has been
/// requested.
#[derive(Clone)]
pub struct ShutdownSignal {
    rx: watch::Receiver<bool>,
}

impl ShutdownSignal {
    /// Whether shutdown has been requested.
    pub fn is_shutdown(&self) -> bool {
        *self.rx.borrow()
    }

    /// Awaits until shutdown is requested, then returns.
    pub async fn wait(&mut self) {
        if *self.rx.borrow() {
            return;
        }
        // Borrow avoids holding the receiver while yielding.
        let _ = self.rx.changed().await;
    }
}

/// Coordinates graceful shutdown of tracked background tasks.
///
/// Construct with [`ShutdownManager::new`], hand each long-running task a
/// [`ShutdownSignal`] via [`ShutdownManager::handle`], register its join
/// handle with [`ShutdownManager::register`], and finally defer the process
/// exit until [`ShutdownManager::drain`] completes.
#[derive(Clone)]
pub struct ShutdownManager {
    inner: Arc<Inner>,
}

struct Inner {
    tx: watch::Sender<bool>,
    /// Kept alive so `tx.send` always has a receiver and therefore always
    /// updates the stored value — even if `request()` fires before any
    /// [`ShutdownSignal`] has been handed out.
    _keepalive: watch::Receiver<bool>,
    tasks: std::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>,
}

impl Default for ShutdownManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ShutdownManager {
    /// Builds a new shutdown manager with no tasks tracked.
    pub fn new() -> Self {
        let (tx, keepalive) = watch::channel(false);
        Self {
            inner: Arc::new(Inner {
                tx,
                _keepalive: keepalive,
                tasks: std::sync::Mutex::new(Vec::new()),
            }),
        }
    }

    /// Returns a new [`ShutdownSignal`] this manager will fire on shutdown.
    pub fn handle(&self) -> ShutdownSignal {
        ShutdownSignal {
            rx: self.inner.tx.subscribe(),
        }
    }

    /// Registers a background task so shutdown waits for it to complete.
    ///
    /// The task is expected to observe its [`ShutdownSignal`] and stop
    /// promptly once it fires; `drain` gives tasks a grace window.
    pub fn register(&self, handle: tokio::task::JoinHandle<()>) {
        self.inner.tasks.lock().unwrap().push(handle);
    }

    /// Whether shutdown has been requested.
    pub fn is_shutdown(&self) -> bool {
        *self.inner.tx.borrow()
    }

    /// Awaits until shutdown is requested (by an OS signal or an explicit
    /// [`ShutdownManager::request`]).
    pub async fn wait_for_shutdown(&self) {
        let mut rx = self.inner.tx.subscribe();
        if *rx.borrow() {
            return;
        }
        let _ = rx.changed().await;
    }

    /// Requests shutdown programmatically (also invoked by the signal
    /// handler). Safe to call more than once.
    pub fn request(&self) {
        let _ = self.inner.tx.send(true);
    }

    /// Waits for shutdown to be requested, then awaits all registered tasks,
    /// allowing at most `grace` per task before giving up.
    ///
    /// Runs in a `mytheclipse_shutdown_task` tracing span.
    pub async fn drain(&self, grace: Duration) {
        let span = tracing::info_span!("mytheclipse_shutdown_task");
        self.wait_for_shutdown().instrument(span.clone()).await;

        let tasks = {
            let mut guard = self.inner.tasks.lock().unwrap();
            std::mem::take(&mut *guard)
        };
        for task in tasks {
            let _ = tokio::time::timeout(grace, task)
                .instrument(span.clone())
                .await;
        }
    }

    /// Waits for the next OS termination signal (SIGINT/SIGTERM on Unix,
    /// Ctrl-C elsewhere).
    pub async fn wait_for_os_signal(&self) {
        let _ = os_signal().await;
    }
}

/// Awaits SIGINT/SIGTERM on Unix, or Ctrl-C on other platforms.
#[cfg(unix)]
async fn os_signal() {
    use tokio::signal::unix::{signal, SignalKind};

    let mut sigint = signal(SignalKind::interrupt()).expect("failed to install SIGINT handler");
    let mut sigterm = signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");

    tokio::select! {
        _ = sigint.recv() => {},
        _ = sigterm.recv() => {},
    }
}

/// Awaits Ctrl-C on non-Unix platforms.
#[cfg(not(unix))]
async fn os_signal() {
    use tokio::signal::ctrl_c;
    let _ = ctrl_c().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn signal_fires_on_request() {
        let manager = ShutdownManager::new();
        let mut sig = manager.handle();
        assert!(!sig.is_shutdown());
        assert!(!manager.is_shutdown());

        let observer = tokio::spawn(async move {
            sig.wait().await;
            sig.is_shutdown()
        });

        manager.request();
        assert!(manager.is_shutdown());
        assert!(observer.await.unwrap());
    }

    #[tokio::test]
    async fn request_is_idempotent() {
        let manager = ShutdownManager::new();
        manager.request();
        manager.request();
        let sig = manager.handle();
        assert!(sig.is_shutdown());
    }

    #[tokio::test]
    async fn drain_waits_for_registered_tasks() {
        let manager = ShutdownManager::new();
        let sig = manager.handle();
        let handle = tokio::spawn(async move {
            let mut sig = sig;
            sig.wait().await;
        });
        manager.register(handle);
        manager.request();
        // drain completes promptly because the task stops on the signal.
        manager.drain(Duration::from_secs(5)).await;
    }

    #[tokio::test]
    async fn drain_times_out_a_slow_task() {
        let manager = ShutdownManager::new();
        let _sig = manager.handle();
        // A task that never observes shutdown — it must be timed out.
        let slow = tokio::spawn(std::future::pending::<()>());
        manager.register(slow);
        manager.request();
        // drain should return (time out) rather than hang forever.
        manager.drain(Duration::from_millis(50)).await;
    }
}
