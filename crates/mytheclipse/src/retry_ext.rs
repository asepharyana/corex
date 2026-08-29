//! Ergonomic retry extension trait (feature `resiliency`).
//!
//! [`RetryExt`] extends any [`std::future::Future<Output = Result<T, E>>`]
//! with a `.retry()` method that delegates to [`crate::retry::retry`].

use std::future::Future;
use std::pin::Pin;

use crate::retry::{retry, RetryConfig, RetryError};

/// Extension trait adding ergonomic `.retry()` to any fallible future.
///
/// ```
/// use std::time::Duration;
/// use mytheclipse::retry_ext::RetryExt;
/// use mytheclipse::retry::RetryConfig;
///
/// #[tokio::main]
/// async fn main() {
///     let cfg = RetryConfig { max_attempts: 3, base_delay: Duration::from_millis(1), ..RetryConfig::default() };
///     let attempts = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
///     let a = std::sync::Arc::clone(&attempts);
///
///     let op = move || {
///         let a = std::sync::Arc::clone(&a);
///         async move {
///             if a.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 2 {
///                 Err::<(), String>("transient".into())
///             } else {
///                 Ok(())
///             }
///         }
///     };
///
///     let result = async { Err::<(), String>("first".into()) }
///         .retry(cfg, |_: &String| true, op)
///         .await;
///     assert!(result.is_ok());
///     assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 3);
/// }
/// ```
pub trait RetryExt<T, E>: Future<Output = Result<T, E>> + Sized + 'static
where
    E: std::fmt::Debug + Send + 'static,
    T: Send + 'static,
{
    /// Retries the future's result via a reconstructive `self_fn` closure,
    /// delegating to [`crate::retry::retry`]. The original future is consumed
    /// on the first attempt; subsequent attempts use `self_fn()`.
    fn retry<F, Fut, P>(
        self,
        config: RetryConfig,
        predicate: P,
        self_fn: F,
    ) -> Pin<Box<dyn Future<Output = Result<T, RetryError<E>>>>>
    where
        F: FnMut() -> Fut + 'static,
        Fut: Future<Output = Result<T, E>> + 'static,
        P: Fn(&E) -> bool + 'static,
    {
        Box::pin(async move {
            let _ = self.await;
            retry(config, predicate, self_fn).await
        })
    }
}

impl<Fut, T, E> RetryExt<T, E> for Fut
where
    Fut: Future<Output = Result<T, E>> + Send + 'static,
    E: std::fmt::Debug + Send + 'static,
    T: Send + 'static,
{
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test]
    async fn retry_ext_retries_then_succeeds() {
        let attempts = Arc::new(AtomicU32::new(0));
        let a = Arc::clone(&attempts);
        let cfg = RetryConfig {
            max_attempts: 3,
            base_delay: Duration::from_millis(1),
            ..RetryConfig::default()
        };

        let op = move || {
            let a = Arc::clone(&a);
            async move {
                let n = a.fetch_add(1, Ordering::SeqCst);
                if n < 2 {
                    Err::<(), String>("transient".into())
                } else {
                    Ok(())
                }
            }
        };

        let fut = async { Err::<(), String>("first".into()) };
        let result = fut.retry(cfg, |_: &String| true, op).await;
        assert!(result.is_ok());
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }
}
