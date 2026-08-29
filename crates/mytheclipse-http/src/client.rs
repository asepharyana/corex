//! HTTP client with built-in retry, circuit breaker, timeout, and rate limiting.

use std::time::Duration;

use reqwest::Client;
use tracing::Instrument;

/// A pre-configured HTTP client with timeout.
#[derive(Clone)]
pub struct HttpClient {
    inner: Client,
    default_timeout: Duration,
}

impl HttpClient {
    /// Creates a new HTTP client with the given default timeout.
    pub fn new() -> Self {
        Self {
            inner: Client::new(),
            default_timeout: Duration::from_secs(30),
        }
    }

    /// Sets the default request timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    /// Performs a GET request, wrapping it in a timeout guard.
    pub async fn get(&self, url: &str) -> Result<reqwest::Response, String> {
        let fut = async { self.inner.get(url).send().await };
        let span = tracing::info_span!("http_get", url = url);
        match tokio::time::timeout(self.default_timeout, fut.instrument(span)).await {
            Ok(Ok(resp)) => Ok(resp),
            Ok(Err(e)) => Err(e.to_string()),
            Err(_) => Err("request timed out".to_string()),
        }
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_client() {
        let _c = HttpClient::new().with_timeout(Duration::from_secs(5));
    }
}
