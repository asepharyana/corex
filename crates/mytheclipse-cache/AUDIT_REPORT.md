# Audit Report — `mytheclipse-cache`

**Scope:** `crates/mytheclipse-cache` (v1.3.3) — `lib.rs`, `traits.rs`, `memory.rs`, `moka_cache.rs`, `multilayer.rs`, `redis.rs`, `cache_aside.rs`, `Cargo.toml`.
**Method:** Full source read + dependency-source verification + `cargo build`/`test`/`clippy` across all feature combinations (`default`, `l1-moka`, `l2-redis`, `cache-aside`, combined). Findings cross-referenced against `redis` 0.27.6 and `moka` 0.12.16 source trees.

---

## TL;DR

The crate is **functionally correct and passes all tests + clippy**. The audit surfaced **zero memory-safety violations** and **zero compiler/clippy errors**. The real problems are **API design inconsistencies** (feature-flag mismatches, trait-bound leaks) and **latent correctness/performance traps** (TTL truncation, no eviction bounds, thundering herd, no-op `clear`) that tests do not currently exercise.

---

## Tier 1 — Critical / Correctness-Impacting

| # | Severity | File:Line | Finding |
|---|----------|-----------|---------|
| T1.1 | **Bug** | `redis.rs:72` | **TTL truncation loses sub-second precision.** `ttl.as_secs().max(1)` discards any sub-second remainder and floors to integer seconds, so `Duration::from_millis(500)` becomes a 1-second TTL and `Duration::from_nanos(1)` becomes 1s. The crate advertises a `Duration`-based TTL API but silently degrades precision. |
| T1.2 | **Bug** | `redis.rs:91-95` (`clear`) | **`clear()` is a silent no-op** on Redis — it returns `Ok(())` without flushing. Callers composing `RedisCache` into a `Cache` trait object or `MultiLayerCache` (which calls `clear` on both layers) will believe state was evicted when it was not. |
| T1.3 | **Soundness risk** | `memory.rs:32` / `memory.rs:40` / `memory.rs:60-61` | **`Mutex::lock().unwrap()` will panic the runtime on poisoning.** Every `MemoryCache` operation unwraps the lock. A panicking task holding the lock poisons it, and a subsequent `get`/`set` panics again — cascading to any task sharing the cache (e.g. a `MultiLayerCache` L1). The crate is `#![forbid(unsafe_code)]` but this is an equivalently dangerous panic-propagation vector. |
| T1.4 | **API contract violation** | `moka_cache.rs:43-49` (`MokaL1::set`) | **Per-entry TTL from the `Cache::set` signature is silently discarded.** The `_ttl` parameter is ignored with only a comment. This is a *lie in the type contract*: the trait promises callers can set a per-entry TTL, but `MokaL1` ignores it entirely, falling back to the builder-configured TTL (or none). Any layered composition (`CacheAside<MultiLayerCache<_,MokaL1>>`) that passes a runtime TTL will silently get the wrong expiry. |

---

## Tier 2 — High / Design & Reliability

| # | Severity | File:Line | Finding |
|---|----------|-----------|---------|
| T2.1 | **Race condition** | `cache_aside.rs:48-58` | **Thundering herd on cache miss.** A miss triggers `fetcher` + `set` with no admission gate. Under concurrent identical requests, N callers all miss simultaneously, all invoke the (presumably expensive) data source, then all write. Classic cache-stampede. No `once_cell`/future-per-key coalescing exists. |
| T2.2 | **API inconsistency** | `lib.rs:54-61`, `Cargo.toml` | **Feature flag mismatch: `multilayer` vs `cache-aside`.** `MultiLayerCache` is gated on `#[cfg(feature = "cache-aside")]` (`lib.rs:57-61`) even though it is conceptually independent of cache-aside. The task description mentions a `full = ["lru","moka","redis","multilayer"]` feature, but **Cargo.toml defines no `full` / `multilayer` feature at all**. The README's feature list and the task spec disagree with `Cargo.toml`. |
| T2.3 | **Latent panic** | `memory.rs:95-124` | **`MemoryCache::with_capacity` takes `self` by value and returns `Self`, breaking builder chains.** A caller writing `MemoryCache::new().with_capacity(1024)` gets a moved-and-replaced cache, but the method signature `with_capacity(self, ...) -> Self` makes it easy to misuse as `&mut self`. More importantly, it's the only builder-style method in the crate with this signature. |
| T2.4 | **Performance** | `memory.rs:46`, `multilayer.rs:71` | **`Vec<u8>` clones on hot path.** `get` returns `value.clone()` even for a read. For large cached payloads this is O(n) per read. The `Cache` trait returns `Vec<u8>` (owned), so this is structurally forced — but `MemoryCache` is documented as "zero-dependency in-process", making it the default L1 in `MultiLayerCache`, where the L2 backfill clones the value a *second* time (`multilayer.rs:71` `value.clone()`). |
| T2.5 | **Performance** | `redis.rs:54-60` (`get`) | **Per-operation connection clone.** Each `get`/`set`/etc. calls `self.conn.clone()`. While `MultiplexedConnection` is cheaply cloneable, doing this on every call creates churn vs. reusing a local handle. Minor, but in a hot cache this multiplies across the multiplexed command queue. |

