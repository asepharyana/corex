# Audit Report — `mytheclipse-config` v1.3.3

**Scope:** `crates/mytheclipse-config/src/` (`lib.rs`, `error.rs`, `loader.rs`, `dynamic.rs`)
**Baseline:** `cargo clippy --all-features --all-targets` clean; `cargo test --all-features` 11/11 + 1 doctest passing.
**Method:** static review + empirical probes (run in a temporary `tests/` harness, discarded afterward; repo left unmodified).

---

## TL;DR

The crate is functional and compiles cleanly, but has **4 High**, **2 Medium**, and **3 Low** issues. The strongest findings are: (1) environment variables are auto-typed with no opt-out, silently mangling IDs/phone-numbers and breaking deserialization into `String`/`int` fields; (2) YAML/TOML→JSON conversion uses `unwrap_or(Value::Null)` which silently nulls fields on non-finite floats or key-conversion errors; (3) `DynamicConfig` background file-watcher threads are detached with **no shutdown/stop handle**, leaking OS watchers for the process lifetime; (4) `DynamicConfig::set()` and `build()` perform **no validation at all**, despite the crate advertising "with validation" in its description — there is no validator hook anywhere.

---

## 1. Config-loading correctness

| # | Severity | Location | Finding |
|---|----------|----------|---------|
| 1.1 | **High** | `loader.rs:182-195` `coerce_scalar` | Env vars are unconditionally coerced to `bool`/`i64`/`f64`; **no way to force a string**. Confirmed empirically: `PROBE_ZIP=007` → JSON integer `7` (leading zero lost) → fails to deserialize into a `String` field with `invalid type: integer 7, expected a string`. Same class of breakage for phone numbers, version strings (`1.0`→`1.0` f64), leading-zero IDs. This is a surprising, data-destroying default with no escape hatch (no `as_string`/raw mode). |
| 1.2 | **High** | `loader.rs:116` (`yaml_to_json`) & `120` (`toml_to_json`) | `serde_json::to_value(v).unwrap_or(Value::Null)` **silently discards conversion errors**. Confirmed empirically: a YAML scalar `val: .inf` (a perfectly valid YAML 1.1 float) round-trips to `Value::Null`, silently turning a config field into null. Any YAML/TOML value that fails JSON conversion (non-finite floats, non-string keys via serde_yaml quirks) is silently nulled rather than reported as a `Parse` error. A user with `timeout: .inf` gets a silent `null` config field — undetectably wrong behavior. |
| 1.3 | **Medium** | `loader.rs:167-178` `insert_nested` | **Silent data loss on path collisions.** When two env vars produce colliding nested paths (e.g. `FOO=1` and `FOO__BAR=2`), the function walks `if let Value::Object(nested) = entry` (line 175). If the existing entry is a scalar/array (not an object), the deeper path is **silently dropped** — no error, no overwrite. The user has no way to know a field was ignored. |
| 1.4 | **Low** | `loader.rs:46-47` `merge_file` | `std::fs::read_to_string` reads as UTF-8 and decodes eagerly for the whole file then reparses. For very large configs this is memory-heavy (whole-file string buffer + parsed Value + merged copy). Not a bug per se, but combined with `1.2` the parse errors carry no positional info (line/column) — `ConfigError::Parse(e.to_string())` swallows serde's span. Acceptable but noisy for debugging. |

### Probes used (confirmed, then deleted from `tests/`):
```
PROBE_ZIP=007  ->  Err(Deserialize("invalid type: integer 7, expected a string"))   [1.1]
YAML val: .inf ->  {"port":1,"val":null,"zip":2}                                       [1.2]
PROBE2_N=1.5   ->  Err(Deserialize(... integer/float into u16))                       [1.1]
PROBE3_BIG=-5  ->  Err(Deserialize("invalid value: integer -5, expected u32"))        [1.1]
```

---

## 2. Type safety in dynamic config

