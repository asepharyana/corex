//! # mytheclipse-tracing
//!
//! Pre-built tracing layers combining all mytheclipse primitives with
//! optional export backends (OTLP, Jaeger, Zipkin).

pub mod fmt;
pub mod otel;

pub use fmt::TracingLayer;
#[cfg(any(feature = "otel", feature = "jaeger", feature = "full"))]
pub use otel::OtelLayer;
