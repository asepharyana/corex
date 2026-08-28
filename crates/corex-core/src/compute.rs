//! Panic-isolated heavy compute on a sized [`rayon::ThreadPool`].

use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::context::context;
use crate::error::CorexError;

/// Runs `f` on the global compute thread pool and returns its result.
///
/// If `f` panics, the panic is caught and converted into
/// [`CorexError::ComputePanic`] instead of unwinding across the pool
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
pub fn compute<F, R>(f: F) -> Result<R, CorexError>
where
    F: FnOnce() -> R + Send,
    R: Send,
{
    let wrapped = AssertUnwindSafe(f);
    context()
        .compute_pool
        .install(move || catch_unwind(wrapped))
        .map_err(|payload| CorexError::ComputePanic(panic_payload_to_string(payload)))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_panic_is_isolated_and_pool_survives() {
        let panicked: Result<u32, CorexError> = compute(|| panic!("boom"));
        assert!(matches!(panicked, Err(CorexError::ComputePanic(_))));

        let recovered = compute(|| 1 + 1);
        assert_eq!(recovered.unwrap(), 2);
    }
}
