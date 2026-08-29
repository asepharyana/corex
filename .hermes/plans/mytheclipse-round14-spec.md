# Implementation Spec: Round 14 — COMPLETE

## New Feature

### parallel_map / parallel_map_unordered (mytheclipse-core, resiliency)
File: `crates/mytheclipse/src/parallel_map.rs`
- Bounded parallel map over a collection with a concurrency limit
  (Semaphore) — removes manual `Semaphore + join_all` + error-aggregation
  boilerplate that races easily by hand
- `parallel_map(items, concurrency, f) -> Result<Vec<T>, AggregateError>` —
  results in input order; all tasks keep running on failure (fan-out), all
  errors aggregated into one AggregateError
- `parallel_map_unordered` API-symmetry alias (input-ordered, documented)
- Requires I::Item/T: Send + 'static (tokio::spawn)
- 3 tests

## Files
- new: core/src/parallel_map.rs
- core/lib.rs: +module+export parallel_map, parallel_map_unordered

Build: 0 errors. Tests: 0 FAILED (100 core pass). Clippy: 0 new warnings.
