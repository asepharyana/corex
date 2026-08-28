//! Runtime configuration hot-reload (feature `hot-reload`).
//!
//! [`DynamicConfig`] holds a typed configuration value behind an `RwLock`,
//! optionally watching a set of source files with `notify` and re-running a
//! caller-supplied reload closure whenever they change. Subscribers can await
//! a [`tokio::sync::broadcast::Receiver`] to react to a successful reload.

use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::broadcast;

use crate::{Config, ConfigError};

/// A hot-reloadable, thread-safe configuration handle.
pub struct DynamicConfig<T> {
    inner: Arc<RwLock<T>>,
    tx: broadcast::Sender<()>,
}

impl<T: Config + Clone> Clone for DynamicConfig<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            tx: self.tx.clone(),
        }
    }
}

impl<T: Config + Clone> DynamicConfig<T> {
    /// Wraps an already-loaded value with no file watching.
    pub fn new(initial: T) -> Self {
        let (tx, _rx) = broadcast::channel(16);
        Self {
            inner: Arc::new(RwLock::new(initial)),
            tx,
        }
    }

    /// Returns a clone of the current configuration snapshot.
    pub fn get(&self) -> T {
        self.inner
            .read()
            .expect("corex-config: RwLock poisoned")
            .clone()
    }

    /// Replaces the current value and notifies subscribers.
    pub fn set(&self, new: T) {
        *self.inner.write().expect("corex-config: RwLock poisoned") = new;
        let _ = self.tx.send(());
    }

    /// Subscribes to change notifications (fired after every successful
    /// [`DynamicConfig::set`] or file-triggered reload).
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.tx.subscribe()
    }

    /// Loads the initial value via `reload`, then watches `paths` for
    /// modifications and re-runs `reload` on each change (debounced), atomically
    /// swapping in the result on success. A failed reload is logged via
    /// `tracing::error!` and the previous value is retained.
    ///
    /// The underlying OS file watcher lives on a dedicated background thread
    /// for the lifetime of the process; there is currently no explicit
    /// "unwatch" — construct one `DynamicConfig` per watched file set.
    pub fn watch_files<F>(paths: Vec<PathBuf>, reload: F) -> Result<Self, ConfigError>
    where
        F: Fn() -> Result<T, ConfigError> + Send + Sync + 'static,
    {
        Self::watch_files_debounced(paths, reload, Duration::from_millis(50))
    }

    /// Same as [`DynamicConfig::watch_files`] with an explicit debounce
    /// window (the minimum time between two applied reloads).
    pub fn watch_files_debounced<F>(
        paths: Vec<PathBuf>,
        reload: F,
        debounce: Duration,
    ) -> Result<Self, ConfigError>
    where
        F: Fn() -> Result<T, ConfigError> + Send + Sync + 'static,
    {
        let initial = reload()?;
        let config = Self::new(initial);
        let inner = Arc::clone(&config.inner);
        let tx = config.tx.clone();

        let (raw_tx, raw_rx) = std::sync::mpsc::channel();
        let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res| {
            let _ = raw_tx.send(res);
        })
        .map_err(|e| ConfigError::Watch(e.to_string()))?;
        for path in &paths {
            watcher
                .watch(path, RecursiveMode::NonRecursive)
                .map_err(|e| ConfigError::Watch(e.to_string()))?;
        }

        std::thread::Builder::new()
            .name("corex-config-watch".into())
            .spawn(move || {
                // Keep the watcher alive for the life of this thread.
                let _watcher = watcher;
                let mut last_applied = Instant::now() - debounce;
                for event in raw_rx {
                    if event.is_err() {
                        continue;
                    }
                    if last_applied.elapsed() < debounce {
                        continue;
                    }
                    match reload() {
                        Ok(new) => {
                            *inner.write().expect("corex-config: RwLock poisoned") = new;
                            let _ = tx.send(());
                            last_applied = Instant::now();
                        }
                        Err(e) => {
                            tracing::error!("corex-config: hot-reload failed: {e}");
                        }
                    }
                }
            })
            .map_err(|e| ConfigError::Watch(e.to_string()))?;

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConfigLoader;
    use serde::Deserialize;
    use std::io::Write;

    #[derive(Debug, Deserialize, Clone, PartialEq)]
    struct Cfg {
        value: u32,
    }

    #[test]
    fn set_and_get_roundtrip() {
        let cfg = DynamicConfig::new(Cfg { value: 1 });
        assert_eq!(cfg.get(), Cfg { value: 1 });
        cfg.set(Cfg { value: 2 });
        assert_eq!(cfg.get(), Cfg { value: 2 });
    }

    #[tokio::test]
    async fn subscribe_receives_change_notification() {
        let cfg = DynamicConfig::new(Cfg { value: 1 });
        let mut rx = cfg.subscribe();
        cfg.set(Cfg { value: 9 });
        rx.recv().await.expect("change notification");
        assert_eq!(cfg.get().value, 9);
    }

    #[tokio::test]
    async fn watch_files_reloads_on_change() {
        let mut file = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
        write!(file, r#"{{"value": 1}}"#).unwrap();
        let path = file.path().to_path_buf();

        let reload_path = path.clone();
        let cfg = DynamicConfig::<Cfg>::watch_files_debounced(
            vec![path.clone()],
            move || {
                ConfigLoader::new()
                    .merge_file(&reload_path)
                    .and_then(|l| l.build())
            },
            Duration::from_millis(10),
        )
        .expect("watch setup");
        assert_eq!(cfg.get().value, 1);

        let mut rx = cfg.subscribe();
        // Rewrite the file to trigger a reload.
        std::fs::write(&path, r#"{"value": 42}"#).unwrap();

        // Wait (with a generous timeout) for the watcher thread to notice.
        let result = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await;
        assert!(result.is_ok(), "expected a reload notification within 5s");
        assert_eq!(cfg.get().value, 42);
    }
}
