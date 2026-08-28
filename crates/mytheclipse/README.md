# mytheclipse

[![Crates.io](https://img.shields.io/crates/v/mytheclipse.svg)](https://crates.io/crates/mytheclipse)
[![Documentation](https://docs.rs/mytheclipse/badge.svg)](https://docs.rs/mytheclipse)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

Resource-aware execution primitives and reliability abstractions for Rust: async I/O, heavy compute, background queue management, resiliency, traffic control, lifecycle management, and observability. Sized automatically from the host's logical core count and exposed through a single, lazily-initialized engine context alongside self-contained, constructible utilities.

## Resource Sizing

Given $N$ logical cores (via `num_cpus::get()`):

| Subsystem | Sizing Formula | Default on 8 cores | Backing Primitive |
| :--- | :--- | :--- | :--- |
| **Async I/O** | $N$ | 8 | Ambient `tokio::spawn` + `tracing` span |
| **Compute** | $\max(1, N - 1)$ | 7 | Sized `rayon::ThreadPool` + `catch_unwind` |
| **Background Queue** | $\max(2, \lfloor N / 2 \rfloor)$ | 4 | `tokio::sync::Semaphore` + `tokio::spawn` |

The three execution primitives (`io`, `compute`, `bg`) are sized from the host's CPU core count through `mytheclipse::context()`. Resiliency, traffic control, lifecycle, and observability utilities are constructible per-instance without global state.

## Features

- **`io`**: enables `mytheclipse::spawn_io`, instrumented async task spawning.
- **`compute`**: enables `mytheclipse::compute`, panic-isolated execution on a sized Rayon pool.
- **`bg`**: enables `mytheclipse::spawn_bg`, semaphore-bounded background tasks.
- **`resiliency`**: fault tolerance abstractions:
  - `retry` — auto-retry with exponential backoff and full/equal jitter.
  - `CircuitBreaker` — failure threshold and cooldown state machine (Closed/Open/HalfOpen).
  - `with_timeout` / `timeout` / `Timeout` — hard execution deadlines.
- **`traffic`**: traffic & load control:
  - `RateLimiter` — lazy token-bucket rate limiter with burst capacity.
  - `BackpressureQueue` — bounded queue with `DropOldest`, `Reject`, and `Block` overflow policies.
  - `ConcurrencyLimiter` — synchronous RAII semaphore wrapper for capping concurrent operations.
- **`lifecycle`**: system lifecycle coordination:
  - `ShutdownManager` / `ShutdownSignal` — OS signal catching (SIGINT/SIGTERM/Ctrl-C) and graceful task draining.
  - `CronSchedule` / `schedule` — self-contained 5-field cron parser and async timer scheduler.
- **`observability`**: runtime visibility:
  - `MetricsCollector` — thread-safe statistics collector with Prometheus text exposition format export.
  - `PanicTracker` — non-fatal panic logging with tracing context and boundary isolation.
- **`full`**: enables all subsystems: `io`, `compute`, `bg`, `resiliency`, `traffic`, `lifecycle`, `observability`.

Zero features enabled by default (`default = []`), so you only pull in the dependencies your application actually uses.

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
mytheclipse = { version = "0.2", features = ["full"] }
```

### 1. Execution Primitives

```rust
#[tokio::main]
async fn main() {
    let ctx = mytheclipse::init();

    // Async I/O (instrumented with tracing)
    let io = mytheclipse::spawn_io(async { 42 });

    // Heavy Compute (isolated from worker panics)
    let sum = mytheclipse::compute(|| (1..=1_000_000u64).sum::<u64>())?;

    // Background Queue (concurrency-bounded)
    let bg = mytheclipse::spawn_bg(async { /* task */ }).await;

    let _ = (io.await, bg.await);
}
```

### 2. Resiliency & Fault Tolerance

```rust
use std::time::Duration;

// Auto-Retry with Exponential Backoff + Jitter
let result = mytheclipse::retry(
    mytheclipse::RetryConfig::default(),
    |err| err.is_transient(),
    || async { make_network_request().await },
).await?;

// Circuit Breaker
let breaker = mytheclipse::CircuitBreaker::new(mytheclipse::CircuitBreakerConfig::default());
let value = breaker.call(|| fetch_remote_resource())?;

// Timeout & Deadlines
let value = mytheclipse::with_timeout(Duration::from_secs(5), async {
    long_running_task().await
}).await?;
```

### 3. Traffic & Resource Control

```rust
// Rate Limiter (Token Bucket)
let limiter = mytheclipse::RateLimiter::new(100.0, 10);
limiter.acquire().await?;

// Concurrency Limiter (Sync Semaphore)
let limiter = mytheclipse::ConcurrencyLimiter::new(10);
let _permit = limiter.acquire(); // released on drop

// Backpressure Queue (Graceful Degradation)
let queue = mytheclipse::BackpressureQueue::new(100, mytheclipse::OverflowPolicy::DropOldest);
queue.push(job).await?;
let next_job = queue.pop().await;
```

### 4. Lifecycle & State Management

```rust
// Graceful Shutdown Manager
let shutdown = mytheclipse::ShutdownManager::new();
let sig = shutdown.handle();
tokio::spawn(async move {
    let mut sig = sig;
    tokio::select! {
        _ = sig.wait() => { /* clean up */ }
        _ = worker_loop() => {}
    }
});
shutdown.drain(Duration::from_secs(10)).await;

// Cron Periodic Job Scheduler (Self-contained, no external crates)
let cron = mytheclipse::CronSchedule::parse("0 1 * * *")?; // 1 AM daily
let job = mytheclipse::schedule("0 1 * * *", || async {
    clean_cache().await;
})?;
```

### 5. Telemetry & Observability

```rust
// Centralized Metrics Collector (Prometheus text exposition format)
let metrics = mytheclipse::MetricsCollector::new();
metrics.record_task(Duration::from_millis(15));
metrics.inc_counter("http_requests_total", 1);
let prometheus_output = metrics.export_prometheus();

// Panic Isolation Tracker
let guard = mytheclipse::PanicTracker::install(); // logs panics with tracing span
let result = mytheclipse::PanicTracker::catch(|| {
    risky_operation()
});
```

## Running the Example

```bash
cargo run --example main --features full
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
