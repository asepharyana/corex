# Implementation Spec: Round 9 — COMPLETE

## New Feature

### RetryExt (mytheclipse-core, resiliency)
File: `crates/mytheclipse/src/retry_ext.rs`
- `RetryExt` trait — `.retry(config, predicate, self_fn)` extension pada Future<Output=Result<T,E>>
- Delegasi ke `crate::retry::retry`
- Non-Send Pin<Box<...>> return (single-threaded test OK)
- 1 test (retries_then_succeeds)

## Files
- new: retry_ext.rs
- core/lib.rs: +module +pub use RetryExt

Build: exit 0. Tests: 0 FAILED. Clippy: 0 new warnings.