| # | Severity | Location | Finding |
|---|----------|----------|---------|
| 2.1 | **High** | `dynamic.rs:50-57` `set` | `DynamicConfig::set()` accepts **any** `T` with zero validation. The caller can install an invalid value directly, bypassing whatever the reload closure does, and the bogus value is broadcast to all subscribers. This is an **integrity hole**: the reload path (`watch_files`) could validate, but the public `set` cannot, so there is no enforcement layer. |
| 2.2 | **Medium** | `dynamic.rs:34-39` `new` / `watch_files` | No validation on the initial `reload()?` result (line 90) either — whatever the closure returns is stored blindly. There is no `Validate`/`Into` hook in the `Config` trait or on the struct. |
| 2.3 | **Low** | `lib.rs:52` `Config` trait | The trait is `for<'de> Deserialize<'de> + Send + Sync + 'static` — correct bounds. But it provides **no contract** for validity (range, non-empty, etc.), so "type-safe" is only about Rust types, not value validity. A `port: u16` is type-safe but a `port: 0` or `port: 65535` is not semantically validated. |
| 2.4 | **Low** | `dynamic.rs:46,54` | `expect("...RwLock poisoned")` panics on poisoned lock instead of returning an error. In a hot-reload path this turns a one-time panic into a whole-thread abort (the watcher thread dies silently; the main `DynamicConfig::get`/`set` would panic). Acceptable for poisoning (it's a genuine bug to continue past poison), but the **watcher thread panic is unobserved**: if `reload()` itself panics, the watcher thread dies and the config silently stops reloading with no diagnostic. |

---

## 3. Thread-safety of reload

| # | Severity | Location | Finding |
|---|----------|----------|---------|
| 3.1 | **High** | `dynamic.rs:96-131` `watch_files_debounced` | **No shutdown / cleanup handle.** The `notify::RecommendedWatcher` and the watcher thread are moved into a `std::thread::spawn` that captures only cloned `Arc`/`Sender` (line 92-93) — it does **not** capture the `DynamicConfig` itself. Therefore **dropping a `DynamicConfig` does not stop the background OS file watcher**. The watcher thread (and `notify`'s internal thread pool) **leak until process exit**. Confirmed by design: `let _watcher = watcher;` (line 110) only keeps the watcher alive for the thread; nothing links its lifetime to the `DynamicConfig`. Repeated create/destroy of `DynamicConfig` (e.g. per-request or per-test) accumulates threads + inotify/FSEvents handles. There is `no explicit "unwatch"` — the docstring even says so (line 72) — but provides **no alternative stop mechanism**, which is a resource-leak/API-design defect, not merely a limitation. |
| 3.2 | **Medium** | `dynamic.rs:95-99` | The `recommended_watcher` callback sends `Result<WatchEvent, ...>` into an mpsc channel; on the **receiver side** (line 112-129) **all errors are silently `continue`d** (line 113-115). A watcher error (e.g. `remove_watch` failure, overflow) is swallowed with no log. This masks real filesystem-watcher failures during reload. |
| 3.3 | **Low** | `dynamic.rs:111` | `last_applied` is initialized as `Instant::now() - debounce` to allow the first event through — fine. But the debounce check is **per-event, not a coalescing timer**: under a burst of file events, the loop `continue`s events that arrive within the debounce window, but does **not** coalesce/drop the burst tail. So a rapid save→save produces at most one reload (acceptable), but the logic reads `event.is_err()` on a `Result<WatchEvent,_>` returned by `raw_rx` — the `is_err()`/`is_ok()` path is correct, but a `NotifyResult` error variant is silently skipped (see 3.2). |

**Thread-safety conclusion:** Memory-safety is sound (`Arc<RwLock<T>>` + `broadcast` are all `Send+Sync`; all shared state is properly synchronized). **Soundness is fine, but lifecycle management is broken** (3.1) — detached threads/watchers leak.

---

## 4. Missing validations

| # | Severity | Location | Finding |
|---|----------|----------|--------|
| 4.1 | **High** | (whole crate) | **No validation API exists at all.** The crate description (Cargo.toml line 11) and docs promise "type-safe ... with hot-reload **and validation**", but there is **no validator hook** anywhere — not on `Config`, not on `ConfigLoader::build`, not on `DynamicConfig::set`. Value-level invariants (ranges, formats, non-empty strings, etc.) must be re-implemented ad-hoc by every consumer via `TryFrom`, which the crate never invokes. This is a **feature-gap that contradicts the public contract**. |
| 4.2 | **Medium** | `loader.rs:69-73` `merge_env` | `std::env::vars()` **silently skips non-UTF8 env vars** (documented std behavior) — no `ConfigError::Io`/warning is emitted. Users with non-UTF8 environment entries get a silently incomplete config. Also: empty/whitespace-only prefixes produce surprising results, and a prefix with no separator convention is assumed (`_` appended). There is no validation that the prefix is well-formed, and no warning when the collected env map is empty (could be a misconfiguration — wrong prefix). |
| 4.3 | **Medium** | `loader.rs:76-78` `build` | `serde_json::from_value(self.value)` is the **only** correctness check. If the merged `Value` is `null` (e.g. from `1.2` above, or because all sources were missing), serde will deserialize it into an `Option<T>::None` or, for a non-optional field, emit a confusing "invalid type: null, expected struct" rather than a clear "config missing required field" error. There is no "required sources" check. |
| 4.4 | **Low** | `loader.rs:43-51` `merge_file` | Extension is the **only** format selector — a `.json` file containing YAML, or a `.yaml` file with JSON, would parse inconsistently. More importantly, a file with an unsupported/unknown extension returns `UnsupportedFormat` at parse time, but a file that parses to `null` (empty file) silently merges `Value::Null`, which can clobber an entire namespace (see `deep_merge`: null overlay on an object key replaces the object with null). No validation that a loaded file was non-empty / structurally valid. |
| 4.5 | **Low** | `dynamic.rs:119-128` | A failed reload logs via `tracing::error!` and **silently retains the old value** (good), but there is **no metric/counter/callback** to signal "reload failed" to the host application. Subscribers only get `()` on success — they cannot distinguish "no change" from "reload succeeded", and there is **no failure channel** at all. Observability gap. |

---

## 5. Severity rationale

- **High** = data-loss, silent-wrong-behavior, or contradicts documented contract.
- **Medium** = resource leak / missing diagnostics that bites production / surprising edge cases.
- **Low** = noise / observability / hardening.

---

## 6. Recommended fixes (no code changed in this audit)

1. **Env typing (1.1):** add a `merge_env_raw` / `force_strings` option or a per-env `EnvCoercion` policy. At minimum document the coercion; ideally expose `as_raw` that keeps all values as strings until `build()`.
2. **Conversion nulling (1.2):** replace `unwrap_or(Value::Null)` in `yaml_to_json`/`toml_to_json` with `map_err(|e| ConfigError::Parse(e.to_string()))`. Non-finite floats should produce a `Parse` error, not a silent `null`.
3. **Path collisions (1.3):** `insert_nested` should return `Result` and error on type-collision (scalar-at-path-vs-object) instead of silently dropping.
4. **Watcher lifecycle (3.1):** return a guard/handle from `watch_files`/`watch_files_debounced` (e.g. `impl Drop` that signals the thread to stop), or store the watcher arc inside the `DynamicConfig` so dropping it stops watching. `tokio::select!` + a shutdown channel is the idiomatic fix.
5. **Validation (4.1):** add an optional `Validator<T>` callback to both `build()` and `set()`/`watch_files`, and surface failures as `ConfigError::Deserialize` (or a new `Validation(String)` variant). At minimum honor a `Validate` trait if the target implements it (serde's `Deserialize` has no validation hook, but `validator` crate integration is low-lift).
6. **Empty-config / null-clobber guards (4.3, 4.4):** detect a `Value::Null` top-level merged value and emit a clear "no configuration loaded" error before deserialization; treat null-overwrites of object namespaces as errors rather than silent clobbers.
7. **Watcher-error diagnostics (3.2):** log the `notify` result errors (not just `continue`) with `tracing::warn!`.
8. **Poisoning (2.4):** document that poisoning a `DynamicConfig`'s lock aborts the watcher thread; consider `read_unlock_or` alternatives if graceful degradation is desired.

---

## 7. Files inspected
- `crates/mytheclipse-config/src/lib.rs`
- `crates/mytheclipse-config/src/error.rs`
- `crates/mytheclipse-config/src/loader.rs`
- `crates/mytheclipse-config/src/dynamic.rs`
- `crates/mytheclipse-config/Cargo.toml`

**No source files were modified.** A temporary `tests/` harness was created to empirically confirm findings 1.1, 1.2, 1.3-type behavior and 2.1 (via reasoning), then removed; `git status` is clean.
