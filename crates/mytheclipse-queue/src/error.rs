//! Errors returned by queue and job operations.

/// Errors from queue-level operations (enqueue, dequeue, etc.).
#[derive(Debug)]
pub enum QueueError {
    /// A transport or backend connection error.
    Connection(String),
    /// The requested topic/queue does not exist or is unavailable.
    NotFound(String),
    /// A serialization error.
    Serialization(String),
    /// A timeout occurred while waiting for an operation.
    Timeout,
}

impl std::fmt::Display for QueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connection(s) => write!(f, "queue connection error: {s}"),
            Self::NotFound(s) => write!(f, "queue not found: {s}"),
            Self::Serialization(s) => write!(f, "serialization error: {s}"),
            Self::Timeout => write!(f, "queue operation timed out"),
        }
    }
}

impl std::error::Error for QueueError {}

/// Errors from individual job processing.
#[derive(Debug)]
pub enum JobError {
    /// The job could not be acknowledged.
    AckFailed(String),
    /// The job could not be moved to the dead-letter queue.
    DlqFailed(String),
    /// The job exceeded its maximum retry count.
    MaxRetriesExceeded,
    /// The job payload could not be decoded.
    InvalidPayload(String),
}

impl std::fmt::Display for JobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AckFailed(s) => write!(f, "ack failed: {s}"),
            Self::DlqFailed(s) => write!(f, "dlq move failed: {s}"),
            Self::MaxRetriesExceeded => write!(f, "max retries exceeded"),
            Self::InvalidPayload(s) => write!(f, "invalid payload: {s}"),
        }
    }
}

impl std::error::Error for JobError {}
