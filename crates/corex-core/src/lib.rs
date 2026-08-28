//! Core allocation logic, global context initialization, and execution
//! primitives for [corex](https://docs.rs/corex).
//!
//! This crate is not typically consumed directly; applications should
//! depend on the `corex` facade crate instead, which re-exports the pieces
//! of this crate behind feature flags.

pub mod context;
#[cfg(feature = "compute")]
pub mod error;

#[cfg(feature = "io")]
pub mod io;

#[cfg(feature = "compute")]
pub mod compute;

#[cfg(feature = "bg")]
pub mod bg;
