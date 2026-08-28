//! Hard execution time bounds for async work (feature `resiliency`).
//!
//! Provides [`with_timeout`] (a convenience that resolves a future to a
//! [`Result`] with an elapsed-vs-completed outcome) and [`Timeout`], a
//! stand-alone [`Future`] wrapper you can build once and hand to
//! [`crate::io::spawn_io`] / [`crate::bg::spawn_bg`]/`tokio::spawn` so the
//! bound is enforced wherever the future actually runs.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use tracing::Instrument;

use crate::error::MytheclipseError;

/// The outcome of timing out a single future.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeoutError {
    /// The deadline elapsed before the future completed.
    Elapsed,
}

impl std::fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Elapsed => write!(f, "deadline elapsed"),
        }
    }
}

impl std::error::Error for TimeoutError {}

/// Runs `future` to completion, giving up after `dur` and returning
/// [`TimeoutError::Elapsed`] if the bound is exceeded.
///
/// The future is cancelled on timeout: its task is aborted when the deadline
/// fires, so no lingering resource is pinned. The call is wrapped in a
/// `mytheclipse_timeout_task` tracing span.
///
/// # Panics
///
/// Panics if called outside the context of a running Tokio runtime.
pub async fn with_timeout<T, F>(dur: Duration, future: F) -> Result<T, TimeoutError>
where
    F: Future<Output = T>,
{
    let span = tracing::info_span!("mytheclipse_timeout_task");
    match tokio::time::timeout(dur, future.instrument(span)).await {
        Ok(value) => Ok(value),
        Err(_) => Err(TimeoutError::Elapsed),
    }
}

/// A future that enforces a [`Duration`] deadline on an inner future.
///
/// Unlike [`with_timeout`], this returns an invocable [`Future`] rather than
/// polling to completion, so it can be constructed ahead of time and spawned
/// through any executor:
///
/// ```no_run
/// use mytheclipse::timeout::Timeout;
/// # fn _r() {
/// let bounded: Timeout<_> = Timeout::new(
///     std::time::Duration::from_secs(1),
///     async { "done" },
/// );
/// # }
/// ```
///
/// If the deadline elapses first, the completed output is `Err(TimeoutError)`.
pub struct Timeout<T> {
    inner: Pin<Box<dyn Future<Output = Result<T, TimeoutError>> + Send>>,
}

impl<T: Send + 'static> Timeout<T> {
    /// Wraps `future` with a deadline of `dur`.
    pub fn new<F>(dur: Duration, future: F) -> Self
    where
        F: Future<Output = T> + Send + 'static,
    {
        let future = async move {
            match tokio::time::timeout(dur, future).await {
                Ok(value) => Ok(value),
                Err(_) => Err(TimeoutError::Elapsed),
            }
        };
        Self {
            inner: Box::pin(future),
        }
    }
}

impl<T> Future for Timeout<T> {
    type Output = Result<T, TimeoutError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // `inner` is a `Pin<Box<...>>`. Because `Box` is `Unpin`, calling
        // `as_mut` on the boxed pin yields `Pin<&mut (dyn Future + Send)>`
        // which polls the inner future. Pinning is upheld by the box, so no
        // unsafe projection is needed.
        self.get_mut().inner.as_mut().poll(cx)
    }
}

/// A hard deadline for a single future, returning `Result<T, MytheclipseError>`.
///
/// Convenience twin of [`with_timeout`] that maps the outcome onto the crate's
/// shared error type, e.g. for use in code that already returns
/// [`MytheclipseError`].
pub async fn timeout<T, F>(dur: Duration, future: F) -> Result<T, MytheclipseError>
where
    F: Future<Output = T>,
{
    with_timeout(dur, future)
        .await
        .map_err(|_| MytheclipseError::Timeout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn completes_within_bound_returns_value() {
        let value = with_timeout(Duration::from_secs(5), async { 42u32 }).await;
        assert_eq!(value.unwrap(), 42);
    }

    #[tokio::test]
    async fn exceeding_bound_yields_elapsed() {
        let outcome = with_timeout(Duration::from_millis(20), async {
            tokio::time::sleep(Duration::from_secs(5)).await;
            42u32
        })
        .await;
        assert_eq!(outcome, Err(TimeoutError::Elapsed));
    }

    #[tokio::test]
    async fn timeout_wrapper_maps_to_shared_error() {
        let outcome = timeout(Duration::from_millis(10), async {
            tokio::time::sleep(Duration::from_secs(5)).await;
        })
        .await;
        assert_eq!(outcome, Err(MytheclipseError::Timeout));
    }

    #[tokio::test]
    async fn timeout_future_is_spawnable() {
        let bounded = Timeout::new(Duration::from_secs(5), async { 7u32 });
        assert_eq!(tokio::spawn(bounded).await.unwrap().unwrap(), 7);
    }
}
