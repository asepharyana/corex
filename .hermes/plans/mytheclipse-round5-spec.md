# Implementation Spec: Round 5

## Status: COMPLETE

## New Features

### 1. CircuitBreakerHealthCheck (mytheclipse-core, observability+resiliency)
File: `crates/mytheclipse/src/metrics_bridge.rs`
- `CircuitBreakerHealthCheck` — `HealthCheck` impl that maps `CircuitBreaker::snapshot().state` to HealthStatus:
  - Open → Unhealthy
  - HalfOpen → Degraded
  - Closed → Ok
- Gated `#[cfg(feature = "resiliency")]`; re-exported when both observability+resiliency enabled
- Feature interaction: `observability` now implies `lifecycle` (needed for `crate::health::{HealthCheck, HealthStatus}`)

### 2. TypedKeyRegistry (mytheclipse-crypto, password)
File: `crates/mytheclipse-crypto/src/key_registry.rs`
- `TypedKeyRegistry<K, V>` — registry keyed by string ID, wraps KeyRing for current/previous rotation
- `key_for(&self, id: &str) -> Option<&K>` typed lookup
- `rotate_with_id(&mut self, id, key)` + `revoke(id)`
- Default impl uses String keys (v4 signers)

### 3. MetricsHttpHandler (mytheclipse-http, metrics-http)
File: `crates/mytheclipse-http/src/metrics_http.rs`
- new feature `metrics-http` (axum + tower + mytheclipse/observability)
- `metrics_routes(collector) -> Router` serving `/metrics` (Prometheus text via `export_prometheus`) + `/`
- `tower` dep added (util feature)
- 1 test via ServiceExt::oneshot

## Verification
- `cargo build --workspace --all-features` → exit 0
- `cargo test --workspace --all-features` → all pass (160+ tests)
- `cargo clippy --workspace --all-features` → no new warnings
