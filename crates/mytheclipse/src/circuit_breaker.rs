//! Circuit breaker for isolating failures in calls to external services
//! (feature `resiliency`).
//!
//! A [`CircuitBreaker`] tracks consecutive failures on calls it guards. Once
//! failures reach a threshold it *trips open*, refusing further calls for a
//! cooldown window so the target gets time to recover. After the window it
//! transitions to *half-open*, admitting a small number of probe calls; a
//! successful probe closes the circuit, a failed probe re-opens it.

use std::sync::atomic::{AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The current state of a circuit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Requests flow normally; failures accumulate toward the threshold.
    Closed,
    /// Requests are refused while the circuit waits to recover.
    Open,
    /// A limited number of probe requests are admitted to test recovery.
    HalfOpen,
}

const CLOSED: u8 = 0;
const OPEN: u8 = 1;
const HALF_OPEN: u8 = 2;

/// Configuration for a [`CircuitBreaker`].
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Consecutive failures that trip `Closed` -> `Open`.
    pub failure_threshold: u64,
    /// How long to remain `Open` before moving to `HalfOpen`.
    pub open_timeout: Duration,
    /// Maximum concurrent probe calls admitted while `HalfOpen`.
    pub half_open_max_calls: usize,
    /// Consecutive successes that close `HalfOpen` -> `Closed`.
    pub close_successes: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            open_timeout: Duration::from_secs(30),
            half_open_max_calls: 1,
            close_successes: 1,
        }
    }
}

/// The result of a guarded call.
#[derive(Debug)]
pub enum CircuitError<E> {
    /// The circuit is open and the call was refused without executing.
    Open,
    /// The guarded operation failed (and the failure was recorded).
    Inner(E),
}

impl<E: std::fmt::Display> std::fmt::Display for CircuitError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open => write!(f, "circuit breaker is open"),
            Self::Inner(err) => write!(f, "guarded call failed: {err}"),
        }
    }
}

impl<E: std::fmt::Debug + std::fmt::Display> std::error::Error for CircuitError<E> {}

struct Inner {
    state: AtomicU8,
    failures: AtomicU64,
    successes: AtomicU64,
    half_open_in_flight: AtomicUsize,
    opened_at: Mutex<Option<Instant>>,
    config: CircuitBreakerConfig,
}

/// A thread-safe circuit breaker.
///
/// Construct with [`CircuitBreaker::new`]; use [`CircuitBreaker::call`] to
/// guard a synchronous operation, or [`CircuitBreaker::allow_request`] +
/// [`CircuitBreaker::record_success`] / [`CircuitBreaker::record_failure`] to
/// guard an async call that cannot hold a borrow across an `.await`.
#[derive(Clone)]
pub struct CircuitBreaker {
    inner: Arc<Inner>,
}

