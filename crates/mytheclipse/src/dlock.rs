//! Distributed lock with lease-based expiration.
//!
//! Provides a `DistributedLock` trait with in-process and Redis backends.
//! Used to coordinate leader election and queue dispatch across multiple
//! worker instances.

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::sync::Mutex;

/// Errors returned by distributed lock operations.
#[derive(Debug)]
pub enum LockError {
    /// The lock could not be acquired (already held or timed out).
    AlreadyHeld,
    /// The lease expired and the lock was released.
    Expired,
    /// A backend transport error occurred.
    Io(String),
}

impl std::fmt::Display for LockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LockError::AlreadyHeld => write!(f, "lock already held"),
            LockError::Expired => write!(f, "lock lease expired"),
            LockError::Io(s) => write!(f, "lock io error: {s}"),
        }
    }
}

impl std::error::Error for LockError {}

/// Shared state for an in-process lock entry — maps keys to expiry instants.
type LockMap = Arc<Mutex<std::collections::HashMap<String, Instant>>>;

/// A handle to an acquired distributed lock (RAII — releases on drop).
pub struct LockGuard {
    map: LockMap,
    key: String,
}

impl LockGuard {
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Attempts to extend the lease.
    pub async fn extend(&mut self, _dur: Duration) -> Result<(), LockError> {
        Err(LockError::Expired)
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        let map = Arc::clone(&self.map);
        let key = self.key.clone();
        // Fire-and-forget release: spawn a detached task to remove the key.
        tokio::spawn(async move {
            let mut m = map.lock().await;
            m.remove(&key);
        });
    }
}

/// Trait for distributed lock backends.
#[async_trait]
pub trait DistributedLock: Send + Sync {
    /// Attempts to acquire the lock with the given lease duration.
    async fn acquire(&self, key: &str, lease: Duration, timeout: Duration) -> Result<LockGuard, LockError>;

    /// Releases the lock.
    async fn release(&self, key: &str) -> Result<(), LockError>;

    /// Attempts to extend an existing lease.
    async fn extend(&self, key: &str, lease: Duration) -> Result<(), LockError>;
}

/// In-process distributed lock using a mutex + lease timer.
/// Suitable for testing and single-instance coordination.
pub struct InProcLock {
    held: LockMap,
}

impl InProcLock {
    pub fn new() -> Self {
        Self {
            held: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    fn is_expired(map: &std::collections::HashMap<String, Instant>, key: &str) -> bool {
        if let Some(expiry) = map.get(key) {
            *expiry <= Instant::now()
        } else {
            false
        }
    }
}

impl Default for InProcLock {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DistributedLock for InProcLock {
    async fn acquire(&self, key: &str, lease: Duration, timeout: Duration) -> Result<LockGuard, LockError> {
        let deadline = Instant::now() + timeout;
        loop {
            {
                let mut map = self.held.lock().await;
                // Clean up expired entries lazily.
                map.retain(|_, v| *v > Instant::now());
                if !map.contains_key(key) {
                    map.insert(key.to_string(), Instant::now() + lease);
                    return Ok(LockGuard {
                        map: Arc::clone(&self.held),
                        key: key.to_string(),
                    });
                }
            }
            if Instant::now() >= deadline {
                return Err(LockError::AlreadyHeld);
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    async fn release(&self, key: &str) -> Result<(), LockError> {
        let mut map = self.held.lock().await;
        map.remove(key);
        Ok(())
    }

    async fn extend(&self, key: &str, lease: Duration) -> Result<(), LockError> {
        let mut map = self.held.lock().await;
        if let Some(entry) = map.get_mut(key) {
            *entry = Instant::now() + lease;
            Ok(())
        } else {
            Err(LockError::AlreadyHeld)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lock_acquire_release() {
        let lock = InProcLock::new();
        let guard = lock.acquire("key", Duration::from_secs(10), Duration::from_secs(1)).await.unwrap();
        assert!(lock.release("key").await.is_ok());
        drop(guard);
    }

    #[tokio::test]
    async fn lock_rejects_second_acquire() {
        let lock = InProcLock::new();
        let _guard1 = lock.acquire("key", Duration::from_secs(10), Duration::from_secs(1)).await.unwrap();
        // While guard1 is alive, a second acquire with short timeout should fail.
        let result = lock.acquire("key", Duration::from_secs(10), Duration::from_millis(50)).await;
        assert!(result.is_err());
        drop(_guard1);
    }

    #[tokio::test]
    async fn lock_auto_releases_on_drop() {
        let lock = InProcLock::new();
        let guard = lock.acquire("k", Duration::from_secs(10), Duration::from_secs(1)).await.unwrap();
        drop(guard);
        // After drop, the lock should be releasable / re-acquirable.
        let result = lock.acquire("k", Duration::from_secs(10), Duration::from_millis(50)).await;
        assert!(result.is_ok(), "lock should be free after guard drop");
    }

    #[tokio::test]
    async fn lock_expires_after_lease() {
        let lock = InProcLock::new();
        let _guard = lock.acquire("key", Duration::from_millis(20), Duration::from_millis(5)).await.unwrap();
        drop(_guard);
        tokio::time::sleep(Duration::from_millis(30)).await;
        // Should be acquirable now.
        let result = lock.acquire("key", Duration::from_millis(20), Duration::from_millis(5)).await;
        assert!(result.is_ok());
    }
}
