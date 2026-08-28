# mytheclipse-storage

A unified storage & file system abstraction so file handling isn't locked to
one physical location.

- **Local disk** (default) — plain files under a root directory.
- **S3-compatible object storage** (`s3`) — Amazon S3, and any S3-compatible
  endpoint (MinIO included) via a custom endpoint URL.
- **Google Cloud Storage** (`gcs`).

All operations are stream-based, so uploading/downloading a huge object never
requires holding it entirely in memory.

## Usage

```rust
use mytheclipse_storage::{LocalFileStorage, StorageDriver, bytes_stream, read_to_vec};

let storage = LocalFileStorage::new("/var/data");
storage.put("uploads/report.csv", bytes_stream(data)).await?;
let bytes = read_to_vec(storage.get("uploads/report.csv").await?).await?;
```

Swap `LocalFileStorage::new(path)` for `S3Storage::connect("bucket").await?` or
`GcsStorage::connect("bucket").await?` to move to a distributed backend
without touching call sites.