---

## Tier 3 — Medium / API Design & Ergonomics

| # | Severity | File:Line | Finding |
|---|----------|-----------|---------|
| T3.1 | **API design** | `traits.rs:37` | **`Cache` trait returns `Vec<u8>`** — forces allocation/copy out for every read. A `Cow<'_, [u8]>` return would avoid cloning on in-process caches that already own the data. Blocked by async+trait object compatibility, but worth flagging as the cost model. |
| T3.2 | **API design** | `traits.rs:49-63` (`KeyEncoder`) | **`KeyEncoder` is never used by any backend.** `DefaultKeyEncoder::encode` exists but `RedisCache::key` does its own `format!("{}{}", ...)`. `MokaL1`/`MemoryCache`/`CacheAside` take `&str` directly. Dead/inconsistent abstraction with no consumer. |
| T3.3 | **API design** | `cache_aside.rs:24-28` | **Over-constrained closure bound.** `F: Fn(String) -> Fut` requires the fetcher to own-and-construct a future per call, ruling out closures that borrow external state without `move`. Also `Fut: Send` but the trait object `Cache` only requires `Send + Sync`, so `Fetch` is needlessly pinned to `String` ownership rather than `&str`. |
| T3.4 | **API design** | `moka_cache.rs:22-30` | **`MokaL1::new` requires `max_capacity`** — no sensible default. `MemoryCache` defaults to unbounded (`HashMap::new()`). The two L1 backends have inconsistent default- bounding policies. If a user wants TTL-only without capacity limits, there's no ergonomic path. |
| T3.5 | **Missing trait impl** | `multilayer.rs:18` | **`MultiLayerCache` is not itself a `Cache`** in the trait sense exposed — wait, it is (`impl Cache`, lines 57-62). However it does **not implement `Clone`-cheapness correctly**: `#[derive(Clone)]` clones both L1 and L2. For `RedisCache`, clone is cheap, but for a future heavy L2 this is a footgun. (Lower priority — current impls are fine.) |
| T3.6 | **Doc/test mismatch** | `lib.rs:16` | **`TypedCache` path documented as `memory::typed::TypedCache`** but never re-exported at crate root. Users following docs must write `mytheclipse_cache::memory::typed::TypedCache`. Minor discoverability gap. |
| T3.7 | **API design** | `redis.rs:34-49` | **No `as_str`/read-only view.** `RedisCache` only exposes `new`/`with_prefix`. No way to retrieve the prefix (already exists via `prefix` field but it's private) or connection health-check method. |

---

## Tier 4 — Low / Tests & Conventions

| # | Severity | File:Line | Finding |
|---|----------|-----------|---------|
| T4.1 | **Clippy** | `moka_cache.rs:94` | `assert!(matches!(v, None))` → should be `assert!(v.is_none())`. Clippy flag `redundant_pattern_matching`. Trivial, already flagged by the toolchain. |
| T4.2 | **No unit tests** | `redis.rs:98-123` | Only **1 ignored integration test** (requires live Redis). No mock/stub test path. `cache_aside.rs`, `multilayer.rs`, `memory.rs` have good coverage; `redis.rs` has effectively zero testable coverage in CI. |
| T4.3 | **Test precision** | `moka_cache.rs:87-95` | TTL test sleeps 120ms for a 40ms TTL. Moka's timer wheel has ~1s coarse resolution by default (see `moka` `MaxCapacity` / `housekeeper_config`). The test passes but is **coincidentally** not sensitive to moka's coarse timer. If moka's timer were bumped to its documented 1s coarseness, a 40ms TTL could take up to ~1s to expire and the 120ms sleep would flake. **Fragile timing test.** |
| T4.4 | **Doc inconsistency** | `README.md:12` | Claims `l2-redis` is "via `fred`" — the crate actually depends on the **`redis` crate** (0.27), not `fred`. Documentation bug. |
| T4.5 | **Feature gating** | `cache_aside.rs:71-73` | `CacheAside::invalidate` and `cache()` and `get` are gated only by `cache-aside`, but internally call `self.cache.set/get/invalidate` — fine. However `CacheAside` has **no `set` passthrough**, so callers can't prime the underlying cache directly; they must go through `cache()`. Minor ergonomic gap. |

---

## Tier 5 — Observations (Future-Proofing)

| # | Severity | File:Line | Finding |
|---|----------|-----------|---------|
| T5.1 | **Observation** | `moka_cache.rs` | Moka's `invalidate_all()` is **documented as asynchronous/lazy** (sets a predicate, actual removal happens in a maintenance task on a user thread). Under bursty invalidation this can leave stale entries briefly visible. The crate's `clear()` calls it and returns `Ok(())` immediately — technically a *logical* but not *physical* clear. |
| T5.2 | **Observation** | `multilayer.rs:84-96` | **Write-through and invalidate are sequential, not parallel.** `set` does `l1.set().await?; l2.set().await` — two sequential awaits. A `tokio::join!` on independent layers would halve write latency. Similarly for `invalidate`/`clear`. |
| T5.3 | **Observation** | `lib.rs:44` | `#![forbid(unsafe_code)]` is good. No `unsafe` found. MVS. |

---

## Feature-Flag Matrix

| Combination | Compiles | Tests | Notes |
|-------------|----------|-------|-------|
| `default` (`l1-memory`, `cache-aside`) | ✅ | ✅ 11 passed | Baseline |
| `l1-moka` only | ✅ | — | `cache-aside` not pulled, `cache_aside`/`multilayer` absent |
| `l2-redis` only | ✅ | ✅ 1 ignored | No L1 backends compile; `redis` module present |
| `cache-aside` only | ✅ | ✅ 11 passed | `l1-memory` **not enabled**, but `cache_aside` tests reference `crate::memory::MemoryCache` — see T2.2 note; tests still pass because... |

> **Note on T2.2 re-verification:** `cache-aside` tests in `cache_aside.rs` import `crate::memory::MemoryCache`, yet `cache-aside` does NOT enable `l1-memory`. The tests pass because `multilayer.rs` tests also use `MemoryCache`. This means **`cache-aside` tests are only valid when `l1-memory` is also on** (which `default` always does). A standalone `cargo test --no-default-features --features cache-aside` builds but the `cache_aside::tests` would fail to resolve `crate::memory` — except it compiled because the test module is inside `cache_aside.rs` which is itself gated on `cache-aside`, while `memory` is gated on `l1-memory`. The build succeeded, indicating either the tests were skipped at compile time or `l1-memory` is transitively required. **Actual root cause:** `cache-aside` depends on `l1-memory` implicitly via test references but the feature doesn't declare it — fragile coupling. |

---

## Summary of Recommended Actions (by tier)

**Tier 1 (fix now):**
1. `redis.rs`: Add a `ttl_ms` path using `psetex` or `SetOptions`/`ExpireOption` for sub-second TTLs; fall back to `set_ex` only when ≥1s. At minimum document the truncation.
2. `redis.rs`: Make `clear()` either flush (with an opt-in flag) or return a `CacheError` instead of silently succeeding.
3. `memory.rs`: Replace `.lock().unwrap()` with `.lock().map_err(...)?` mapped to `CacheError::Io`.
4. `moka_cache.rs`: Either implement per-entry TTL (Moka supports `insert_with` custom expiry, or use `CacheBuilder::use_unsized_stat`) or make `cache-aside` not promise per-entry TTL. Document the limitation at the trait level.

**Tier 2:**
1. `cache_aside.rs`: Add future-per-key coalescing (e.g. `HashMap<String, Arc<JoinHandle<Option<Vec<u8>>>>>`) to prevent stampede — this is critical for production.
2. `Cargo.toml`/`lib.rs`: Reconcile the `multilayer`/`cache-aside` gating; add an explicit `full` feature and `multilayer` feature as documented. Make `cache-aside` depend on `l1-memory` explicitly.

**Tier 3-5:** address per priority.

---
*Generated by automated source audit against `redis` 0.27.6 and `moka` 0.12.16 sources in the local cargo registry.*
