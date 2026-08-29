//! Formatted tracing subscriber layer with env filtering.

use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

/// A pre-configured tracing subscriber builder.
#[derive(Clone)]
pub struct TracingLayer;

impl TracingLayer {
    /// Installs the global default subscriber with formatting and env filter.
    ///
    /// Reads `RUST_LOG` from the environment, defaulting to `mytheclipse=info`.
    pub fn install() {
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("mytheclipse=info"));
        let _ = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .try_init();
    }

    /// Returns a formatted layer for manual composition.
    pub fn layer() -> impl tracing_subscriber::layer::Layer<tracing_subscriber::Registry> {
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("mytheclipse=info"));
        fmt::layer().with_filter(filter)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn layer_builds() {
        let _ = super::TracingLayer::layer();
    }
}
