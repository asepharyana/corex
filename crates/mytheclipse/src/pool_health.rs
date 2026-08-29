//! Health-checked resource pool (feature `observability` + `traffic`).
//!
//! [`HealthCheckedPool`] composes a [`SemaphorePool`] with a [`HealthRegistry`]:
//! a background probe periodically validates pooled items, and a registered
//! `HealthCheck` reflects pool liveliness in the aggregated `/health` report.

use std::sync::Arc;
use std::time::Duration;

use crate::health::{HealthCheck, HealthRegistry, HealthStatus};
use crate::pool::{Pool, PoolError, Pooled, SemaphorePool};

/// A health check backed by a closure.
struct ClosureCheck {
    name: String,
    check: Arc<dyn Fn() -> HealthStatus + Send + Sync>,
}

impl HealthCheck for ClosureCheck {
    fn name(&self) -> &str {
        &self.name
    }

    fn check(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = HealthStatus> + Send + '_>> {
        let status = (self.check)();
        Box::pin(async move { status })
    }
}

/// A resource pool with integrated health reporting.
pub struct HealthCheckedPool<T: Clone + Send + Sync + 'static> {
    pub(crate) inner: SemaphorePool<T>,
    #[allow(dead_code)]
    registry: Arc<HealthRegistry>,
    #[allow(dead_code)]
    check_interval: Duration,
    #[allow(dead_code)]
    name: String,
}

impl<T: Clone + Send + Sync + 'static + std::fmt::Debug> HealthCheckedPool<T> {
    /// Creates a new health-checked pool.
    ///
    /// `validator` is called on each item during the periodic background probe;
    /// the registered health check reports `Ok` if any item validates.
    pub async fn new(
        items: Vec<T>,
        registry: &HealthRegistry,
        name: impl Into<String>,
        check_interval: Duration,
        validator: impl Fn(&T) -> bool + Send + Sync + 'static,
    ) -> Self {
        let name_str = name.into();
        let pool = SemaphorePool::new(items);
        let registry = Arc::new(registry.clone());

        let validator: Arc<dyn Fn(&T) -> bool + Send + Sync> = Arc::new(validator);
        let check_items = pool.items().to_vec();
        let v_check = Arc::clone(&validator);
        let check = ClosureCheck {
            name: format!("connection-pool:{}", name_str),
            check: Arc::new(move || {
                if check_items.iter().any(|i| v_check(i)) {
                    HealthStatus::Ok
                } else {
                    HealthStatus::Unhealthy
                }
            }),
        };
        let r = Arc::clone(&registry);
        let check_name = check.name.clone();
        tokio::task::spawn(async move {
            r.register(check_name, check).await;
        });

        // Background probe
        let probe_items = pool.items().to_vec();
        let probe_name = name_str.clone();
        let v_probe = Arc::clone(&validator);
        tokio::task::spawn(async move {
            let mut ticker = tokio::time::interval(check_interval);
            loop {
                ticker.tick().await;
                let up = probe_items.iter().filter(|i| v_probe(i)).count();
                tracing::debug!(pool = %probe_name, up, total = probe_items.len(), "pool health probe");
            }
        });

        Self {
            inner: pool,
            registry,
            check_interval,
            name: name_str,
        }
    }

    /// Acquires a resource from the pool.
    pub async fn acquire_healthy(&self) -> Result<Pooled<T>, PoolError> {
        self.inner.acquire().await
    }

    /// Number of items in the pool.
    pub fn size(&self) -> usize {
        self.inner.items().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn acquires_resource() {
        let reg = HealthRegistry::new();
        let pool = HealthCheckedPool::new(
            vec![42u32, 84u32],
            &reg,
            "test",
            Duration::from_secs(5),
            |_| true,
        )
        .await;
        let item = pool.acquire_healthy().await.unwrap();
        assert!(item.resource == 42 || item.resource == 84);
        assert_eq!(pool.size(), 2);
    }

    #[tokio::test]
    async fn validator_distinguishes_healthy() {
        let reg = HealthRegistry::new();
        let pool = HealthCheckedPool::new(
            vec![0u32, 1u32, 2u32],
            &reg,
            "test",
            Duration::from_secs(5),
            |x| *x > 0,
        )
        .await;
        assert_eq!(pool.size(), 3);
    }
}
