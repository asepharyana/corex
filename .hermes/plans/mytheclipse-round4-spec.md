# Implementation Spec: Round 4

## Status: COMPLETE

## New Features

### 1. CircuitBreakerMetrics (circuit_breaker.rs)
- Added `CircuitSnapshot { state: CircuitState, failures: u64, successes: u64 }` struct
- Added `CircuitBreaker::snapshot() -> CircuitSnapshot` method (atomic load)
- Test: `snapshot_reflects_state_and_counts`

### 2. RetryStats (retry.rs)
- Added `RetryStats { attempts: u32, retries: u32, last_error: Option<String> }`
- Added `retry_with_stats()` returning `(Result, RetryStats)` (parallel to retry())
- Tests: 2 new

### 3. AsyncLifecycleManager (lifecycle.rs) — Round 3 carryover, verified
- Composes ShutdownManager + HealthRegistry + health loop
- Tests: 3

### 4. MetricsBridge (metrics_bridge.rs) — Round 3 carryover
- `MetricsBridge` emits MetricsCollector → tracing
- `MetricsHealthCheck` wraps collector as HealthCheck
- Tests: 2

## Fixes in round 4
- `HealthRegistry` wrapped in `Arc` in AsyncLifecycleManager (not Clone)
- Removed unused `span`/`Instrument` import in lifecycle.rs
- Fixed `op_ref` mutability in service_builder.rs
- Fixed `last_error` assertion (None on success) in retry test
- Fixed snapshot test assertions (successes not incremented in Closed state)

## Build Status
- cargo build --workspace --all-features: OK (2 pre-existing warnings in crypto/cli)
- cargo test --workspace --all-features: ALL PASS
- cargo clippy: 0 warnings on round-4 code (pre-existing in crypto/cli only)
- Committed + pushed
