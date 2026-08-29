# Implementation Spec: Round 17 — Race-Safety Stress Tests + Doctests

## Goal
Misi project: menghilangkan boilerplate race-condition. Bukti nyata bahwa
primitives aman di bawah kontensi tinggi. Tambahkan:
1. Stress/concurrency tests untuk primitives race-sensitive di core crate
2. Doctest `# Examples` untuk fitur round 7-16 agar docs.rs langsung berguna

## 1. New file: crates/mytheclipse/tests/race_stress.rs

Integration test (tests/ dir = pakai public API saja, autentik dari luar):
- `tokio::test(flavor = "multi_thread", worker_threads = 8)` — kontensi asli
- High-contention tests:
  a. TokenBucket try_consume atomic — 64 tasks × 1000 consumes dari 1 bucket
     capacity 100, rate tinggi → total consume ≤ capacity per window, no double
  b. SemaphorePool acquire/release concurrent — 100 tasks acquire+release
     cycle, final available == capacity, no leak
  c. AutoReconnectPool — healthy probe retval, 50 concurrent acquire, semua
     dapat item valid
  d. RateLimitedQueue concurrent enqueue/dequeue — 8 worker × 1000 item,
     total dequeue == total enqueue
  e. ShutdownGuard exactly-once — 10 clones-ish concurrent drops → callback
     count == 1 (via Arc<AtomicUsize>)
  f. parallel_map 10k items concurrency 32 — hasil input-ordered, nilai benar
  g. parallel_for_each 10k items concurrency 32 — side-effect count == 10k
  h. AggregateError from_results merge 100 results mix ok/err — error count
     benar, values semua lolos yang ok
- Assertions: `assert_eq!` pada counts; harness FAILS kalau race → flaky

## 2. Doctest `# Examples` additions

Untuk file baru round 9-16 (masing-masing sudah punya unit tests; tambah
doctest singkat di doc comment pub item paling utama):
- retry_ext.rs: `RetryExt::retry` contoh 1-liner
- auto_metrics_service.rs: AutoMetricsServiceBuilder contoh
- runtime_auto.rs: RuntimeConfig::auto contoh
- shutdown_guard.rs: ShutdownGuard contoh
- aggregate_error.rs: AggregateError::from_results contoh
- parallel_map.rs: parallel_map + parallel_for_each contoh
- pool.rs AutoReconnectPool: contoh

Doctest wajib compile: `cargo test --doc --workspace --all-features`

## Files
- new: crates/mytheclipse/tests/race_stress.rs
- edit: parallel_map.rs, retry_ext.rs, auto_metrics_service.rs, runtime_auto.rs,
  shutdown_guard.rs, aggregate_error.rs, pool.rs (doctest blocks)

## Verification
1. `cargo test -p mytheclipse --tests --all-features` — 0 FAILED
2. `cargo test -p mytheclipse --doc --all-features` — 0 FAILED
3. `cargo build --workspace --all-features` — exit 0
4. `cargo clippy --workspace --all-features` — 0 new warnings
5. Commit + push