//! Multipart upload trait for large-object uploads in parallel parts.

use async_trait::async_trait;

use crate::ObjectStream;

/// A single part of a multipart upload.
#[derive(Debug, Clone)]
pub struct UploadPart {
    pub part_number: u32,
    pub data: Vec<u8>,
}

/// A handle for an in-progress multipart upload.
pub struct MultipartUpload {
    upload_id: String,
    path: String,
    parts: Vec<UploadPart>,
}

impl MultipartUpload {
    /// Creates a new multipart upload handle.
    pub fn new(upload_id: String, path: String) -> Self {
        Self {
            upload_id,
            path,
            parts: Vec::new(),
        }
    }

    /// Adds a part to the upload.
    pub fn add_part(&mut self, part_number: u32, data: Vec<u8>) {
        self.parts.push(UploadPart { part_number, data });
    }

    /// Returns the number of parts staged so far.
    pub fn part_count(&self) -> usize {
        self.parts.len()
    }

    /// Returns the upload ID.
    pub fn upload_id(&self) -> &str {
        &self.upload_id
    }

    /// Returns the destination path.
    pub fn path(&self) -> &str {
        &self.path
    }
}

/// Trait for backends supporting multipart uploads.
#[async_trait]
pub trait MultipartUploadDriver: Send + Sync {
    /// Initiates a multipart upload.
    async fn init_multipart(&self, path: &str) -> Result<MultipartUpload, String>;

    /// Uploads a single part.
    async fn upload_part(
        &self,
        upload_id: &str,
        path: &str,
        part_number: u32,
        data: ObjectStream,
    ) -> Result<u64, String>;

    /// Completes the multipart upload.
    async fn complete_multipart(&self, upload_id: &str, path: &str) -> Result<(), String>;

    /// Aborts the multipart upload.
    async fn abort_multipart(&self, upload_id: &str, path: &str) -> Result<(), String>;
}