impl CircuitBreaker {
    /// Builds a new breaker with the given configuration.
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            inner: Arc::new(Inner {
                state: AtomicU8::new(CLOSED),
                failures: AtomicU64::new(0),
                successes: AtomicU64::new(0),
                half_open_in_flight: AtomicUsize::new(0),
                opened_at: Mutex::new(None),
                config,
            }),
        }
    }

    /// Returns the current circuit state, applying the open->half-open
    /// transition if the cooldown has elapsed.
    pub fn state(&self) -> CircuitState {
        match self.inner.state.load(Ordering::Acquire) {
            OPEN => {
                let opened = self.inner.opened_at.lock().unwrap();
                if opened
                    .map(|t| t.elapsed() >= self.inner.config.open_timeout)
                    .unwrap_or(false)
                {
                    self.transition(HALF_OPEN);
                    CircuitState::HalfOpen
                } else {
                    CircuitState::Open
                }
            }
            HALF_OPEN => CircuitState::HalfOpen,
            _ => CircuitState::Closed,
        }
    }

    /// Whether requests should currently be refused.
    pub fn is_open(&self) -> bool {
        self.state() == CircuitState::Open
    }

    /// Whether a request may proceed right now without tripping the breaker.
    ///
    /// Useful as a gate for an async call: call this before `.await`ing, then
    /// report the outcome via [`CircuitBreaker::record_success`] /
    /// [`CircuitBreaker::record_failure`].
    pub fn allow_request(&self) -> bool {
        match self.state() {
            CircuitState::Open => false,
            CircuitState::HalfOpen => {
                let in_flight = self.inner.half_open_in_flight.load(Ordering::Acquire);
                if in_flight < self.inner.config.half_open_max_calls {
                    self.inner
                        .half_open_in_flight
                        .fetch_add(1, Ordering::AcqRel);
                    true
                } else {
                    false
                }
            }
            CircuitState::Closed => true,
        }
    }

    /// Runs `f`, recording its outcome and refusing the call if the circuit is
    /// open.
    pub fn call<T, E, F>(&self, f: F) -> Result<T, CircuitError<E>>
    where
        F: FnOnce() -> Result<T, E>,
    {
        if !self.allow_request() {
            return Err(CircuitError::Open);
        }
        let result = f();
        match result {
            Ok(value) => {
                self.record_result(true);
                Ok(value)
            }
            Err(err) => {
                self.record_result(false);
                Err(CircuitError::Inner(err))
            }
        }
    }

    /// Records that a (previously-admitted) call succeeded.
    ///
    /// Use with [`CircuitBreaker::allow_request`] when guarding an async call.
    pub fn record_success(&self) {
        self.record_result(true)
    }

    /// Records that a (previously-admitted) call failed.
    ///
    /// Use with [`CircuitBreaker::allow_request`] when guarding an async call.
    pub fn record_failure(&self) {
        self.record_result(false)
    }

    /// Resets the breaker to its initial `Closed` state.
    pub fn reset(&self) {
        self.inner.state.store(CLOSED, Ordering::Release);
        self.inner.failures.store(0, Ordering::Release);
        self.inner.successes.store(0, Ordering::Release);
        self.inner.half_open_in_flight.store(0, Ordering::Release);
        *self.inner.opened_at.lock().unwrap() = None;
    }

    fn record_result(&self, success: bool) {
        match self.inner.state.load(Ordering::Acquire) {
            HALF_OPEN => {
                // A probe finished; release its slot regardless of outcome.
                self.inner
                    .half_open_in_flight
                    .fetch_sub(1, Ordering::AcqRel);
                if success {
                    let successes = self.inner.successes.fetch_add(1, Ordering::AcqRel) + 1;
                    if successes >= self.inner.config.close_successes {
                        self.close();
                    }
                } else {
                    self.inner.failures.fetch_add(1, Ordering::AcqRel);
                    self.open();
                }
            }
            OPEN => {
                if success {
                    self.close();
                }
            }
            _ => {
                if success {
                    self.inner.failures.store(0, Ordering::Release);
                } else {
                    let failures = self.inner.failures.fetch_add(1, Ordering::AcqRel) + 1;
                    if failures >= self.inner.config.failure_threshold {
                        self.open();
                    }
                }
            }
        }
    }

    fn transition(&self, next: u8) {
        self.inner.state.store(next, Ordering::Release);
    }

    fn close(&self) {
        self.inner.state.store(CLOSED, Ordering::Release);
        self.inner.failures.store(0, Ordering::Release);
        self.inner.successes.store(0, Ordering::Release);
        self.inner.half_open_in_flight.store(0, Ordering::Release);
        self.inner.opened_at.lock().unwrap().take();
    }

    fn open(&self) {
        self.inner.state.store(OPEN, Ordering::Release);
        self.inner.failures.store(0, Ordering::Release);
        self.inner.successes.store(0, Ordering::Release);
        self.inner.half_open_in_flight.store(0, Ordering::Release);
        *self.inner.opened_at.lock().unwrap() = Some(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn breaker() -> CircuitBreaker {
        CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 3,
            open_timeout: Duration::from_millis(50),
            half_open_max_calls: 1,
            close_successes: 1,
        })
    }

    #[test]
    fn closed_start_state() {
        let b = breaker();
        assert_eq!(b.state(), CircuitState::Closed);
        assert!(!b.is_open());
    }

    #[test]
    fn trips_open_after_threshold() {
        let b = breaker();
        for _ in 0..3 {
            let r: Result<(), CircuitError<u8>> = b.call(|| Err(1u8));
            assert!(r.is_err());
        }
        assert_eq!(b.state(), CircuitState::Open);
        assert!(b.is_open());
    }

    #[test]
    fn open_refuses_calls() {
        let b = breaker();
        for _ in 0..3 {
            let _: Result<(), CircuitError<u8>> = b.call(|| Err(1u8));
        }
        let refused: Result<(), CircuitError<u8>> = b.call(|| Ok(()));
        assert!(matches!(refused, Err(CircuitError::Open)));
    }

    #[test]
    fn success_resets_failure_count_in_closed() {
        let b = breaker();
        let _: Result<(), CircuitError<u8>> = b.call(|| Err(1u8));
        let _: Result<(), CircuitError<u8>> = b.call(|| Err(1u8));
        assert_eq!(b.state(), CircuitState::Closed);
        let r: Result<(), CircuitError<u8>> = b.call(|| Ok(()));
        assert!(r.is_ok());
        // Failure count reset — two more failures should not trip (needs 3).
        let _: Result<(), CircuitError<u8>> = b.call(|| Err(1u8));
        let _: Result<(), CircuitError<u8>> = b.call(|| Err(1u8));
        assert_eq!(b.state(), CircuitState::Closed);
    }

    #[test]
    fn half_open_closes_after_success() {
        let b = breaker();
        for _ in 0..3 {
            let _: Result<(), CircuitError<u8>> = b.call(|| Err(1u8));
        }
        assert_eq!(b.state(), CircuitState::Open);

        std::thread::sleep(Duration::from_millis(60));
        // Once half-open, a success closes the circuit.
        let admitted = {
            // allow_request true implies half-open probe admitted
            b.allow_request()
        };
        if admitted {
            b.record_success();
        }
        assert_eq!(b.state(), CircuitState::Closed);
    }

    #[test]
    fn half_open_failure_reopens() {
        let b = breaker();
        for _ in 0..3 {
            let _: Result<(), CircuitError<u8>> = b.call(|| Err(1u8));
        }
        std::thread::sleep(Duration::from_millis(60));
        if b.allow_request() {
            b.record_failure();
        }
        assert_eq!(b.state(), CircuitState::Open);
    }

    #[test]
    fn reset_returns_to_closed() {
        let b = breaker();
        for _ in 0..3 {
            let _: Result<(), CircuitError<u8>> = b.call(|| Err(1u8));
        }
        assert_eq!(b.state(), CircuitState::Open);
        b.reset();
        assert_eq!(b.state(), CircuitState::Closed);
    }

    #[test]
    fn tracks_inner_error_and_success() {
        let b = breaker();
        let ok: Result<u32, CircuitError<u8>> = b.call(|| Ok(5u32));
        assert_eq!(ok.unwrap(), 5);
        let err: Result<u32, CircuitError<u8>> = b.call(|| Err(9u8));
        assert!(matches!(err, Err(CircuitError::Inner(9))));
    }
}
