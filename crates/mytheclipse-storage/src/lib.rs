//! # mytheclipse-storage
//!
//! A unified storage & file system abstraction so file handling doesn't get
//! locked to one physical storage location.
//!
//! - **Local disk** ([`local::LocalFileStorage`], feature `local`, default).
//! - **S3-compatible object storage** ([`s3::S3Storage`], feature `s3`) —
//!   works with Amazon S3 and any S3-compatible endpoint, including MinIO
//!   (pass a custom endpoint via [`s3::S3Storage::connect_with_endpoint`]).
//! - **Google Cloud Storage** ([`gcs::GcsStorage`], feature `gcs`).
//!
//! All operations are stream-based (an [`traits::ObjectStream`] is a boxed
//! [`tokio::io::AsyncRead`]), so uploading/downloading a huge object never
//! requires holding it entirely in memory.
//!
//! ## Example
//!
//! Local disk storage (`local` feature, default):
//!
//! ```no_run
//! use mytheclipse_storage::{StorageDriver, bytes_stream, read_to_vec};
//! # #[cfg(feature = "local")]
//! # #[tokio::main]
//! # async fn main() {
//! use mytheclipse_storage::LocalFileStorage;
//! # let dir = tempfile::tempdir().unwrap();
//! let storage = LocalFileStorage::new(dir.path());
//! storage.put("hello.txt", bytes_stream(b"hi".to_vec())).await.unwrap();
//! let data = read_to_vec(storage.get("hello.txt").await.unwrap()).await.unwrap();
//! assert_eq!(data, b"hi");
//! # }
//! # #[cfg(not(feature = "local"))]
//! # fn main() {}
//! ```

pub mod traits;

#[cfg(feature = "local")]
pub mod local;

#[cfg(feature = "s3")]
pub mod s3;

#[cfg(feature = "gcs")]
pub mod gcs;

pub use traits::{
    bytes_stream, read_to_vec, ObjectMeta, ObjectStream, StorageDriver, StorageError,
};

#[cfg(feature = "local")]
pub use local::LocalFileStorage;

#[cfg(feature = "s3")]
pub use s3::S3Storage;

#[cfg(feature = "gcs")]
pub use gcs::GcsStorage;
