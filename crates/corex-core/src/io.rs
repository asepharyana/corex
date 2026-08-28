//! Async I/O task spawning, instrumented with [`tracing`].

use tracing::Instrument;

/// Spawns `future` onto the ambient Tokio runtime, wrapped in a
/// `corex_io_task` tracing span.
///
/// # Panics
///
/// Panics if called outside the context of a running Tokio runtime; corex
/// does not construct or own a runtime of its own, it schedules onto
/// whichever runtime the caller is already inside.
pub fn spawn_io<F>(future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    let span = tracing::info_span!("corex_io_task");
    tokio::spawn(future.instrument(span))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn spawn_io_roundtrips_a_value() {
        let handle = spawn_io(async { 7u32 });
        assert_eq!(handle.await.unwrap(), 7);
    }
}
