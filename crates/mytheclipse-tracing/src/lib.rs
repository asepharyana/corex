//! # mytheclipse-tracing
//!
//! Pre-built tracing layers combining all mytheclipse primitives with
//! optional export backends (OTLP, Jaeger, Zipkin).

#[cfg(any(
    feature = "env",
    feature = "otel",
    feature = "jaeger",
    feature = "zipkin"
))]
pub mod fmt;
pub mod otel;

#[cfg(any(
    feature = "env",
    feature = "otel",
    feature = "jaeger",
    feature = "zipkin"
))]
pub use fmt::TracingLayer;
#[cfg(any(feature = "otel", feature = "jaeger", feature = "full"))]
pub use otel::OtelLayer;
