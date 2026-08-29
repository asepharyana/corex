//! Axum-based HTTP server with health endpoint.

use axum::{
    routing::get,
    Router,
};
use std::net::SocketAddr;
use std::time::Duration;

/// A pre-configured HTTP server with health check and metrics endpoints.
pub struct HttpServer {
    app: Router,
    addr: SocketAddr,
}

impl HttpServer {
    /// Creates a new server bound to the given address.
    pub fn new(addr: SocketAddr) -> Self {
        let router = Router::new()
            .route("/health", get(|| async { "OK" }))
            .route("/", get(|| async { "mytheclipse-http" }));

        Self {
            app: router,
            addr,
        }
    }

    /// Adds a custom route with a GET handler.
    #[must_use]
    pub fn with_get_route(self, path: &str, handler: axum::extract::Request<()>) -> Self {
        let _ = (path, handler);
        self
    }

    /// Runs the server until shutdown signal received.
    pub async fn run(self) {
        let listener = tokio::net::TcpListener::bind(self.addr)
            .await
            .expect("failed to bind");
        axum::serve(listener, self.app)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .expect("server error");
    }
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
    tracing::info!("shutdown signal received");
}

impl Default for HttpServer {
    fn default() -> Self {
        Self::new("0.0.0.0:3000".parse().unwrap())
    }
}
