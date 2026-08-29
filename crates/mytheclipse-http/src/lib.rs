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

#[cfg(feature = "client")]
pub mod client;

#[cfg(feature = "client")]
pub use client::HttpClient;

#[cfg(feature = "server-axum")]
pub mod server;
