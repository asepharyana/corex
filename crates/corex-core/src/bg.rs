//! Bounded-concurrency background task execution.

use tracing::Instrument;

use crate::context::context;

/// Spawns `future` as a background task once a concurrency permit is
/// available, returning its [`tokio::task::JoinHandle`].
///
/// At most [`crate::context::EngineContext::bg_concurrency`] background
/// tasks run at any one time; awaiting `spawn_bg` blocks the caller until a
/// slot frees up, which is what provides the bound. A task's permit is held
/// for the task's full lifetime and released automatically when it
/// completes.
///
/// Panic isolation is provided by Tokio itself: a panicking background task
/// cannot crash the runtime or any sibling task, and is surfaced to the
/// caller as `Err(JoinError)` when the returned handle is awaited, exactly
/// as with [`crate::io::spawn_io`].
///
/// # Panics
///
/// Panics if called outside the context of a running Tokio runtime.
pub async fn spawn_bg<F>(future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    let permit = context()
        .bg_semaphore
        .acquire()
        .await
        .expect("corex: bg semaphore closed unexpectedly");
    let span = tracing::info_span!("corex_bg_task");
    tokio::spawn(
        async move {
            let _permit = permit;
            future.await
        }
        .instrument(span),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn spawn_bg_roundtrips_a_value() {
        let handle = spawn_bg(async { 9u32 }).await;
        assert_eq!(handle.await.unwrap(), 9);
    }
}
