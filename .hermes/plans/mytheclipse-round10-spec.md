# Implementation Spec: Round 10 — COMPLETE

## Goal
Auto-integration + ergonomics: rate-limit workers, auto-metrics on service calls —
reduce manual wiring/boilerplate.

## New Features

### 1. AutoMetricsServiceBuilder (mytheclipse-core, observability)
File: `crates/mytheclipse/src/auto_metrics_service.rs`
- Composes ServiceBuilder + MetricsCollector (+ MetricsBridge when resiliency)
- `.run()` auto-records: calls_total counter (labelled by outcome ok/err/timeout/
  circuit_open/rate_limited) + duration histogram; emits bridge when attached
- Chainable .with_collector/.with_bridge/.with_builders
- 1 test

### 2. RateLimitedWorkerPool (mytheclipse-queue, in-memory)
File: `crates/mytheclipse-queue/src/worker_rate_limited.rs`
- Wraps WorkerPool with RateLimitedQueue — token-bucket back-pressured dequeue,
  prevents workers hammering upstream beyond rate limit
- new(queue, worker_cfg, rate_per_sec, burst) + start(topic, handler)
- 1 test (construction)

## Files
- new: core/src/auto_metrics_service.rs, queue/src/worker_rate_limited.rs
- core/lib.rs: +module+export AutoMetricsServiceBuilder
- queue/lib.rs: +module+export RateLimitedWorkerPool (rewrote export block)

Build: exit 0. Tests: 0 FAILED (86 core pass). Clippy: 0 new warnings.
