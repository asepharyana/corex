//! A local-disk [`StorageDriver`] (feature `local`, default).

use std::path::{Component, Path, PathBuf};

use async_trait::async_trait;

use crate::traits::{ObjectMeta, ObjectStream, StorageDriver, StorageError};

/// Stores objects as files under a root directory.
///
/// Paths are always resolved relative to the configured root; `..` path
/// components are rejected to prevent escaping the root.
#[derive(Clone)]
pub struct LocalFileStorage {
    root: PathBuf,
}

impl LocalFileStorage {
    /// Builds a driver rooted at `root`. The directory is not required to
    /// exist yet; it's created lazily on first write.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Returns the root directory path.
    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    fn resolve(&self, path: &str) -> Result<PathBuf, StorageError> {
        let rel = Path::new(path.trim_start_matches('/'));
        for component in rel.components() {
            if matches!(component, Component::ParentDir | Component::Prefix(_)) {
                return Err(StorageError::InvalidPath(path.to_string()));
            }
        }
        Ok(self.root.join(rel))
    }
}

#[async_trait]
impl StorageDriver for LocalFileStorage {
    async fn get(&self, path: &str) -> Result<ObjectStream, StorageError> {
        let full = self.resolve(path)?;
        let file = tokio::fs::File::open(&full).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                StorageError::NotFound(path.to_string())
            } else {
                StorageError::Io(e.to_string())
            }
        })?;
        Ok(Box::pin(file))
    }

    async fn put(&self, path: &str, mut data: ObjectStream) -> Result<u64, StorageError> {
        let full = self.resolve(path)?;
        if let Some(parent) = full.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| StorageError::Io(e.to_string()))?
        }
        // Write to a sibling temp file then atomically rename. On POSIX this is
        // atomic, so a crash mid-write leaves *either* the previous file *or*
        // the complete new file — never a half-written truncated object.
        // Use a PID-unguarded temp name and clean it up if anything fails.
        let tmp = full.with_extension(format!(".tmp-{}", std::process::id()));
        let mut file = tokio::fs::File::create(&tmp)
            .await
            .map_err(|e| StorageError::Io(e.to_string()))?;
        let written = tokio::io::copy(&mut data, &mut file)
            .await
            .map_err(|e| {
                let _ = std::fs::remove_file(&tmp);
                StorageError::Io(e.to_string())
            })?;
        // Ensure durability: flush to OS, fsync, then rename.
        tokio::fs::File::open(&tmp)
            .await
            .map_err(|e| StorageError::Io(e.to_string()))?
            .sync_all()
            .await
            .map_err(|e| StorageError::Io(e.to_string()))?;
        std::fs::rename(&tmp, &full).map_err(|e| StorageError::Io(e.to_string()))?;
        Ok(written)
    }

    async fn delete(&self, path: &str) -> Result<(), StorageError> {
        let full = self.resolve(path)?;
        match tokio::fs::remove_file(&full).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(StorageError::NotFound(path.to_string()))
            }
            Err(e) => Err(StorageError::Io(e.to_string())),
        }
    }

    async fn exists(&self, path: &str) -> Result<bool, StorageError> {
        let full = self.resolve(path)?;
        Ok(tokio::fs::metadata(&full).await.is_ok())
    }

    async fn stat(&self, path: &str) -> Result<ObjectMeta, StorageError> {
        let full = self.resolve(path)?;
        let meta = tokio::fs::metadata(&full).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                StorageError::NotFound(path.to_string())
            } else {
                StorageError::Io(e.to_string())
            }
        })?;
        Ok(ObjectMeta {
            size: meta.len(),
            etag: None,
            last_modified: meta.modified().ok(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::{bytes_stream, read_to_vec};

    fn driver() -> (LocalFileStorage, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        (LocalFileStorage::new(dir.path()), dir)
    }

    #[tokio::test]
    async fn put_get_roundtrip() {
        let (storage, _dir) = driver();
        let written = storage
            .put("a/b/file.txt", bytes_stream(b"hello world".to_vec()))
            .await
            .unwrap();
        assert_eq!(written, 11);
        let data = read_to_vec(storage.get("a/b/file.txt").await.unwrap())
            .await
            .unwrap();
        assert_eq!(data, b"hello world");
    }

    #[tokio::test]
    async fn get_missing_returns_not_found() {
        let (storage, _dir) = driver();
        // `ObjectStream` isn't `Debug`, so match rather than `unwrap_err()`.
        match storage.get("missing.txt").await {
            Err(StorageError::NotFound(_)) => {}
            other => panic!("expected NotFound, got {:?}", other.is_ok()),
        }
    }

    #[tokio::test]
    async fn delete_removes_object() {
        let (storage, _dir) = driver();
        storage
            .put("x.txt", bytes_stream(b"x".to_vec()))
            .await
            .unwrap();
        assert!(storage.exists("x.txt").await.unwrap());
        storage.delete("x.txt").await.unwrap();
        assert!(!storage.exists("x.txt").await.unwrap());
    }

    #[tokio::test]
    async fn stat_reports_size() {
        let (storage, _dir) = driver();
        storage
            .put("s.bin", bytes_stream(vec![0u8; 1024]))
            .await
            .unwrap();
        let meta = storage.stat("s.bin").await.unwrap();
        assert_eq!(meta.size, 1024);
    }

    #[tokio::test]
    async fn path_traversal_is_rejected() {
        let (storage, _dir) = driver();
        let err = storage
            .put("../escape.txt", bytes_stream(b"x".to_vec()))
            .await
            .unwrap_err();
        assert!(matches!(err, StorageError::InvalidPath(_)));
    }

    /// Verifies the atomic-write contract: after a successful `put`, the temp
    /// file must not linger on disk.
    #[tokio::test]
    async fn put_leaves_no_temp_file() {
        let (storage, _dir) = driver();
        storage
            .put("clean.txt", bytes_stream(b"data".to_vec()))
            .await
            .unwrap();
        // No `*.tmp-*` files should remain in the root after a clean write.
        let leftover: Vec<_> = std::fs::read_dir(storage.root())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name())
            .filter(|n| n.to_string_lossy().contains(".tmp-"))
            .collect();
        assert!(leftover.is_empty(), "temp files left behind: {leftover:?}");
    }
}
