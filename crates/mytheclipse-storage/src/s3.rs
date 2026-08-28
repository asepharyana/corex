//! An S3-compatible [`StorageDriver`] (feature `s3`), via `aws-sdk-s3`.
//!
//! Works against Amazon S3 or any S3-compatible endpoint — pass a custom
//! endpoint via [`S3Storage::connect_with_endpoint`] to target MinIO or
//! another compatible service.
//!
//! [`put`](S3Storage::put) streams the input in fixed-size chunks through
//! S3's multipart upload API rather than buffering the whole object in
//! memory, so large uploads stay within a bounded memory footprint.

use async_trait::async_trait;
use aws_sdk_s3::config::{Credentials, Region};
use aws_sdk_s3::primitives::ByteStream as AwsByteStream;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};
use aws_sdk_s3::Client;
use tokio::io::AsyncReadExt;

use crate::traits::{ObjectMeta, ObjectStream, StorageDriver, StorageError};

/// S3's minimum multipart part size (5 MiB), except for the final part.
const MULTIPART_CHUNK_SIZE: usize = 8 * 1024 * 1024;

fn map_sdk_err<E: std::fmt::Debug>(e: E) -> StorageError {
    StorageError::Io(format!("{e:?}"))
}

/// A [`StorageDriver`] backed by an S3 (or S3-compatible) bucket.
#[derive(Clone)]
pub struct S3Storage {
    client: Client,
    bucket: String,
}

impl S3Storage {
    /// Connects using the ambient AWS environment/credential chain
    /// (`AWS_ACCESS_KEY_ID`, IAM role, profile, etc.) against real S3.
    pub async fn connect(bucket: impl Into<String>) -> Self {
        let config = aws_config::load_from_env().await;
        let client = Client::new(&config);
        Self {
            client,
            bucket: bucket.into(),
        }
    }

    /// Connects to a custom S3-compatible endpoint (e.g. MinIO) with static
    /// credentials and path-style addressing.
    pub async fn connect_with_endpoint(
        bucket: impl Into<String>,
        endpoint: &str,
        region: &str,
        access_key: &str,
        secret_key: &str,
    ) -> Self {
        let credentials = Credentials::new(access_key, secret_key, None, None, "mytheclipse-storage");
        let config = aws_sdk_s3::Config::builder()
            .region(Region::new(region.to_string()))
            .endpoint_url(endpoint)
            .credentials_provider(credentials)
            .force_path_style(true)
            .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
            .build();
        Self {
            client: Client::from_conf(config),
            bucket: bucket.into(),
        }
    }

    /// Wraps an already-configured client (advanced use: sharing a client
    /// across multiple `S3Storage` instances pointed at different buckets).
    pub fn from_client(client: Client, bucket: impl Into<String>) -> Self {
        Self {
            client,
            bucket: bucket.into(),
        }
    }
}

#[async_trait]
impl StorageDriver for S3Storage {
    async fn get(&self, path: &str) -> Result<ObjectStream, StorageError> {
        let resp = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(path)
            .send()
            .await
            .map_err(|e| {
                if is_not_found(&e) {
                    StorageError::NotFound(path.to_string())
                } else {
                    map_sdk_err(e)
                }
            })?;
        Ok(Box::pin(resp.body.into_async_read()))
    }

    async fn put(&self, path: &str, mut data: ObjectStream) -> Result<u64, StorageError> {
        // Read the first chunk to decide between a simple `PutObject` (small
        // objects) and a streamed multipart upload (anything larger).
        let mut first_chunk = vec![0u8; MULTIPART_CHUNK_SIZE];
        let mut filled = 0usize;
        while filled < first_chunk.len() {
            let n = data
                .read(&mut first_chunk[filled..])
                .await
                .map_err(|e| StorageError::Io(e.to_string()))?;
            if n == 0 {
                break;
            }
            filled += n;
        }
        first_chunk.truncate(filled);

        if filled < MULTIPART_CHUNK_SIZE {
            // The whole object fit in one chunk: a single PutObject suffices.
            let len = first_chunk.len() as u64;
            self.client
                .put_object()
                .bucket(&self.bucket)
                .key(path)
                .body(AwsByteStream::from(first_chunk))
                .send()
                .await
                .map_err(map_sdk_err)?;
            return Ok(len);
        }

        // Larger objects: stream the rest through multipart upload.
        let create = self
            .client
            .create_multipart_upload()
            .bucket(&self.bucket)
            .key(path)
            .send()
            .await
            .map_err(map_sdk_err)?;
        let upload_id = create
            .upload_id()
            .ok_or_else(|| StorageError::Io("S3 did not return an upload id".to_string()))?;

        let result = upload_parts(
            &self.client,
            &self.bucket,
            path,
            upload_id,
            first_chunk,
            &mut data,
        )
        .await;

        match result {
            Ok((total, parts)) => {
                self.client
                    .complete_multipart_upload()
                    .bucket(&self.bucket)
                    .key(path)
                    .upload_id(upload_id)
                    .multipart_upload(
                        CompletedMultipartUpload::builder()
                            .set_parts(Some(parts))
                            .build(),
                    )
                    .send()
                    .await
                    .map_err(map_sdk_err)?;
                Ok(total)
            }
            Err(e) => {
                let _ = self
                    .client
                    .abort_multipart_upload()
                    .bucket(&self.bucket)
                    .key(path)
                    .upload_id(upload_id)
                    .send()
                    .await;
                Err(e)
            }
        }
    }

