# Implementation Spec: Round 19 — Criterion Benchmarks

## Goal
Buktikan klaim "secepat mungkin" (tujuan awal project) dengan benchmark
nyata. Ukur overhead primitives race-safe vs baseline naif, supaya user
tahu trade-off dan bisa memilih fitur dengan data.

## New files

### crates/mytheclipse/benches/primitives.rs
Criterion bench untuk primitives core (feature `full`):
- `parallel_map`: throughput 1000 item, concurrency 8 vs sequential loop
  (pakai `black_box`)
- `retry_ext`: overhead `.retry()` success-first vs 2 retries
- `rate_limiter`: `RateLimiter::try_acquire` throughput (atomic CAS)
- `semaphore_pool`: acquire/release cycle throughput — bukti no-leak + low
  overhead
- `aggregate_error`: `from_results` 1000 results all-ok vs 50% err
- `shutdown_guard`: new + drop cost

## Dependency
- dev-deps: `criterion = "0.5"` + `[[bench]]` harness = false
- `harness = false` di Cargo.toml bench section (criterion punya main sendiri)

## Verification
1. `cargo bench -p mytheclipse --bench primitives --all-features` — runs,
   reports times
2. `cargo build --workspace --all-features` — exit 0
3. `cargo clippy --workspace --all-features` — 0 new warnings
4. Spec + commit + push

## Notes
- Criterion 0.5 mendukung MSRV 1.60 — aman untuk rust-version 1.75.
- Bench tidak jalan di CI (hanya manual) — tidak mempengaruhi pipeline.