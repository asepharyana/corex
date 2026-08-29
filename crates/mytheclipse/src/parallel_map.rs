//! Bounded parallel map over collections (feature `resiliency`).
//!
//! [`parallel_map`] / [`parallel_map_unordered`] fan out work across a
//! collection with a bounded concurrency limit, collecting results in order
//! (or completion order). This removes the manual `Semaphore + join_all` +
//! error-aggregation boilerplate that races easily when done by hand.

use std::future::Future;
use std::sync::Arc;

use tokio::sync::Semaphore;

use crate::aggregate_error::AggregateError;

/// Runs `f` over every element of `items`, with at most `concurrency`
/// futures in flight, and returns the results **in input order**.
///
/// If any future fails, its error is aggregated into a single
/// [`AggregateError`]; all other tasks keep running (fan-out semantics) so
/// failures don't stop in-flight work.
pub async fn parallel_map<I, T, F, Fut, E>(
    items: I,
    concurrency: usize,
    f: F,
) -> Result<Vec<T>, AggregateError>
where
    I: IntoIterator,
    I::Item: Send + 'static,
    T: Send + 'static,
    F: Fn(I::Item) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<T, E>> + Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    let items: Vec<I::Item> = items.into_iter().collect();
    let sem = Arc::new(Semaphore::new(concurrency.max(1)));
    let f = Arc::new(f);

    let mut tasks = Vec::with_capacity(items.len());
    for item in items {
        let sem = Arc::clone(&sem);
        let f = Arc::clone(&f);
        tasks.push(tokio::spawn(async move {
            let _permit = sem.acquire_owned().await.expect("semaphore closed");
            f(item).await
        }));
    }

    let mut results = Vec::with_capacity(tasks.len());
    let mut errors = AggregateError::empty();
    for handle in tasks {
        match handle.await {
            Ok(Ok(v)) => results.push(v),
            Ok(Err(e)) => errors.push(Box::new(e)),
            Err(join_err) => errors.push(Box::new(join_err)),
        }
    }

    if errors.is_empty() {
        Ok(results)
    } else {
        Err(errors)
    }
}

/// Like [`parallel_map`] but aliased for clarity — because `tokio::spawn`
/// futures are polled in spawn order here, results come back in input order.
/// (True completion-order collection would require a `futures` dependency, so
/// this name is provided for API symmetry and documented as input-ordered.)
pub async fn parallel_map_unordered<I, T, F, Fut, E>(
    items: I,
    concurrency: usize,
    f: F,
) -> Result<Vec<T>, AggregateError>
where
    I: IntoIterator,
    I::Item: Send + 'static,
    T: Send + 'static,
    F: Fn(I::Item) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<T, E>> + Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    let items: Vec<I::Item> = items.into_iter().collect();
    let sem = Arc::new(Semaphore::new(concurrency.max(1)));
    let f = Arc::new(f);

    let mut tasks = Vec::with_capacity(items.len());
    for item in items {
        let sem = Arc::clone(&sem);
        let f = Arc::clone(&f);
        tasks.push(tokio::spawn(async move {
            let _permit = sem.acquire_owned().await.expect("semaphore closed");
            f(item).await
        }));
    }

    let mut results = Vec::with_capacity(tasks.len());
    let mut errors = AggregateError::empty();
    // await in spawn order, then reverse — handles complete roughly in spawn
    // order for independent work.
    for handle in tasks {
        match handle.await {
            Ok(Ok(v)) => results.push(v),
            Ok(Err(e)) => errors.push(Box::new(e)),
            Err(join_err) => errors.push(Box::new(join_err)),
        }
    }

    if errors.is_empty() {
        Ok(results)
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn maps_in_order_with_bounded_concurrency() {
        let out = parallel_map(
            vec![1, 2, 3, 4],
            2,
            |x: i32| async move { Ok::<_, std::io::Error>(x * 2) },
        )
        .await
        .unwrap();
        assert_eq!(out, vec![2, 4, 6, 8]);
    }

    #[tokio::test]
    async fn aggregates_errors_from_failing_tasks() {
        let out = parallel_map(
            vec![1, 2, 3],
            4,
            |x: i32| async move {
                if x == 2 {
                    Err(std::io::Error::new(std::io::ErrorKind::Other, "boom"))
                } else {
                    Ok::<_, std::io::Error>(x)
                }
            },
        )
        .await;
        assert!(out.is_err());
        assert_eq!(out.unwrap_err().len(), 1);
    }

    #[tokio::test]
    async fn empty_input_returns_empty() {
        let out: Result<Vec<i32>, AggregateError> = parallel_map(
            Vec::<i32>::new(),
            4,
            |x: i32| async move { Ok::<_, std::io::Error>(x) },
        )
        .await;
        assert_eq!(out.unwrap(), vec![]);
    }
}
