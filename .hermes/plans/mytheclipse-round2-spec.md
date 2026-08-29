# Implementation Spec: Round 2

## New Features

### 1. ServiceBuilder (mytheclipse-core)
File: `crates/mytheclipse/src/service_builder.rs`
- Builder that wraps async operations with retry + circuit breaker + timeout + rate limiter
- Fluent API: `.retry(config)`, `.circuit(config)`, `.timeout(dur)`, `.rate(rate, burst)`, `.concurrency(max)`, `.run(fut)`
- Feature gate: `resiliency` (uses existing retry/CircuitBreaker/timeout primitives)
- Integrates with metrics: records retries, circuit events, timeouts

### 2. DistributedLock (mytheclipse-core)  
File: `crates/mytheclipse/src/dlock.rs`
- `DistributedLock` trait: `acquire(timeout)`, `release()`, `extend(lease_dur)`
- `InProcDistributedLock` impl using tokio Mutex + lease time tracking
- `RedisLock` impl (feature `redis`) — Redis SETNX with PX expiry
- Feature gate: `lifecycle` (uses existing leader election infra)

### 3. StreamingPipeline (mytheclipse-queue)
File: `crates/mytheclipse-queue/src/pipeline.rs`
- Pipe stages: `Stage<Input, Output>` trait with async `process(item) -> Output`
- Pipeline: `add_stage(impl Stage)`, `run(input_stream)`, `collect()`
- Backpressure: bounded channel between stages
- Feature gate: `in-memory` (uses tokio + std)
