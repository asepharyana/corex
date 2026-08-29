//! Health check registry for reporting component status.

use std::fmt;
use std::sync::Arc;

use tokio::sync::RwLock;

/// Status levels returned by health checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    Ok,
    Degraded,
    Unhealthy,
}

impl fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HealthStatus::Ok => write!(f, "ok"),
            HealthStatus::Degraded => write!(f, "degraded"),
            HealthStatus::Unhealthy => write!(f, "unhealthy"),
        }
    }
}

/// A single health check.
pub trait HealthCheck: Send + Sync {
    fn name(&self) -> &str;
    fn check(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = HealthStatus> + Send + '_>>;
}

/// A registered health check with its name and trait object.
struct RegisteredCheck {
    name: String,
    check: Arc<dyn HealthCheck>,
}

/// Registry of health checks for aggregated /health reporting.
#[derive(Default, Clone)]
pub struct HealthRegistry {
    checks: Arc<RwLock<Vec<RegisteredCheck>>>,
}

impl HealthRegistry {
    pub fn new() -> Self {
        Self {
            checks: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn register(&self, name: impl Into<String>, check: impl HealthCheck + 'static) {
        let mut checks = self.checks.write().await;
        checks.push(RegisteredCheck {
            name: name.into(),
            check: Arc::new(check),
        });
    }

    /// Runs all checks and returns aggregated results.
    pub async fn check_all(&self) -> Vec<(String, HealthStatus)> {
        let checks = self.checks.read().await;
        let mut results = Vec::new();
        for registered in checks.iter() {
            let status = registered.check.check().await;
            results.push((registered.name.clone(), status));
        }
        results
    }
}
