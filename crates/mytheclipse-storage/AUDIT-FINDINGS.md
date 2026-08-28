# Audit Findings — `mytheclipse-storage`

**Scope:** `crates/mytheclipse-storage/src/` (`lib.rs`, `traits.rs`, `local.rs`, `s3.rs`, `gcs.rs`)
**Method:** static review + `cargo clippy --all-features` + `cargo test --all-features` (5 passed, 0 failed; 2 live tests ignored).
**Severity legend:** CRITICAL · HIGH · MEDIUM · LOW

---

## 1. Path traversal — `local.rs` (mostly PASS, one gap)

`LocalFileStorage::resolve` (`local.rs:25-33`) rejects `Component::ParentDir` (`..`) and `Component::Prefix` (Windows drive letters). This correctly blocks `foo/../../etc/passwd`, `..`, and absolute `C:\...` paths. ✅

**Gap — MEDIUM `local.rs:26`:** Leading/trailing slashes and NUL bytes are not sanitized. `path.trim_start_matches('/')` turns `/etc/passwd` into `etc/passwd` (sandboxed — safe), but a NUL byte (`foo\0bar`) is passed straight to `tokio::fs::File::open`/`create`. On Unix this is benign, but it is passed unmodified to `resolve`. Not exploited, but inconsistent with the documented "rejected path" posture. Also `..` *inside* a single filename like `foo/./../bar` is decomposed by `components()` and caught — good.

No actual traversal found. The existing `path_traversal_is_rejected` test (`local.rs:156-164`) covers `../escape.txt` only; add coverage for `%2e%2e` encoded input and NUL.

## 2. File handle leaks — `local.rs` (NO OS-handle leak; durability + cleanup gaps)

There is **no raw file-descriptor leak**: every `tokio::fs::File` is a Rust local that is dropped (and closed) when its function returns — including the `copy`-error path in `put` (`local.rs:57-63`), where the `?` drops `file`. ✅

However two related gaps exist:

- **MEDIUM — `local.rs:60-62` (no flush/sync):** `put` calls `tokio::io::copy(&mut data, &mut file)` then returns `Ok(written)` with **no `file.flush().await`** and no `sync_all`. The data sits in the kernel page cache; on process/crash before close it may be lost, and the returned `written` count reflects bytes handed to `copy`, not bytes durably on disk. For an object store promising "written", this is a durability gap.

- **MEDIUM — `local.rs:57-63` (partial file on failure):** `File::create` truncates the destination, then if `copy` fails partway the function returns `Err(StorageError::Io)` but a **partially-written, truncated file remains at `full`**. The caller gets an error and a corrupt object on disk. Suggested: write to a temp path (e.g. `full.with_extension("tmp").with_added_extension("partial")`) and `rename` only on success; on error, `remove_file`.

- **LOW — `local.rs:38-48` (get stream lifetime):** `get` returns `ObjectStream` wrapping the open `File`. The handle stays open until the consumer drains/drops the stream. If a caller abandons the stream, the handle is held until the `ObjectStream` is dropped (correct), but the trait offers no explicit "release" — callers must scope-drop the stream. This is inherent to streaming APIs, but the doc on `StorageDriver::get` should warn the caller they own the handle lifetime.

## 3. Multipart upload correctness — `s3.rs` (PASS with validation gap)

`S3Storage::put` (`s3.rs:100-183`) and `upload_parts` (`s3.rs:238-291`) implement the standard **read-first-chunk → 1-part PutObject vs. multipart** pattern correctly:

- Single-part upload when `filled < MULTIPART_CHUNK_SIZE` (`s3.rs:117`). ✅
- First chunk becomes part 1 of the multipart (`s3.rs:144-151`). ✅
- Loop reads full 8 MiB chunks and uploads each; breaks when a read returns empty. ✅
- On part-upload error, `abort_multipart_upload` is invoked (`s3.rs:171-181`) — proper cleanup. ✅
- `CompletedPart` is built with `e_tag` + `part_number`. ✅
- `complete_multipart_upload` is sent with the accumulated parts (`s3.rs:154-169`). ✅

