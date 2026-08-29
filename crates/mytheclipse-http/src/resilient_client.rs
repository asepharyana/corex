//! Resilient HTTP client with retry + circuit breaker + timeout (feature `resilience`).
//!
//! Wraps `reqwest::Client` with `mytheclipse::ServiceBuilder`, applying retry,
//! circuit-breaker, and timeout layers around every request.

use std::pin::Pin;
use std::time::Duration;

use reqwest::Client;
use reqwest::Method;
use reqwest::RequestBuilder;
use tracing::Instrument;

use mytheclipse::{CircuitBreaker, RunError, ServiceBuilder, ServiceConfig};

type HttpError = Box<dyn std::error::Error + Send + Sync>;

/// Configuration for [`ResilientHttpClient`].
#[derive(Clone)]
pub struct ResilientClientConfig {
    pub timeout: Duration,
    pub max_attempts: u32,
    pub rate_per_sec: f64,
    pub rate_burst: u64,
    pub circuit_breaker: Option<CircuitBreaker>,
}

impl Default for ResilientClientConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            max_attempts: 1,
            rate_per_sec: 0.0,
            rate_burst: 0,
            circuit_breaker: None,
        }
    }
}

/// A reqwest client that runs every request through a `ServiceBuilder`
/// pipeline (retry + circuit breaker + timeout).
pub struct ResilientHttpClient {
    inner: Client,
    config: ResilientClientConfig,
    builder: ServiceBuilder,
}

impl ResilientHttpClient {
    /// Creates a new resilient client from the given config.
    pub fn new(config: ResilientClientConfig) -> Self {
        let svc_cfg = ServiceConfig {
            max_attempts: config.max_attempts,
            timeout: config.timeout,
            rate_per_sec: config.rate_per_sec,
            rate_burst: config.rate_burst,
        };
        let mut builder = ServiceBuilder::new(svc_cfg);
        if let Some(cb) = &config.circuit_breaker {
            builder = builder.with_circuit_breaker(cb.clone());
        }
        Self {
            inner: Client::new(),
            config,
            builder,
        }
    }

    /// Returns the configured default timeout.
    pub fn timeout(&self) -> Duration {
        self.config.timeout
    }

    /// Returns a `RequestBuilder` for `method` + `url`.
    pub fn request(&self, method: Method, url: &str) -> RequestBuilder {
        self.inner.request(method, url)
    }

    /// Sends a pre-built `RequestBuilder` through the resiliency pipeline.
    /// Returns the response bytes on success.
    pub async fn send(
        &self,
        req: RequestBuilder,
    ) -> Result<Vec<u8>, RunError<HttpError>> {
        let span = tracing::info_span!("resilient_http_send");
        let op = move || {
            let req = req.try_clone().unwrap();
            let fut: Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, HttpError>> + Send>> =
                Box::pin(async move {
                    let resp = req.send().instrument(tracing::trace_span!("http_send")).await?;
                    let bytes = resp.bytes().await?;
                    Ok::<Vec<u8>, HttpError>(bytes.to_vec())
                });
            fut
        };
        self.builder
            .run(op)
            .instrument(span)
            .await
    }

    /// Convenience: GET `url`, returning response bytes.
    pub async fn get(&self, url: &str) -> Result<Vec<u8>, RunError<HttpError>> {
        self.send(self.inner.get(url)).await
    }

    /// Convenience: POST `url`, returning response bytes.
    pub async fn post(&self, url: &str, body: Vec<u8>) -> Result<Vec<u8>, RunError<HttpError>> {
        self.send(self.inner.post(url).body(body)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_with_default_config() {
        let client = ResilientHttpClient::new(ResilientClientConfig::default());
        assert_eq!(client.timeout(), Duration::from_secs(30));
    }

    #[test]
    fn config_default_values() {
        let c = ResilientClientConfig::default();
        assert_eq!(c.timeout, Duration::from_secs(30));
        assert_eq!(c.max_attempts, 1);
        assert!(c.circuit_breaker.is_none());
    }
}
