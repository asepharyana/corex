# Implementation Spec: Round 21 — CPU Parallel Compute Primitives

## Goal
Fitur parallel khusus CPU (rayon) yang bounded, panic-isolated, error-aggregated.
Melengkapi `compute()` (single call) dengan batch parallel + fork-join.

## New API (crates/mytheclipse/src/compute.rs, feature `compute`)

### 1. `compute_map<I, T, F>(items, f) -> Result<Vec<T>, ComputeErrors>`
- `pack_items` di rayon compute pool: `par_iter().map(f)` — bounded concurrency
  otomatis (rayon work-stealing sizing = CPU cores), ordered output.
- `f: Fn(I::Item) -> Result<T, ComputeMapItemError>`:
  - item error string → dikumpulkan
  - panic per item di-catch (catch_unwind) → jadi error, pool survive
- `ComputeErrors { errors: Vec<String> }` — Display, Error, len, is_empty.
  (Tidak pakai AggregateError — feature `compute` harus compile tanpa resiliency.)
- `I: IntoParallelIterator` (rayon) — work langsung di pool, tanpa materialize.

### 2. `compute_join<A, B, RA, RB>(a, b) -> Result<(RA, RB), MytheclipseError>`
- `rayon::join` wrapper di compute pool: 2 heavy closures run parallel.
- Panic-isolated (catch_unwind per branch) — pool survive, error jadi
  ComputePanic.

### 3. `compute_par_for_each<I>(items, f) -> Result<(), ComputeErrors>`
- `par_iter().for_each` idiom — fire side-effects parallel di pool.
- Panic isolation per item.

## Design notes
- Reuse `context().compute_pool` (existing sizing: compute_threads dari
  RuntimeConfig / available_parallelism) — konsisten dengan `compute()`.
- `rayon::ThreadPool::install` untuk semua — force run di pool.
- Panic isolation: `std::panic::catch_unwind` + AssertUnwindSafe per item
  (sama seperti `compute()` yang sudah proven).
- Bounded = rayon work-stealing — concurrency = pool threads (CPU cores),
  bukan item count. Tidak perlu semaphore.

## Files
- crates/mytheclipse/src/compute.rs (3 fungsi + error type)
- crates/mytheclipse/src/lib.rs (export)
- doctests: compute_map, compute_join, compute_par_for_each
- tests: unit di compute.rs

## Verification
1. `cargo build --workspace --all-features` — exit 0
2. `cargo test -p mytheclipse compute --all-features` — 0 FAILED
3. `cargo test -p mytheclipse --doc --all-features` — 0 FAILED
4. `cargo clippy --workspace --all-features` — 0 new
5. spec + commit + push