**Gap — MEDIUM — `s3.rs:248` / `s3.rs:269` (part-number overflow, no client cap):** `part_number` is `i32`, starting at 1 and incremented per part. S3 allows **max 10,000 parts**. With `MULTIPART_CHUNK_SIZE = 8 MiB` (`s3.rs:21`), the silent failure point is **~80 GiB**. At part 10001 the AWS SDK returns a server error; the code maps it to `StorageError::Io` *after* a part has already been uploaded, triggering `abort_multipart_upload` (good — cleanup runs). But the failure should be detected client-side with a clear `StorageError` before attempting an illegal part number, and the constant `8 MiB` (`s3.rs:21`) exceeds S3's *documented minimum* of 5 MiB — the comment says "5 MiB" but the value is `8 * 1024 * 1024`. The comment is wrong.

- **`s3.rs:21` comment mismatch:** `// S3's minimum multipart part size (5 MiB)` but `MULTIPART_CHUNK_SIZE = 8 MiB`. Fix the comment. ✅-ish but misleading docs.

- **LOW — `s3.rs:265`:** `resp.e_tag().unwrap_or_default()` — if S3 omits the ETag (rare), an empty string is sent in `CompletedPart`. Some backends reject empty ETags. Use `unwrap_or("")` explicitly with a TODO, or skip. Minor.

## 4. Error-handling gaps — all backends

### 4.1 Fragile NotFound detection (`s3.rs:295-297`, `gcs.rs:66`, `gcs.rs:125`)

`is_not_found` (`s3.rs:295`):
```rust
fn is_not_found<E: std::fmt::Debug>(e: &E) -> bool {
    format!("{e:?}").contains("NotFound") || format!("{e:?}").contains("404")
}
```
- **`404` substring match** will false-positive on any error whose Debug string mentions "404" (e.g. a 4048-port URL, a `4040`-style code in a message). HIGH risk for incorrect `NotFound` mapping on S3.
- Relies on `Debug`, not typed error matching. The `aws-sdk-s3` v1 `Error` enum exposes typed `NoSuchKey`/`NotFound` via `.kind()` — prefer `e.kind() == Some(ErrorCode::NoSuchKey)` or the `Display`-based `NotFound` variant.

GCS (`gcs.rs:66`, `gcs.rs:125`) uses `e.to_string().contains("404")` — same `404` false-positive problem; GCS errors surface `404` in `Display` for `Status` but a non-NotFound 404-ish message would be mis-mapped to `NotFound`. MEDIUM.

**Recommendation:** match on typed error variants / HTTP status codes, not substring.

### 4.2 Inconsistent `delete` semantics (MEDIUM)

- `local.rs:66-75`: deleting a missing object → `StorageError::NotFound`. ✅ (documented in test)
- `s3.rs:185-194`: `delete_object` on a missing key returns **204 No Content** from S3 (idempotent) → `Ok(())`. ✅ consistent with S3 semantics.
- `gcs.rs:96-105`: `delete_object` on missing → GCS returns **404**, mapped to `StorageError::Io` (via `map_err`), **not** `NotFound`. **Inconsistent**: `GcsStorage::delete` of a missing object errors with `Io`, while `LocalFileStorage::delete` errors with `NotFound`. Callers handling "ignore missing" must special-case. MEDIUM.

### 4.3 `StorageError` has no `Backend`-level detail, only `Io(String)` (LOW)

`traits.rs:17` — `Io(String)` collapses all transport errors into a string. No structured access to the underlying `std::io::Error` (e.g. `ErrorKind::UnexpectedEof`). Limits retry logic. Acceptable for v1, but `map_sdk_err` (`s3.rs:23-25`) re-formats via `{e:?}` (Debug) — callers see a Debug dump, not a clean message. Consider `Display`. LOW.

### 4.4 `stat` size coercion (LOW — `s3.rs:227`)

`resp.content_length().unwrap_or(0).max(0) as u64` — `content_length()` returns `Option<i64>`; `.max(0)` clamps negatives. Fine, but the `.unwrap_or(0)` for a *missing* object is unreachable here because a 404 is mapped to `NotFound` first. OK.

## 5. Feature / API consistency gaps (MEDIUM)

