//! Panic-isolated heavy compute on a sized [`rayon::ThreadPool`].

use std::panic::{catch_unwind, AssertUnwindSafe};

use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::context::context;
use crate::error::MytheclipseError;

/// Runs `f` on the global compute thread pool and returns its result.
///
/// If `f` panics, the panic is caught and converted into
/// [`MytheclipseError::ComputePanic`] instead of unwinding across the pool
/// boundary or poisoning the pool; subsequent calls to [`compute`] continue
/// to work normally.
///
/// # Panic-safety caveat
///
/// `f` is wrapped in [`AssertUnwindSafe`] so that closures capturing
/// ordinary references or non-[`UnwindSafe`](std::panic::UnwindSafe) state
/// can be submitted without a compile error. This is sound with respect to
/// the compute pool itself, since a panicking closure's stack (and any
/// locals it holds) is discarded entirely rather than observed afterward.
/// It does not, however, guarantee exception-safety of state the closure
/// captured by mutable reference: if `f` panics partway through mutating a
/// captured `&mut T`, that `T` may be left in an inconsistent state from
/// the caller's perspective.
pub fn compute<F, R>(f: F) -> Result<R, MytheclipseError>
where
    F: FnOnce() -> R + Send,
    R: Send,
{
    let wrapped = AssertUnwindSafe(f);
    context()
        .compute_pool
        .install(move || catch_unwind(wrapped))
        .map_err(|payload| MytheclipseError::ComputePanic(panic_payload_to_string(payload)))
}

fn panic_payload_to_string(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "compute closure panicked with a non-string payload".to_string()
    }
}

/// Errors collected while mapping over items on the CPU compute pool.
///
/// Each entry is one item's error message (from an `Err` return or a panic).
/// Unlike [`crate::aggregate_error::AggregateError`], this lives under the
/// `compute` feature only and carries plain strings, so the compute pool
/// primitives compile without the `resiliency` feature.
#[derive(Debug, Default)]
pub struct ComputeErrors {
    errors: Vec<String>,
}

impl ComputeErrors {
    /// Number of items that failed (returned `Err` or panicked).
    pub fn len(&self) -> usize {
        self.errors.len()
    }

    /// `true` when every item succeeded.
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// Iterate over the error messages.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.errors.iter().map(String::as_str)
    }
}

