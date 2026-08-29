//! # mytheclipse-config
//!
//! A type-safe, dynamic configuration engine: load environment variables,
//! `.env` files, YAML, JSON, or TOML directly into a typed Rust struct, with
//! optional runtime hot-reload.
//!
//! - [`ConfigLoader`] merges any number of file/env sources into a single
//!   typed value (files first, environment variables override).
//! - [`DynamicConfig`] wraps a loaded value behind an `RwLock`, optionally
//!   watching its source files and swapping in a freshly reloaded value when
//!   they change, broadcasting the change to subscribers.
//!
//! ## Example
//!
//! ```no_run
//! use serde::Deserialize;
//! use mytheclipse_config::ConfigLoader;
//!
//! #[derive(Debug, Deserialize, Clone)]
//! struct AppConfig {
//!     port: u16,
//!     database: DatabaseConfig,
//! }
//! #[derive(Debug, Deserialize, Clone)]
//! struct DatabaseConfig {
//!     url: String,
//! }
//!
//! let config: AppConfig = ConfigLoader::new()
//!     .merge_file("config.yaml".as_ref())
//!     .unwrap()
//!     .merge_env("APP")
//!     .build()
//!     .unwrap();
//! ```

pub mod error;
pub mod loader;

#[cfg(feature = "hot-reload")]
pub mod dynamic;

#[cfg(feature = "schema")]
pub mod schema;

pub use error::ConfigError;
pub use loader::ConfigLoader;

#[cfg(feature = "hot-reload")]
pub use dynamic::DynamicConfig;

/// Marker trait for types loadable by [`ConfigLoader`].
///
/// Blanket-implemented for any `Deserialize + Send + Sync + 'static`.
pub trait Config: for<'de> serde::Deserialize<'de> + Send + Sync + 'static {}
impl<T> Config for T where T: for<'de> serde::Deserialize<'de> + Send + Sync + 'static {}
