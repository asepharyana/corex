//! Resource pool for sharing bounded resources across async tasks.
//!
//! Provides a `Pool` trait and a built-in `SemaphorePool<T>` implementation
//! that distributes items drawn from a `Vec<T>` under a counting semaphore.

use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;

/// Errors returned by pool operations.
#[derive(Debug, thiserror::Error)]
pub enum PoolError {
    #[error("pool exhausted")]
    Exhausted,
    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}

/// A pooled resource that releases the permit when dropped.
pub struct Pooled<T> {
    pub resource: T,
    _permit: OwnedSemaphorePermit,
}

/// A pool of resources with bounded concurrency.
#[async_trait]
pub trait Pool<T>: Send + Sync {
    /// Acquires a resource from the pool, waiting if none are available.
    async fn acquire(&self) -> Result<Pooled<T>, PoolError>;
}

/// In-memory pool backed by a semaphore + `Vec`.
#[derive(Clone)]
pub struct SemaphorePool<T: Clone> {
    semaphore: Arc<Semaphore>,
    items: Arc<Vec<T>>,
}

impl<T: Clone> SemaphorePool<T> {
    /// Creates a new pool from a vector of items.
    pub fn new(items: Vec<T>) -> Self {
        let permits = items.len().max(1);
        Self {
            semaphore: Arc::new(Semaphore::new(permits)),
            items: Arc::new(items),
        }
    }
}

#[async_trait]
impl<T: Clone + Send + Sync + 'static> Pool<T> for SemaphorePool<T> {
    async fn acquire(&self) -> Result<Pooled<T>, PoolError> {
        let permit = self.semaphore.clone().acquire_owned().await
            .map_err(|_| PoolError::Exhausted)?;
        let idx = rand::random::<usize>() % self.items.len();
        Ok(Pooled {
            resource: self.items[idx].clone(),
            _permit: permit,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pool_returns_item() {
        let pool = SemaphorePool::new(vec![42u32, 84u32]);
        let item = pool.acquire().await.unwrap();
        assert!(item.resource == 42 || item.resource == 84);
    }
}
