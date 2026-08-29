# [1.18.0](https://github.com/asepharyana/mytheclipse/compare/v1.17.0...v1.18.0) (2026-08-29)

### Documentation

* add `high_level` example wiring round-7-15 abstractions (RuntimeConfig, parallel_map, RetryExt, ShutdownGuard, AutoReconnectPool, AutoMetricsServiceBuilder)

# [1.17.0](https://github.com/asepharyana/mytheclipse/compare/v1.16.0...v1.17.0) (2026-08-29)

### Features

* **parallel_map:** add `parallel_for_each` streaming bounded fan-out (no full input materialization)

# [1.16.0](https://github.com/asepharyana/mytheclipse/compare/v1.15.0...v1.16.0) (2026-08-29)

### Features

* **parallel_map:** add `parallel_map` / `parallel_map_unordered` bounded fan-out with AggregateError

# [1.15.0](https://github.com/asepharyana/mytheclipse/compare/v1.14.0...v1.15.0) (2026-08-29)

### Features

* **aggregate_error:** add `AggregateError` for parallel/fan-out error aggregation

# [1.14.0](https://github.com/asepharyana/mytheclipse/compare/v1.13.0...v1.14.0) (2026-08-29)

### Features

* **pool:** add `AutoReconnectPool` + `Reconnectable` self-healing resource pool

# [1.13.0](https://github.com/asepharyana/mytheclipse/compare/v1.12.0...v1.13.0) (2026-08-29)

### Features

* **runtime_auto:** add `RuntimeConfig` auto thread/core allocation + `ShutdownGuard` RAII

# [1.12.0](https://github.com/asepharyana/mytheclipse/compare/v1.11.0...v1.12.0) (2026-08-29)

### Features

* **auto_metrics:** add `AutoMetricsServiceBuilder` (metrics + [mytheclipse-queue](crates/mytheclipse-queue) `RateLimitedWorkerPool`)

# [1.11.0](https://github.com/asepharyana/mytheclipse/compare/v1.10.0...v1.11.0) (2026-08-29)

### Features

* **retry_ext:** add ergonomic `RetryExt` Future extension — also covers ResilientHttpClient from [mytheclipse-http](crates/mytheclipse-http)

# [1.10.0](https://github.com/asepharyana/mytheclipse/compare/v1.9.0...v1.10.0) (2026-08-29)

### Features

* **http:** add `ResilientHttpClient` ([mytheclipse-http](crates/mytheclipse-http)) with retry + circuit breaker + timeout
* **middleware:** add composable `MiddlewarePipeline`
* **bg_join:** add `BgJoiner` graceful task joiner

# [1.9.0](https://github.com/asepharyana/mytheclipse/compare/v1.8.0...v1.9.0) (2026-08-28)

### Features

* **bg_join:** add `BgJoiner` graceful task joiner
* **middleware:** add composable `MiddlewarePipeline`
* **queue:** add `RateLimitedQueue` ([mytheclipse-queue](crates/mytheclipse-queue))

# [1.8.0](https://github.com/asepharyana/mytheclipse/compare/v1.7.0...v1.8.0) (2026-08-28)

### Features

* **pool_health:** add `HealthCheckedPool`
* **crypto:** add `HkdfKeyDeriver` ([mytheclipse-crypto](crates/mytheclipse-crypto))
* **queue:** add `BackpressureEnforcer` / `enqueue_with_backpressure`

# [1.7.0](https://github.com/asepharyana/mytheclipse/compare/v1.6.0...v1.7.0) (2026-08-28)

### Features

* **batch:** add `BatchProcessor` & `BatchJobHandler`
* **health:** add `CircuitBreakerHealthCheck` / `MetricsHealthCheck`
* **crypto:** add `TypedKeyRegistry`
* **http:** add `MetricsHttpHandler`

# [1.6.0](https://github.com/asepharyana/mytheclipse/compare/v1.5.0...v1.6.0) (2026-08-28)

