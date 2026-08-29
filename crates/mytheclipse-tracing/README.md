# mytheclipse-tracing

Pre-built tracing infrastructure for mytheclipse applications, wrapping
[`tracing-subscriber`] with sensible defaults and optional OTLP/Jaeger/Zipkin
export.

## Features

| Feature | Default | Description |
| :--- | :---: | :--- |
| `env` | yes | `tracing-subscriber` with `EnvFilter` support. |
| `otel` | no | OpenTelemetry OTLP gRPC exporter. |
| `jaeger` | no | Jaeger thrift over `tracing-flame`. |
| `zipkin` | no | Zipkin exporter (stub — extend as needed). |
| `full` | — | Enables `otel` + `jaeger`. |

## Usage

```toml
[dependencies]
mytheclipse-tracing = "0.2"
```

### Basic subscriber

```rust
use mytheclipse_tracing::TracingLayer;

fn main() {
    TracingLayer::install();
    tracing::info!("hello, world!");
}
```

### With OTLP export

```toml
[dependencies]
mytheclipse-tracing = { version = "0.2", features = ["otel"] }
```

```rust
use mytheclipse_tracing::TracingLayer;

fn main() {
    TracingLayer::install();
    // OTLP exporter defaults to http://localhost:4317
    tracing::info!("span data sent to OTLP collector");
}
```
