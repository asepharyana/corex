# mytheclipse

A personal collection of Rust abstractions for building resource-aware,
resilient, and well-instrumented applications without hand-rolling the same
plumbing every time — organized as a Cargo workspace, one focused crate per
concern.

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](crates/mytheclipse/LICENSE-MIT)

## Crates

| Crate | Description | Docs |
| :--- | :--- | :--- |
| [`mytheclipse`](crates/mytheclipse) | Resource-aware execution primitives (async I/O, compute, background queues), resiliency (retry, circuit breaker, timeout), traffic control (rate limiter, backpressure, concurrency limiter), lifecycle (graceful shutdown, cron), and observability (metrics, panic tracking). | [README](crates/mytheclipse/README.md) |
| [`mytheclipse-cache`](crates/mytheclipse-cache) | Unified multi-layer (L1/L2) cache abstraction: in-memory or Moka L1, Redis/Valkey L2, cache-aside read-through. | [README](crates/mytheclipse-cache/README.md) |
| [`mytheclipse-storage`](crates/mytheclipse-storage) | Unified storage & file system abstraction: one driver interface over local disk, S3/MinIO, and Google Cloud Storage, stream-based. | [README](crates/mytheclipse-storage/README.md) |
| [`mytheclipse-event`](crates/mytheclipse-event) | Unified events & message bus abstraction: in-memory pub/sub dispatcher plus RabbitMQ and NATS broker adapters behind one trait. | [README](crates/mytheclipse-event/README.md) |
| [`mytheclipse-config`](crates/mytheclipse-config) | Type-safe, dynamic configuration engine: load `.env`/YAML/JSON/TOML into typed structs, with hot-reload. | [README](crates/mytheclipse-config/README.md) |
| [`mytheclipse-crypto`](crates/mytheclipse-crypto) | Safe hashing (Argon2id), encryption (AES-256-GCM), and JWT tokens, with key rotation support. | [README](crates/mytheclipse-crypto/README.md) |

Every crate follows the same philosophy: **one small interface, pluggable
backends behind feature flags, and a working default that needs no external
service to build or test.** Distributed backends (Redis, S3, GCS, RabbitMQ,
NATS) are feature-gated and their integration tests are `#[ignore]`d unless
the corresponding environment variables point at a live service.

## Getting started

Each crate is published independently; add the ones you need:

```toml
[dependencies]
mytheclipse = { version = "1", features = ["full"] }
mytheclipse-cache = "0.1"
mytheclipse-storage = { version = "0.1", features = ["s3"] }
mytheclipse-event = { version = "0.1", features = ["nats"] }
mytheclipse-config = "0.1"
mytheclipse-crypto = "0.1"
```

See each crate's own README (linked above) for usage examples and the full
feature-flag list.

## Development

This is a Cargo workspace; run commands from the repository root:

```bash
cargo build --workspace --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-features -- -D warnings
cargo fmt --all --check
```

Or target a single crate with `-p <name>`, e.g. `cargo test -p mytheclipse-cache`.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](crates/mytheclipse/LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](crates/mytheclipse/LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
