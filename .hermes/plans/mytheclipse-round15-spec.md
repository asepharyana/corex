# Implementation Spec: Round 15 — COMPLETE

## New Feature

### parallel_for_each (mytheclipse-core, resiliency)
File: `crates/mytheclipse/src/parallel_map.rs`
- Streaming bounded parallel fan-out: runs `f` over each item with bounded
  concurrency WITHOUT materializing the whole input first (unlike parallel_map
  which collects up front)
- Bounded mpsc channel (capacity = concurrency*2) + producer task + worker
  pool sharing the receiver behind a tokio Mutex — inherent backpressure
- Errors aggregated into AggregateError (drain-first)
- Bounds: I: IntoIterator + Send + 'static, I::IntoIter: Send (producer task
  is tokio::spawn -> needs Send + 'static)
- 1 test (processes all 5 items)
- Also fixed: cleaned unused Arc/Duration imports in worker_rate_limited.rs
  (round-10 leftover)

## Files
- modified: core/src/parallel_map.rs (+parallel_for_each)
- core/lib.rs: export parallel_for_each
- queue/src/worker_rate_limited.rs: remove unused imports

Build: 0 errors. Tests: 0 FAILED (101 core pass). Clippy: 0 new warnings.
