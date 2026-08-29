//! Prometheus metrics HTTP endpoint (feature `metrics-http`).
//!
//! [`metrics_routes`] returns an [`axum::Router`] that serves the
//! [`MetricsCollector`]'s Prometheus text exposition format at `/metrics`.

use axum::routing::get;
use axum::Router;
use std::sync::Arc;

use mytheclipse::MetricsCollector;

/// Builds a small axum router exposing `/metrics` (Prometheus text) and
/// `/` (a one-line description).
pub fn metrics_routes(collector: MetricsCollector) -> Router {
    let collector = Arc::new(collector);
    Router::new()
        .route("/", get(|| async { "mytheclipse metrics" }))
        .route("/metrics", get(metrics_handler))
        .with_state(collector)
}

/// Axum handler serving the Prometheus text format.
async fn metrics_handler(
    axum::extract::State(collector): axum::extract::State<Arc<MetricsCollector>>,
) -> axum::response::Response {
    let body = collector.export_prometheus();
    axum::response::Response::builder()
        .status(200)
        .header("content-type", "text/plain; version=0.0.4")
        .body(axum::body::Body::from(body))
        .unwrap_or_else(|_| axum::response::Response::new(axum::body::Body::from("internal error")))
}

#[cfg(test)]
mod tests {
    use super::*;

    use tower::util::ServiceExt;

    #[tokio::test]
    async fn metrics_routes_serves_prometheus() {
        let collector = MetricsCollector::new();
        collector.inc_counter("test_reqs", 42);
        let app = metrics_routes(collector);

        let request = axum::extract::Request::get("/metrics")
            .body(axum::body::Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), 200);
    }
}
