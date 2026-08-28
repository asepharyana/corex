//! The core [`StorageDriver`] trait and supporting types.

use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::SystemTime;

use async_trait::async_trait;
use tokio::io::{AsyncRead, ReadBuf};

/// Errors returned by storage operations.
#[derive(Debug)]
pub enum StorageError {
    /// The object does not exist.
    NotFound(String),
    /// The path was rejected (e.g. attempted directory traversal).
    InvalidPath(String),
    /// An I/O or backend transport error occurred.
    Io(String),
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(p) => write!(f, "object not found: {p}"),
            Self::InvalidPath(p) => write!(f, "invalid path: {p}"),
            Self::Io(s) => write!(f, "storage io error: {s}"),
        }
    }
}

impl std::error::Error for StorageError {}

/// A boxed, unpin, streaming byte source used for both `get` results and
/// `put` input, so large objects never need to be buffered in memory whole.
pub type ObjectStream = Pin<Box<dyn AsyncRead + Send + Unpin>>;

/// Metadata about a stored object.
#[derive(Debug, Clone)]
pub struct ObjectMeta {
    /// Size in bytes.
    pub size: u64,
    /// A backend-provided content hash/version tag, if available.
    pub etag: Option<String>,
    /// Last-modified timestamp, if available.
    pub last_modified: Option<SystemTime>,
}

/// A unified interface for storing, reading, and deleting files regardless of
/// the physical backend (local disk, S3/MinIO, Google Cloud Storage, ...).
///
/// All operations are stream-based: [`StorageDriver::put`] takes an
/// [`ObjectStream`] and [`StorageDriver::get`] returns one, so a caller
/// forwarding a large upload/download never has to hold the whole object in
/// memory.
#[async_trait]
pub trait StorageDriver: Send + Sync {
    /// Opens `path` for streaming read.
    async fn get(&self, path: &str) -> Result<ObjectStream, StorageError>;

    /// Writes `data` to `path`, returning the number of bytes written.
    async fn put(&self, path: &str, data: ObjectStream) -> Result<u64, StorageError>;

    /// Removes the object at `path`.
    async fn delete(&self, path: &str) -> Result<(), StorageError>;

    /// Whether an object exists at `path`.
    async fn exists(&self, path: &str) -> Result<bool, StorageError>;

    /// Fetches metadata about the object at `path` without downloading it.
    async fn stat(&self, path: &str) -> Result<ObjectMeta, StorageError>;
}

/// A minimal, dependency-free in-memory [`AsyncRead`] over an owned buffer.
struct VecCursor {
    data: Vec<u8>,
    pos: usize,
}

impl AsyncRead for VecCursor {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let remaining = &self.data[self.pos..];
        let n = remaining.len().min(buf.remaining());
        buf.put_slice(&remaining[..n]);
        self.pos += n;
        Poll::Ready(Ok(()))
    }
}

/// Convenience: wraps an owned `Vec<u8>` as an [`ObjectStream`] for
/// [`StorageDriver::put`].
pub fn bytes_stream(data: Vec<u8>) -> ObjectStream {
    Box::pin(VecCursor { data, pos: 0 })
}

/// Convenience: drains an [`ObjectStream`] into an owned `Vec<u8>` (defeats
/// the point of streaming for huge objects — intended for tests/small files).
pub async fn read_to_vec(mut stream: ObjectStream) -> Result<Vec<u8>, StorageError> {
    use tokio::io::AsyncReadExt;
    let mut buf = Vec::new();
    stream
        .read_to_end(&mut buf)
        .await
        .map_err(|e| StorageError::Io(e.to_string()))?;
    Ok(buf)
}
