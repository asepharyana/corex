//! OpenTelemetry / Jaeger export (feature-gated).

#[cfg(any(feature = "otel", feature = "jaeger", feature = "full"))]
pub mod otel_layer {
    /// OpenTelemetry exporter layer (OTLP over gRPC).
    ///
    /// This is a lightweight stub that provides the type and builder pattern;
    /// real OTLP setup requires the `opentelemetry` + `tracing-opentelemetry`
    /// crates and an OTLP collector endpoint.
    #[derive(Clone)]
    pub struct OtelLayer {
        endpoint: String,
    }

    impl OtelLayer {
        /// Creates a new OTLP exporter layer.
        pub fn new(endpoint: impl Into<String>) -> Self {
            Self {
                endpoint: endpoint.into(),
            }
        }

        /// Returns the configured OTLP endpoint.
        pub fn endpoint(&self) -> &str {
            &self.endpoint
        }
    }
}

#[cfg(any(feature = "otel", feature = "jaeger", feature = "full"))]
pub use otel_layer::OtelLayer;
