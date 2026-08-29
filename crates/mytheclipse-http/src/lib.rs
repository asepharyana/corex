//! # mytheclipse-http
//!
//! HTTP client/server abstraction with built-in resilience primitives.
//!
//! ## Quick Start
//!
//! ```toml
//! [dependencies]
//! mytheclipse-http = { version = "0.2", features = ["client"] }
//! ```

#[cfg(feature = "resilience")]
pub mod resilient_client;
#[cfg(feature = "resilience")]
pub use resilient_client::{ResilientClientConfig, ResilientHttpClient};

#[cfg(feature = "client")]
pub mod client;

#[cfg(feature = "client")]
pub use client::HttpClient;

#[cfg(feature = "server-axum")]
pub mod server;

#[cfg(feature = "metrics-http")]
pub mod metrics_http;

#[cfg(feature = "metrics-http")]
pub use metrics_http::metrics_routes;
