//! # mytheclipse-cli
//!
//! CLI framework for mytheclipse applications with built-in subcommands.
//!
//! ## Quick Start
//!
//! ```toml
//! [dependencies]
//! mytheclipse-cli = "0.2"
//! ```

#[cfg(feature = "clap-derive")]
pub mod builder;

#[cfg(feature = "clap-derive")]
pub use builder::{CliApp, CliBuilder, Subcommand};