    async fn delete(&self, path: &str) -> Result<(), StorageError> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(path)
            .send()
            .await
            .map_err(map_sdk_err)?;
        Ok(())
    }

    async fn exists(&self, path: &str) -> Result<bool, StorageError> {
        match self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(path)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(e) if is_not_found(&e) => Ok(false),
            Err(e) => Err(map_sdk_err(e)),
        }
    }

    async fn stat(&self, path: &str) -> Result<ObjectMeta, StorageError> {
        let resp = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(path)
            .send()
            .await
            .map_err(|e| {
                if is_not_found(&e) {
                    StorageError::NotFound(path.to_string())
                } else {
                    map_sdk_err(e)
                }
            })?;
        Ok(ObjectMeta {
            size: resp.content_length().unwrap_or(0).max(0) as u64,
            etag: resp.e_tag().map(|s| s.to_string()),
            last_modified: resp
                .last_modified()
                .and_then(|d| d.to_owned().try_into().ok()),
        })
    }
}

/// Uploads `first_chunk` as part 1, then continues reading `data` in
/// [`MULTIPART_CHUNK_SIZE`] chunks until exhausted, uploading each as a part.
async fn upload_parts(
    client: &Client,
    bucket: &str,
    key: &str,
    upload_id: &str,
    first_chunk: Vec<u8>,
    data: &mut ObjectStream,
) -> Result<(u64, Vec<CompletedPart>), StorageError> {
    let mut total = 0u64;
    let mut parts = Vec::new();
    let mut part_number = 1i32;
    let mut chunk = first_chunk;

    loop {
        total += chunk.len() as u64;
        let resp = client
            .upload_part()
            .bucket(bucket)
            .key(key)
            .upload_id(upload_id)
            .part_number(part_number)
            .body(AwsByteStream::from(chunk))
            .send()
            .await
            .map_err(map_sdk_err)?;
        parts.push(
            CompletedPart::builder()
                .e_tag(resp.e_tag().unwrap_or_default())
                .part_number(part_number)
                .build(),
        );
        part_number += 1;

        let mut next = vec![0u8; MULTIPART_CHUNK_SIZE];
        let mut filled = 0usize;
        while filled < next.len() {
            let n = data
                .read(&mut next[filled..])
                .await
                .map_err(|e| StorageError::Io(e.to_string()))?;
            if n == 0 {
                break;
            }
            filled += n;
        }
        next.truncate(filled);
        if next.is_empty() {
            break;
        }
        chunk = next;
    }

    Ok((total, parts))
}

/// Best-effort detection of a "not found" S3 error across the SDK's error
/// variants (`GetObject`/`HeadObject` surface this differently).
fn is_not_found<E: std::fmt::Debug>(e: &E) -> bool {
    format!("{e:?}").contains("NotFound") || format!("{e:?}").contains("404")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::{bytes_stream, read_to_vec};

    /// Requires a live S3-compatible endpoint (e.g. MinIO) configured via
    /// `S3_ENDPOINT`, `S3_BUCKET`, `S3_ACCESS_KEY`, `S3_SECRET_KEY`. Run with:
    /// `S3_ENDPOINT=http://127.0.0.1:9000 S3_BUCKET=test S3_ACCESS_KEY=... S3_SECRET_KEY=... \
    ///   cargo test -p mytheclipse-storage --features s3 -- --ignored`.
    #[tokio::test]
    #[ignore = "requires a live S3-compatible endpoint (S3_ENDPOINT, S3_BUCKET, ...)"]
    async fn put_get_roundtrip_live() {
        let endpoint = std::env::var("S3_ENDPOINT").expect("set S3_ENDPOINT");
        let bucket = std::env::var("S3_BUCKET").expect("set S3_BUCKET");
        let access_key = std::env::var("S3_ACCESS_KEY").expect("set S3_ACCESS_KEY");
        let secret_key = std::env::var("S3_SECRET_KEY").expect("set S3_SECRET_KEY");

        let storage = S3Storage::connect_with_endpoint(
            bucket,
            &endpoint,
            "us-east-1",
            &access_key,
            &secret_key,
        )
        .await;

        storage
            .put("mytheclipse-storage-test.txt", bytes_stream(b"hello s3".to_vec()))
            .await
            .unwrap();
        let data = read_to_vec(storage.get("mytheclipse-storage-test.txt").await.unwrap())
            .await
            .unwrap();
        assert_eq!(data, b"hello s3");
        storage.delete("mytheclipse-storage-test.txt").await.unwrap();
    }
}
