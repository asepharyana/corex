# Implementation Spec: Round 11 — COMPLETE

## Goal
Auto thread/core allocation + race hardening (RAII shutdown).

## New Features

### 1. RuntimeConfig (mytheclipse-core, lifecycle)
File: `crates/mytheclipse/src/runtime_auto.rs`
- `RuntimeConfig::auto()` / `from_cores(n)` / `compact()` infer worker_threads,
  max_blocking_threads, compute_threads, io_threads from host CPU topology
  (std::thread::available_parallelism)
- `available_parallelism()` helper
- `build_rayon_pool(cfg)` gated on `compute` feature
- 3 tests

### 2. ShutdownGuard (mytheclipse-core, lifecycle)
File: `crates/mytheclipse/src/shutdown_guard.rs`
- RAII guard — runs completion callback exactly once on drop (panic-safe via
  Mutex<Option<Box<FnOnce>>>), prevents double-shutdown race
- `new(cb)` + `finish()` (fire now + disarm)
- 3 tests (fires on drop, finish once, panic path)

## Files
- new: core/src/runtime_auto.rs, core/src/shutdown_guard.rs
- core/lib.rs: +module+export for both

Build: exit 0. Tests: 0 FAILED. Clippy: 0 new warnings.
