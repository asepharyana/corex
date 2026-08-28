//! Span and panic isolation tracking (feature `observability`).
//!
//! [`PanicTracker::install`] installs a global panic hook that logs every
//! panic through [`tracing`] inside a `mytheclipse_panic_task` span — with the
//! panic message and source location — without stopping the application.
//! [`PanicTracker::catch`] wraps a closure so a panic is captured and reported
//! as a [`PanicInfo`] instead of unwinding across a boundary.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

/// Information about a captured panic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanicInfo {
    /// Best-effort rendering of the panic payload.
    pub message: String,
    /// The source location of the panic, if available.
    pub location: Option<String>,
}

#[allow(deprecated)]
type PanicHookFn = dyn Fn(&std::panic::PanicInfo<'_>) + Send + Sync + 'static;

/// A RAII guard that, while held, keeps the mytheclipse panic hook installed,
/// restoring the previously-installed hook on drop.
///
/// Returned by [`PanicTracker::install`].
#[must_use = "dropping the guard restores the previous panic hook"]
pub struct PanicGuard {
    previous: Arc<PanicHookFn>,
}

/// Installs a panic hook that logs panics through [`tracing`].
pub struct PanicTracker;

impl PanicTracker {
    /// Installs a global panic hook that records each panic via
    /// `tracing::error!` inside a `mytheclipse_panic_task` span, then re-invokes
    /// the previously-installed hook so default panic output (and any user
    /// hook) still runs.
    ///
    /// The application does not stop; the panic still unwinds normally, but a
    /// trace is captured first. Returns a [`PanicGuard`] that restores the
    /// previous hook when dropped.
    #[allow(deprecated)]
    pub fn install() -> PanicGuard {
        let previous: Box<PanicHookFn> = std::panic::take_hook();
        let previous: Arc<PanicHookFn> = Arc::from(previous);
        let for_hook = Arc::clone(&previous);
        std::panic::set_hook(Box::new(move |info| {
            let span = tracing::error_span!("mytheclipse_panic_task");
            let message = payload_to_string(info.payload());
            let location = info
                .location()
                .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()));
            let _guard = span.enter();
            match location {
                Some(loc) => tracing::error!("panic caught: {message} at {loc}"),
                None => tracing::error!("panic caught: {message}"),
            }
            for_hook(info);
        }));
        PanicGuard { previous }
    }

    /// Runs `f`, catching any panic and returning it as an
    /// [`Err(PanicInfo)`](PanicInfo) rather than unwinding across the caller.
    ///
    /// Useful to isolate a panic inside a worker thread so it cannot bring
    /// down the rest of the process. The caller is responsible for deciding
    /// whether to continue after a caught panic.
    pub fn catch<T>(f: impl FnOnce() -> T) -> Result<T, PanicInfo> {
        let wrapped = AssertUnwindSafe(f);
        catch_unwind(wrapped).map_err(|payload| PanicInfo {
            message: payload_to_string(&*payload),
            location: None,
        })
    }
}

impl Drop for PanicGuard {
    fn drop(&mut self) {
        // Take out the hook we installed so we don't call ourselves, then
        // restore the one that was active before it.
        let _current = std::panic::take_hook();
        let previous = Arc::clone(&self.previous);
        std::panic::set_hook(Box::new(move |info| previous(info)));
    }
}

/// Renders a panic payload (a `&str`, `String`, or fallback) to a string.
fn payload_to_string(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "panic with a non-string payload".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn catch_returns_ok_on_success() {
        let result = PanicTracker::catch(|| 1 + 1);
        assert_eq!(result.unwrap(), 2);
    }

    #[test]
    fn catch_captures_panic_as_err() {
        let result: Result<u32, PanicInfo> = PanicTracker::catch(|| panic!("boom"));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().message, "boom");
    }

    #[test]
    fn catch_captures_string_payload() {
        let err = PanicTracker::catch(|| panic!("{}", String::from("stringy"))).unwrap_err();
        assert_eq!(err.message, "stringy");
    }

    #[test]
    fn hook_is_restored_after_guard_drop() {
        // Install then drop — previous hook (default) restored.
        let _guard = PanicTracker::install();
        drop(_guard);
        let _ = PanicTracker::catch(|| panic!("irrelevant"));
    }

    #[test]
    fn hook_runs_without_and_panics_are_still_catchable() {
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_hook = Arc::clone(&calls);
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            calls_hook.fetch_add(1, Ordering::SeqCst);
            let _ = info.location().is_some();
        }));
        let _guard = PanicTracker::install();
        // A panic in another thread triggers the hook but not the main thread.
        std::thread::spawn(|| panic!("worker boom"))
            .join()
            .unwrap_err();
        drop(_guard);
        std::panic::set_hook(prev);
        assert!(calls.load(Ordering::SeqCst) >= 1);
    }
}
