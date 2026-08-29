# Implementation Spec: Round 12 — COMPLETE

## Goal
Self-healing resource pool (auto-reconnect) — remove per-call "is connection
dead? rebuild" boilerplate.

## New Feature

### AutoReconnectPool + Reconnectable (mytheclipse-core, traffic)
File: `crates/mytheclipse/src/pool.rs`
- `Reconnectable` trait: is_healthy(&item) sync probe + reconnect() async builder
- `AutoReconnectPool<P,R>` wraps any Pool<T>; on acquire, checks checked-out item
  health and transparently replaces dead ones via reconnect() — reuses the
  permit so pool size stays stable
- Gated on `traffic` (reuses Pool/SemaphorePool)
- 2 tests (pool returns item + reconnects_broken_item)

## Files
- pool.rs: +Reconnectable +AutoReconnectPool +test
- lib.rs: export AutoReconnectPool, Reconnectable

Build: exit 0. Tests: 0 FAILED. Clippy: 0 new warnings.
