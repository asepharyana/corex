# corex

[![Crates.io](https://img.shields.io/crates/v/corex.svg)](https://crates.io/crates/corex)
[![Documentation](https://docs.rs/corex/badge.svg)](https://docs.rs/corex)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

Resource-aware execution primitives for Rust: async I/O, heavy compute, and background queue management, sized automatically from the host's logical core count and exposed through a single, lazily-initialized engine context.

## Resource Sizing

Given $N$ logical cores (via `num_cpus::get()`):

| Subsystem | Sizing Formula | Default on 8 cores | Backing Primitive |
| :--- | :--- | :--- | :--- |
| **Async I/O** | $N$ | 8 | Ambient `tokio::spawn` + `tracing` span |
| **Compute** | $\max(1, N - 1)$ | 7 | Sized `rayon::ThreadPool` + `catch_unwind` |
| **Background Queue** | $\max(2, \lfloor N / 2 \rfloor)$ | 4 | `tokio::sync::Semaphore` + `tokio::spawn` |

## Features

- **`io`**: enables `corex::spawn_io`, instrumented async task spawning.
- **`compute`**: enables `corex::compute`, panic-isolated execution on a sized Rayon pool.
- **`bg`**: enables `corex::spawn_bg`, semaphore-bounded background tasks.
- **`full`**: enables all three subsystems.

Zero features enabled by default (`default = []`), so you only pull in the dependencies your application actually uses.

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
corex = { version = "0.1", features = ["full"] }
```

Use the entry points directly:

```rust
#[tokio::main]
async fn main() {
    // Optional explicit bootstrap: logs or validates resource sizing upfront.
    // Omit it and the first call to any primitive below will initialize it lazily.
    let ctx = corex::init();
    println!(
        "io_threads={} compute_threads={} bg_concurrency={}",
        ctx.io_threads, ctx.compute_threads, ctx.bg_concurrency
    );

    // 1. Async I/O (instrumented with tracing)
    let io = corex::spawn_io(async {
        // ... network / disk work ...
        42
    });

    // 2. Heavy Compute (isolated from worker panics)
    let sum = corex::compute(|| (1..=1_000_000u64).sum::<u64>())?;

    // 3. Background Queue (concurrency-bounded)
    let bg = corex::spawn_bg(async {
        // ... deferred cleanup / telemetry ...
    }).await;

    let _ = (io.await, bg.await);
}
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