### Features

* **circuit_breaker:** add snapshot metrics
* **retry:** add `RetryStats`
* lifecycle fixes

# [1.5.0](https://github.com/asepharyana/mytheclipse/compare/v1.4.1...v1.5.0) (2026-08-28)

### Features

* **validation:** add `ConfigValidator`
* **lifecycle:** add `AsyncLifecycleManager`
* **metrics:** add `MetricsBridge`
* **ratelimit:** rate limiter pre-acquire

# [1.4.0](https://github.com/asepharyana/mytheclipse/compare/v1.3.5...v1.4.0) (2026-08-28)

### Features

* add 4 new crates: [mytheclipse-queue](crates/mytheclipse-queue), [mytheclipse-tracing](crates/mytheclipse-tracing), [mytheclipse-http](crates/mytheclipse-http), [mytheclipse-cli](crates/mytheclipse-cli)

# [1.3.0](https://github.com/asepharyana/mytheclipse/compare/v1.2.0...v1.3.0) (2026-08-28)

### Features

* add [mytheclipse-storage](crates/mytheclipse-storage) crate for unified storage abstraction

# [1.2.0](https://github.com/asepharyana/mytheclipse/compare/v1.1.0...v1.2.0) (2026-08-28)

### Features

* add panic tracking and logging with `PanicTracker`

# [1.1.0](https://github.com/asepharyana/mytheclipse/compare/v1.0.0...v1.1.0) (2026-08-28)


### Features

* add panic tracking and isolation with PanicTracker ([8dec413](https://github.com/asepharyana/mytheclipse/commit/8dec4130057420320500d7a9265ccb3cbfd0e030))

# [1.0.0](https://github.com/asepharyana/mytheclipse/compare/v0.2.4...v1.0.0) (2026-08-28)


* feat!: rename CorexError to MytheclipseError ([7cdf525](https://github.com/asepharyana/mytheclipse/commit/7cdf52544c611b05c6365595a8ae713672706b67))


### BREAKING CHANGES

* CorexError is renamed to MytheclipseError.

## [0.2.4](https://github.com/asepharyana/corex/compare/v0.2.3...v0.2.4) (2026-08-28)


### Bug Fixes

* **release:** add actions: write permission for workflow dispatch ([5684938](https://github.com/asepharyana/corex/commit/5684938b4cd5a133a57ee835426035a804edcb38))

## [0.2.3](https://github.com/asepharyana/corex/compare/v0.2.2...v0.2.3) (2026-08-28)


### Bug Fixes

* **release:** hardcode repo in successCmd gh api ([d3c35ec](https://github.com/asepharyana/corex/commit/d3c35eccd0e3e1da21bcfed50ce2ba1f82d03cc5))

## [0.2.2](https://github.com/asepharyana/corex/compare/v0.2.1...v0.2.2) (2026-08-28)


### Bug Fixes

* **release:** dispatch publish via gh api in exec successCmd ([3c04aa4](https://github.com/asepharyana/corex/commit/3c04aa44b6110c8ce15278d7dc76828c44dfc8d9))

## [0.2.1](https://github.com/asepharyana/corex/compare/v0.2.0...v0.2.1) (2026-08-28)


### Bug Fixes

* **release:** dispatch publish via exec successCmd not step output ([6c8c98e](https://github.com/asepharyana/corex/commit/6c8c98e86cf27b6a998124e3de3b226a78c82f66))

# [0.2.0](https://github.com/asepharyana/corex/compare/v0.1.0...v0.2.0) (2026-08-28)


### Bug Fixes

* **release:** remove --locked from cargo check in release prepare ([90322b5](https://github.com/asepharyana/corex/commit/90322b5f970aaf48b6da0adaf89c2f516aec0e4c))


### Features

* **release:** add semantic-release auto versioning ([12a1ab1](https://github.com/asepharyana/corex/commit/12a1ab10d04dec40f8b93edb51bb6104f6a75bcb))
