# Implementation Spec: Round 3

## New Features (4)

### 1. ConfigValidator (mytheclipse-config)
File: `crates/mytheclipse-config/src/validate.rs`
- `ConfigValidator` trait: `fn validate(&self) -> Result<(), ValidationError>`
- `ConfigValidatorExt` trait: blanket impl for `T: ConfigValidator`
- Built-in validators: `validate_url`, `validate_port`, `validate_non_empty`, `validate_range`, `collect_failures`
- `ValidationFailure { path, message }` + `ValidationError` type alias
- Feature gate: `validation` (default)
- Tests: 17 (unit + doctest)

### 2. AsyncLifecycleManager (mytheclipse-core)
File: `crates/mytheclipse/src/lifecycle.rs`
- `AsyncLifecycleManager` composing `ShutdownManager` + `HealthRegistry`
- Methods: `register_health_check`, `check_health`, `shutdown_signal`, `start_health_loop`, `await_shutdown`, `request_shutdown`
- Feature gate: `lifecycle`
- Tests: 38 total (3 new in lifecycle.rs)

### 3. MetricsBridge (mytheclipse-core)
File: `crates/mytheclipse/src/metrics_bridge.rs`
- `MetricsBridge` — emits MetricsCollector snapshot to tracing
- `MetricsHealthCheck` — wraps MetricsCollector as HealthCheck (unhealthy if error counters > 0)
- Feature gate: `observability`

### 4. ServiceBuilder RateLimiter API (mytheclipse-core)
File: `crates/mytheclipse/src/service_builder.rs`
- `with_rate_limiter` fluent builder (already existed)
- `check_pre` performs rate-limit pre-acquire before calling service
- Returns `RunError::RateLimited` when rate limiter exhausted

## Build Status
- cargo build --workspace --all-features: OK
- cargo test --workspace --all-features: all pass (77+17+18+16+6+5+...)
- cargo clippy: 0 warnings on new code (pre-existing warnings in crypto/base64/cli only)
- Committed + pushed

## Notes
- `Arc<HealthRegistry>` in AsyncLifecycleManager because HealthRegistry doesn't impl Clone
- Doctest marked `ignore` (async runtime not available in doctest context)
- Lint checker false-positives on `async fn` (edition 2015 phantom) but actual cargo build/tests pass
