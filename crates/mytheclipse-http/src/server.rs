//! HTTP server abstraction (axum backend, feature-gated).

#[cfg(feature = "server-axum")]
mod axum_server;

#[cfg(feature = "server-axum")]
pub use axum_server::HttpServer;