| Operation | `local` (default) | `s3` | `gcs` |
|---|---|---|---|
| `put` streaming | ✅ streaming | ✅ multipart streaming | **❌ buffers whole file in `Vec<u8>`** (`gcs.rs:76-79`) — contradicts crate promise "never holds the whole object in memory" |
| `get` streaming | ✅ file handle | ✅ `into_async_read` | ✅ but actually **downloads full object into `Vec<u8>`** then re-boxes (`gcs.rs:54-72`) — not true streaming |
| Multipart / resumable | n/a | ✅ multipart | ❌ none (simple upload only) |
| `not_impl!` for disabled features | n/a | the trait is always compiled; backends are feature-gated at module level | ✅ |

**`gcs.rs:75-93` `put`:** `data.read_to_end(&mut buf)` materializes the *entire* upload in memory. The module doc (`gcs.rs:4-8`) admits this. For a storage abstraction whose USP is "huge objects never buffered in memory", GCS is the weak backend. Recommend implementing resumable upload (`google-cloud-storage` supports `upload_types::Resumable`) to match S3.

**Feature consistency note:** because `StorageDriver` is in `traits.rs` (always compiled) and `tokio` is declared with only `io-util` (`Cargo.toml`), consumers using `s3`/`gcs` without the `tokio` full feature for their own runtime could hit compile friction. Minor — `Cargo.toml` `dev-dependencies` pulls `tokio = { features = ["full"] }`. LOW.

## 6. Build / clippy

`cargo clippy -p mytheclipse-storage --all-features` → **0 warnings**. ✅
`cargo test -p mytheclipse-storage --all-features` → **5 passed**, 2 ignored (live-only). ✅

---

## Summary table

| # | Area | Severity | Location |
|---|------|----------|----------|
| 1 | No real path traversal (control rejected) | — | `local.rs:25-33` ✅ |
| 2 | No OS file-handle leak (Rust Drop closes files) | — | `local.rs:57-63` ✅ |
| 3 | `put` missing flush/sync (durability) | MEDIUM | `local.rs:60-62` |
| 4 | Failed `put` leaves partial/truncated file | MEDIUM | `local.rs:57-63` |
| 5 | `get` stream handle lifetime not documented | LOW | `local.rs:38-48`, `traits.rs:56-58` |
| 6 | Multipart part-number not capped at 10000 | MEDIUM | `s3.rs:248,269` |
| 7 | Comment says 5 MiB min, value is 8 MiB | LOW | `s3.rs:21` |
| 8 | Fragile `404`/`NotFound` substring matching (S3) | HIGH | `s3.rs:295-297` |
| 9 | Fragile `404` substring matching (GCS) | MEDIUM | `gcs.rs:66,125` |
| 10 | Inconsistent `delete` on missing object (GCS returns `Io`, not `NotFound`) | MEDIUM | `gcs.rs:96-105` vs `local.rs:66-75` |
| 11 | GCS `put`/`get` buffer whole object (no resumable/multipart) | MEDIUM | `gcs.rs:54-93` |
| 12 | `map_sdk_err` uses `Debug` format for messages | LOW | `s3.rs:23-25` |
| 13 | `CompletedPart` ETag `unwrap_or_default()` can be empty | LOW | `s3.rs:265` |
| 14 | `StorageError::Io(String)` loses structured io::Error | LOW | `traits.rs:17-18` |

## Recommended remediation (priority)

1. **(HIGH)** `s3.rs:295` / `gcs.rs:66,125` — replace `format!("{e:?}").contains("404")` with typed error-variant matching (S3 `Error::kind()` / `ErrorCode::NoSuchKey`; GCS `Error::status.code() == 404`).
2. **(MEDIUM)** `s3.rs` — validate `part_number <= 10000` client-side and return a clear `StorageError` before exceeding; fix the `5 MiB` → `8 MiB` comment.
3. **(MEDIUM)** `local.rs` — write to a temp file and `rename` on success; clean up on error (`remove_file`).
4. **(MEDIUM)** `local.rs` — `put` should `file.flush().await` (and optionally `sync_all`) before returning `Ok`.
5. **(MEDIUM)** `gcs.rs` — implement resumable uploads to restore the streaming contract; map GCS 404 on `delete` to `NotFound` for cross-backend consistency.
6. **(LOW)** `s3.rs:265` — handle missing ETag explicitly; `traits.rs` — consider `StorageError::Io` carrying the raw `io::Error` or an `ErrorKind` for retry heuristics.
