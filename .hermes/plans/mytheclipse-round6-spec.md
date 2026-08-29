# Implementation Spec: Round 6

## New Features

### 1. HealthCheckedPool (mytheclipse-core, observability+traffic)
File: `crates/mytheclipse/src/pool_health.rs`
- `HealthCheckedPool<T>` — wraps `SemaphorePool<T>`, integrates `HealthRegistry`
- `check_connection(&self) -> HealthStatus` — validates pooled resource
- auto-registers health check at construction
- gated feature observability+traffic

### 2. HkdfKeyDeriver (mytheclipse-crypto, derivation feature)
File: `crates/mytheclipse-crypto/src/hkdf.rs`
- `HkdfKeyDeriver` — HKDF-SHA256 (RFC 5869) from master secret
- `derive_key(&self, purpose: &str, output_len) -> Vec<u8>` — context-specific sub-key
- domain separation via purpose as info
- gated feature "derivation"

### 3. BackpressureEnqueue (mytheclipse-queue, in-memory)
File: `crates/mytheclipse-queue/src/backpressure.rs`
- `BackpressureEnforcer` — tracks in-flight count, enforces max
- `enqueue_or_nack(queue, topic, payload, max_inflight) -> Result<(), BackpressureError>`
- non-blocking: returns BackpressureError when at capacity

## Verification
- build + test + clippy + commit + push