impl std::fmt::Display for ComputeErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} compute item(s) failed:", self.errors.len())?;
        for e in &self.errors {
            write!(f, "\n  - {e}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ComputeErrors {}

/// Runs `f` over every item **on the rayon compute pool**, in parallel,
/// returning the results **in input order**.
///
/// This is the CPU-specific counterpart to [`crate::parallel_map::parallel_map`]:
/// work is distributed across the compute pool's worker threads (sized from
/// [`crate::runtime_auto::RuntimeConfig::auto`] — one per logical core) by
/// rayon's work-stealing scheduler, so concurrency is bounded by the pool and
/// **auto-scales to the host CPU** without a manual `concurrency` parameter.
///
/// Each item's closure runs inside [`std::panic::catch_unwind`]: a panic is
/// caught and recorded as an error instead of unwinding across the pool. All
/// errors (returned or panicked) are collected into a single
/// [`ComputeErrors`].
///
/// ```
/// use mytheclipse::compute::compute_map;
///
/// let squares = compute_map(vec![1u32, 2, 3, 4], |x| Ok::<_, String>(x * x)).unwrap();
/// assert_eq!(squares, vec![1, 4, 9, 16]);
///
/// // Failures are aggregated, order is preserved:
/// let out = compute_map(vec![1, 2, 3], |x| {
///     if x == 2 { Err("boom".to_string()) } else { Ok(x * 10) }
/// }).unwrap_err();
/// assert_eq!(out.len(), 1);
/// ```
pub fn compute_map<I, T, F>(items: I, f: F) -> Result<Vec<T>, ComputeErrors>
where
    I: IntoParallelIterator + Send,
    I::Item: Send,
    T: Send,
    F: Fn(I::Item) -> Result<T, String> + Send + Sync,
{
    let wrapped = AssertUnwindSafe(f);
    let collected: Vec<Result<T, String>> = context().compute_pool.install(move || {
        let f = wrapped;
        items
            .into_par_iter()
            .map(|item| {
                catch_unwind(AssertUnwindSafe(|| f(item)))
                    .unwrap_or_else(|payload| Err(panic_payload_to_string(payload)))
            })
            .collect()
    });

    let mut values = Vec::with_capacity(collected.len());
    let mut errors = Vec::new();
    for r in collected {
        match r {
            Ok(v) => values.push(v),
            Err(e) => errors.push(e),
        }
    }
    if errors.is_empty() {
        Ok(values)
    } else {
        Err(ComputeErrors { errors })
    }
}

/// Runs two heavy closures in parallel on the compute pool, returning both
/// results.
///
/// This wraps [`rayon::join`] with panic isolation: if either branch panics,
/// its panic is converted into a [`MytheclipseError::ComputePanic`] and the
/// other branch still completes. The pool remains usable afterward.
///
/// ```
/// use mytheclipse::compute::compute_join;
///
/// let (a, b) = compute_join(
///     || (0..1_000_000u64).sum::<u64>(),
///     || (1_000_000..2_000_000u64).sum::<u64>(),
/// ).unwrap();
/// assert_eq!(a + b, (0..2_000_000u64).sum::<u64>());
/// ```
pub fn compute_join<A, RA, B, RB>(a: A, b: B) -> Result<(RA, RB), MytheclipseError>
where
    A: FnOnce() -> RA + Send,
    RA: Send,
    B: FnOnce() -> RB + Send,
    RB: Send,
{
    let a = AssertUnwindSafe(a);
    let b = AssertUnwindSafe(b);
    context().compute_pool.install(|| {
        let (ra, rb) = rayon::join(
            move || {
                catch_unwind(a)
                    .map_err(|p| MytheclipseError::ComputePanic(panic_payload_to_string(p)))
            },
            move || {
                catch_unwind(b)
                    .map_err(|p| MytheclipseError::ComputePanic(panic_payload_to_string(p)))
            },
        );
        Ok((ra?, rb?))
    })
}

/// Runs `f` over every item on the compute pool in parallel, discarding
/// return values (side effects only), collecting errors and panics.
///
/// ```
/// use std::sync::atomic::{AtomicUsize, Ordering};
/// use std::sync::Arc;
/// use mytheclipse::compute::compute_par_for_each;
///
/// let count = Arc::new(AtomicUsize::new(0));
/// let c = Arc::clone(&count);
/// compute_par_for_each(0..100, move |x| {
///     c.fetch_add(x as usize, Ordering::SeqCst);
///     Ok::<_, String>(())
/// }).unwrap();
/// assert_eq!(count.load(Ordering::SeqCst), 4950);
/// ```
pub fn compute_par_for_each<I, F>(items: I, f: F) -> Result<(), ComputeErrors>
where
    I: IntoParallelIterator + Send,
    I::Item: Send,
    F: Fn(I::Item) -> Result<(), String> + Send + Sync,
{
    let wrapped = AssertUnwindSafe(f);
    let collected: Vec<Result<(), String>> = context().compute_pool.install(move || {
        let f = wrapped;
        items
            .into_par_iter()
            .map(|item| {
                catch_unwind(AssertUnwindSafe(|| f(item)))
                    .unwrap_or_else(|payload| Err(panic_payload_to_string(payload)))
            })
            .collect()
    });

    if collected.iter().any(|r| r.is_err()) {
        Err(ComputeErrors {
            errors: collected.into_iter().filter_map(|r| r.err()).collect(),
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_panic_is_isolated_and_pool_survives() {
        let panicked: Result<u32, MytheclipseError> = compute(|| panic!("boom"));
        assert!(matches!(panicked, Err(MytheclipseError::ComputePanic(_))));

        let recovered = compute(|| 1 + 1);
        assert_eq!(recovered.unwrap(), 2);
    }

    #[test]
    fn compute_map_ordered_and_aggregates_errors() {
        let squares = compute_map(vec![1u32, 2, 3, 4], |x| Ok::<_, String>(x * x)).unwrap();
        assert_eq!(squares, vec![1, 4, 9, 16]);

        let err = compute_map(vec![1u32, 2, 3], |x| {
            if x == 2 {
                Err("boom".to_string())
            } else {
                Ok(x * 10)
            }
        })
        .unwrap_err();
        assert_eq!(err.len(), 1);
        assert_eq!(err.iter().next().unwrap(), "boom");
    }

    #[test]
    fn compute_map_panic_is_isolated_and_collected() {
        let err = compute_map(vec![1u32, 2, 3], |x| {
            if x == 2 {
                panic!("item panic")
            } else {
                Ok::<_, String>(x)
            }
        })
        .unwrap_err();
        assert_eq!(err.len(), 1);
        assert!(err.iter().next().unwrap().contains("item panic"));

        // pool still usable
        let recovered = compute_map(vec![1u32], |x| Ok::<_, String>(x + 1)).unwrap();
        assert_eq!(recovered, vec![2]);
    }

    #[test]
    fn compute_join_runs_both_branches() {
        let (a, b) = compute_join(
            || (0..100_000u64).sum::<u64>(),
            || (100_000..200_000u64).sum::<u64>(),
        )
        .unwrap();
        assert_eq!(a + b, (0..200_000u64).sum::<u64>());
    }

    #[test]
    fn compute_join_panic_is_isolated() {
        let a = compute_join(|| panic!("branch a"), || 42u32);
        assert!(matches!(a, Err(MytheclipseError::ComputePanic(_))));

        // pool still usable
        let ok = compute_join(|| 1u32, || 2u32).unwrap();
        assert_eq!(ok, (1, 2));
    }

    #[test]
    fn compute_par_for_each_runs_all_side_effects() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let count = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&count);
        compute_par_for_each(0..100, move |x| {
            c.fetch_add(x as usize, Ordering::SeqCst);
            Ok::<_, String>(())
        })
        .unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 4950);
    }
}
