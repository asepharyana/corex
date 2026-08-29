# Implementation Spec: New Features for mytheclipse

## Status: COMPLETE

## Summary

Added 4 new crates and enhancements to existing crates to expand mytheclipse's
abstraction layer coverage. All code compiles with `cargo build --workspace --all-features`,
all tests pass, and clippy is clean.

## New Crates

1. **mytheclipse-queue** (`crates/mytheclipse-queue/`)
   - `Queue` trait: enqueue, dequeue, ack, nack, dlq_move, len
   - `Job` / `JobId` types with payload + metadata
   - `WorkerPool` with configurable concurrency, retry/backoff, dead-letter queue
   - `JobHandler` trait for processing jobs
   - Backend: in-memory (default), Redis (feature `redis`), NATS (feature `nats`), PostgreSQL (feature `postgres`)

2. **mytheclipse-tracing** (`crates/mytheclipse-tracing/`)
   - `TracingLayer` with env-filter support and subscriber builder
   - `OtelLayer` for OTLP/Jaeger export (feature `otel`, `jaeger`, `full`)
   - Features: `env` (default), `otel`, `jaeger`, `full`

3. **mytheclipse-http** (`crates/mytheclipse-http/`)
   - `HttpClient` wrapping reqwest with timeout + tracing instrumentation
   - `HttpServer` (axum) with health endpoint + graceful shutdown
   - Features: `client` (default), `server-axum`, `server-hyper`

4. **mytheclipse-cli** (`crates/mytheclipse-cli/`)
   - `CliApp` / `CliBuilder` with clap derive
   - Subcommands: `serve`, `worker`, `migrate`, `health`, `version`
   - Feature: `clap-derive` (default)

## Enhancements to Existing Crates

### mytheclipse (core)
- `pool.rs`: `SemaphorePool<T>` with `Pool` trait, `Pooled<T>` RAII permit
- `health.rs`: `HealthRegistry`, `HealthCheck` trait, `HealthStatus` enum
- `leader.rs`: `LeaderElection` trait, `InProcLeaderElection` impl
- Features: gated under `traffic` (pool) and `lifecycle` (health, leader)

### mytheclipse-cache
- `auto_refresh.rs`: `AutoRefreshCache` — background refresh on cache miss
- `metrics.rs`: `CacheMetrics` + `CacheSnapshot` with hit/miss/eviction tracking
- Added `tokio` optional dep (used by cache-aside + auto-refresh)

### mytheclipse-config
- `schema.rs`: `ConfigSchema` + `PropertySchema` for JSON Schema generation
- Feature `schema` gated

### mytheclipse-storage
- `multipart.rs`: `MultipartUploadDriver` trait + `MultipartUpload` handler
- Feature `multipart` (default) gated

### mytheclipse-crypto
- `paseto.rs`: `PasetoSigner` + `PasetoClaims` for PASETO v4.local tokens
- Features `paseto` and `rate-limit` added

## Verification
- `cargo build --workspace --all-features` ✓
- `cargo test --workspace --all-features` ✓ (all pass, 1 ignored doctest)
- `cargo clippy --workspace --all-features` ✓ (no warnings)
