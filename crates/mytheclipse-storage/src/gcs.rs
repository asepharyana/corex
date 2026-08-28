//! A Google Cloud Storage [`StorageDriver`] (feature `gcs`), via
//! `google-cloud-storage`.
//!
//! [`get`](GcsStorage::get) streams the downloaded object; [`put`](GcsStorage::put)
//! currently buffers the input before uploading in a single request (the
//! `google-cloud-storage` crate's simple upload API takes an owned body). For
//! very large uploads prefer chunked application-level batching until this
//! crate grows resumable-upload support.

use async_trait::async_trait;
use google_cloud_storage::client::{Client, ClientConfig};
use google_cloud_storage::http::objects::delete::DeleteObjectRequest;
use google_cloud_storage::http::objects::download::Range;
use google_cloud_storage::http::objects::get::GetObjectRequest;
use google_cloud_storage::http::objects::upload::{Media, UploadObjectRequest, UploadType};
use tokio::io::AsyncReadExt;

use crate::traits::{ObjectMeta, ObjectStream, StorageDriver, StorageError};

fn map_err<E: std::fmt::Display>(e: E) -> StorageError {
    StorageError::Io(e.to_string())
}

/// A [`StorageDriver`] backed by a Google Cloud Storage bucket.
#[derive(Clone)]
pub struct GcsStorage {
    client: Client,
    bucket: String,
}

impl GcsStorage {
    /// Connects using Application Default Credentials (`GOOGLE_APPLICATION_CREDENTIALS`,
    /// workload identity, etc.).
    pub async fn connect(bucket: impl Into<String>) -> Result<Self, StorageError> {
        let config = ClientConfig::default().with_auth().await.map_err(map_err)?;
        Ok(Self {
            client: Client::new(config),
            bucket: bucket.into(),
        })
    }

    /// Wraps an already-configured client.
    pub fn from_client(client: Client, bucket: impl Into<String>) -> Self {
        Self {
            client,
            bucket: bucket.into(),
        }
    }
}

#[async_trait]
impl StorageDriver for GcsStorage {
    async fn get(&self, path: &str) -> Result<ObjectStream, StorageError> {
        let bytes = self
            .client
            .download_object(
                &GetObjectRequest {
                    bucket: self.bucket.clone(),
                    object: path.to_string(),
                    ..Default::default()
                },
                &Range::default(),
            )
            .await
            .map_err(|e| {
                if e.to_string().contains("404") {
                    StorageError::NotFound(path.to_string())
                } else {
                    map_err(e)
                }
            })?;
        Ok(crate::traits::bytes_stream(bytes))
    }

    async fn put(&self, path: &str, mut data: ObjectStream) -> Result<u64, StorageError> {
        let mut buf = Vec::new();
        data.read_to_end(&mut buf)
            .await
            .map_err(|e| StorageError::Io(e.to_string()))?;
        let len = buf.len() as u64;
        let upload_type = UploadType::Simple(Media::new(path.to_string()));
        self.client
            .upload_object(
                &UploadObjectRequest {
                    bucket: self.bucket.clone(),
                    ..Default::default()
                },
                buf,
                &upload_type,
            )
            .await
            .map_err(map_err)?;
        Ok(len)
    }

    async fn delete(&self, path: &str) -> Result<(), StorageError> {
        self.client
            .delete_object(&DeleteObjectRequest {
                bucket: self.bucket.clone(),
                object: path.to_string(),
                ..Default::default()
            })
            .await
            .map_err(map_err)
    }

    async fn exists(&self, path: &str) -> Result<bool, StorageError> {
        match self.stat(path).await {
            Ok(_) => Ok(true),
            Err(StorageError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    async fn stat(&self, path: &str) -> Result<ObjectMeta, StorageError> {
        let obj = self
            .client
            .get_object(&GetObjectRequest {
                bucket: self.bucket.clone(),
                object: path.to_string(),
                ..Default::default()
            })
            .await
            .map_err(|e| {
                if e.to_string().contains("404") {
                    StorageError::NotFound(path.to_string())
                } else {
                    map_err(e)
                }
            })?;
        Ok(ObjectMeta {
            size: obj.size.max(0) as u64,
            etag: Some(obj.etag),
            last_modified: obj.updated.map(|t| t.into()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::{bytes_stream, read_to_vec};

    /// Requires a live GCS bucket with Application Default Credentials
    /// configured, and `GCS_BUCKET` set. Run with:
    /// `GCS_BUCKET=my-bucket cargo test -p mytheclipse-storage --features gcs -- --ignored`.
    #[tokio::test]
    #[ignore = "requires a live GCS bucket + Application Default Credentials"]
    async fn put_get_roundtrip_live() {
        let bucket = std::env::var("GCS_BUCKET").expect("set GCS_BUCKET");
        let storage = GcsStorage::connect(bucket).await.unwrap();
        storage
            .put(
                "mytheclipse-storage-test.txt",
                bytes_stream(b"hello gcs".to_vec()),
            )
            .await
            .unwrap();
        let data = read_to_vec(storage.get("mytheclipse-storage-test.txt").await.unwrap())
            .await
            .unwrap();
        assert_eq!(data, b"hello gcs");
        storage.delete("mytheclipse-storage-test.txt").await.unwrap();
    }
}
