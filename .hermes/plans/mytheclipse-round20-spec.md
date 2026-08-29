# Implementation Spec: Round 20 — Auto Concurrency

## Goal
`parallel_map` / `parallel_map_unordered` / `parallel_for_each` terima
`usize` (eksplisit, existing) ATAU `()` (auto dari host CPU cores). Tidak
perlu nama API baru — trait `ParallelConcurrency` resolve di call-site.

## Design
- Trait `ParallelConcurrency`: `fn resolve(self) -> usize`
  - impl `usize` → `self.max(1)` (behavior lama, backward compatible)
  - impl `()` → `std::thread::available_parallelism()` fallback 1
- 3 fungsi berubah: `concurrency: usize` → `concurrency: C where C: ParallelConcurrency`
  - `let n = concurrency.resolve();`
  - Body tidak berubah (pakai `n`)
- Export trait di lib.rs

## Backward compat
Caller existing `parallel_map(items, 4, f)` tetap compile — `4` resolve ke
`usize` (satu-satunya impl integer). Literal inference OK karena trait bound
memaksa `usize`.

## Files
- crates/mytheclipse/src/parallel_map.rs (trait + 3 signature)
- crates/mytheclipse/src/lib.rs (export ParallelConcurrency)
- crates/mytheclipse/examples/scaling_demo.rs (demo auto run)
- doctests: tambah contoh auto `()` di parallel_map & parallel_for_each
- tests: `auto_concurrency_uses_cpu_cores` (peak ≤ cores), hasil benar

## Verification
1. `cargo test -p mytheclipse parallel --all-features` — 0 FAILED
2. `cargo test -p mytheclipse --test race_stress --all-features` — 0 FAILED
3. `cargo build --workspace --all-features` — exit 0
4. `cargo clippy --workspace --all-features` — 0 new
5. `cargo run --example scaling_demo` — auto run peak == cores
6. spec + commit + push