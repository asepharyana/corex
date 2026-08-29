//! Automatic retry with exponential backoff and jitter (feature `resiliency`).
//!
//! [`retry`] re-runs a fallible async operation according to a
//! [`RetryConfig`], sleeping an exponentially-growing, optionally-jittered
//! delay between attempts. A predicate decides which errors are retryable, so
//! permanent failures (e.g. a 4xx response) short-circuit immediately while
//! transient ones (network hiccups, connection refused) are retried.

use std::future::Future;
use std::time::Duration;

use rand::Rng;
use tracing::Instrument;

/// How much random jitter to apply to each backoff delay.
///
/// Jitter prevents the "thundering herd" of many retrying clients waking
/// simultaneously; [`Full`](JitterKind::Full) is the most aggressive and is
/// the recommended default for distributed systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JitterKind {
    /// No randomness: sleep exactly the computed backoff.
    None,
    /// Sleep in `[delay, 2 * delay)` — sometimes called "equal jitter".
    Equal,
    /// Sleep in `[0, delay)` — "full jitter".
    Full,
}

/// Configuration driving [`retry`].
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Total number of attempts (including the first). Must be `>= 1`.
    pub max_attempts: u32,
    /// Initial delay before the first retry.
    pub base_delay: Duration,
    /// Upper bound on the computed backoff delay.
    pub max_delay: Duration,
    /// Exponential growth factor applied after each failure.
    pub factor: f64,
    /// Jitter strategy applied to each delay.
    pub jitter: JitterKind,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(10),
            factor: 2.0,
            jitter: JitterKind::Full,
        }
    }
}

/// The error returned by [`retry`] once it gives up.
#[derive(Debug)]
pub enum RetryError<E> {
    /// All attempts were made and the last one failed.
    ///
    /// `attempts` is the total number of attempts performed and `last` is the
    /// error produced by the final attempt.
    Exhausted { attempts: u32, last: E },
}

impl<E> RetryError<E> {
    /// Returns a reference to the error produced by the final attempt.
    pub fn last(&self) -> &E {
        match self {
            Self::Exhausted { last, .. } => last,
        }
    }
}

impl<E: std::fmt::Display> std::fmt::Display for RetryError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exhausted { attempts, last } => {
                write!(f, "retry exhausted after {attempts} attempts: {last}")
            }
        }
    }
}

impl<E: std::fmt::Debug + std::fmt::Display> std::error::Error for RetryError<E> {}

/// Statistics collected during a [`retry`] call.
#[derive(Debug, Clone, Default)]
pub struct RetryStats {
    /// Total number of attempts made (including the first).
    pub attempts: u32,
    /// Number of retries performed (= `attempts - 1` if exhausted, or
    /// `attempts - 1` if ultimately succeeded after at least one retry).
    pub retries: u32,
    /// The error message from the final attempt, if any.
    pub last_error: Option<String>,
}

/// Like [`retry`] but also returns [`RetryStats`] capturing attempt counts.
///
/// Retries `op` according to `config`, retrying only errors for which
/// `filter` returns `true`.
///
/// Like [`retry`] but also returns [`RetryStats`].
pub async fn retry_with_stats<T, E, F, Fut, P>(
    config: RetryConfig,
    filter: P,
    mut op: F,
) -> (Result<T, RetryError<E>>, RetryStats)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    P: Fn(&E) -> bool,
    E: std::fmt::Display,
{
    let mut attempt: u32 = 0;
    let mut last_error: Option<String> = None;
    loop {
        attempt += 1;
        let span = tracing::info_span!(
            "mytheclipse_retry_task",
            attempt,
            max_attempts = config.max_attempts
        );
        let result = op().instrument(span).await;

        match result {
            Ok(value) => {
                let stats = RetryStats {
                    attempts: attempt,
                    retries: attempt.saturating_sub(1),
                    last_error,
                };
                return (Ok(value), stats);
            }
            Err(err) => {
                last_error = Some(err.to_string());
                let retryable = filter(&err);
                if !retryable || attempt >= config.max_attempts {
                    let stats = RetryStats {
                        attempts: attempt,
                        retries: attempt.saturating_sub(1),
                        last_error,
                    };
                    return (
                        Err(RetryError::Exhausted {
                            attempts: attempt,
                            last: err,
                        }),
                        stats,
                    );
                }
                let delay = backoff_delay(&config, attempt, rand::thread_rng());
                tokio::time::sleep(delay).await;
            }
        }
    }
}

/// Retries `op` according to `config`, retrying only errors for which
/// `filter` returns `true`.
///
/// On the first attempt and after each retryable failure, `op` is invoked
/// again (rebuilt from the captured closure) until it succeeds, the retryable
/// error becomes permanent, or `max_attempts` is reached. Between attempts the
/// coroutine sleeps for a backoff delay computed as
/// `min(max_delay, base_delay * factor^attempt)` with jitter applied per
/// [`RetryConfig::jitter`].
///
/// Each attempt runs inside a `mytheclipse_retry_task` tracing span carrying
/// the attempt index and total.
pub async fn retry<T, E, F, Fut, P>(
    config: RetryConfig,
    filter: P,
    mut op: F,
) -> Result<T, RetryError<E>>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    P: Fn(&E) -> bool,
{
    let mut attempt: u32 = 0;
    loop {
        attempt += 1;
        let span = tracing::info_span!(
            "mytheclipse_retry_task",
            attempt,
            max_attempts = config.max_attempts
        );
        let result = op().instrument(span).await;

        match result {
            Ok(value) => return Ok(value),
            Err(err) => {
                let retryable = filter(&err);
                if !retryable || attempt >= config.max_attempts {
                    return Err(RetryError::Exhausted {
                        attempts: attempt,
                        last: err,
                    });
                }
                let delay = backoff_delay(&config, attempt, rand::thread_rng());
                tokio::time::sleep(delay).await;
            }
        }
    }
}

/// Computes the (jittered) delay to sleep before retry `attempt` (1-based).
///
/// Kept as a pure function for testability.
pub(crate) fn backoff_delay<R: Rng>(config: &RetryConfig, attempt: u32, mut rng: R) -> Duration {
    let exponent = attempt.saturating_sub(1) as f64; // first retry uses base
    let computed = config.base_delay.as_millis() as f64 * config.factor.powf(exponent);
    let max_ms = config.max_delay.as_millis() as f64;
    let capped = computed.min(max_ms);

    let millis = match config.jitter {
        JitterKind::None => capped,
        JitterKind::Equal => capped / 2.0 + rng.gen_range(0.0..capped / 2.0),
        JitterKind::Full => rng.gen_range(0.0..capped),
    };

    Duration::from_millis(millis.clamp(0.0, max_ms) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn always<E>(_: &E) -> bool {
        true
    }

    #[tokio::test]
    async fn succeeds_on_first_attempt() {
        let calls = Cell::new(0u32);
        let result = retry(RetryConfig::default(), always, || async {
            calls.set(calls.get() + 1);
            Ok::<_, ()>(42u32)
        })
        .await;
        assert_eq!(result.unwrap(), 42);
        assert_eq!(calls.get(), 1);
    }

    #[tokio::test]
    async fn succeeds_after_transient_failures() {
        let config = RetryConfig {
            max_attempts: 5,
            base_delay: Duration::from_millis(1),
            ..RetryConfig::default()
        };
        let calls = Cell::new(0u32);
        let result = retry(config, always, || async {
            calls.set(calls.get() + 1);
            if calls.get() < 3 {
                Err::<u32, u8>(9)
            } else {
                Ok(7u32)
            }
        })
        .await;
        assert_eq!(result.unwrap(), 7);
        assert_eq!(calls.get(), 3);
    }

    #[tokio::test]
    async fn exhausts_after_max_attempts() {
        let config = RetryConfig {
            max_attempts: 3,
            base_delay: Duration::from_millis(1),
            ..RetryConfig::default()
        };
        let calls = Cell::new(0u32);
        let result = retry(config, always, || async {
            calls.set(calls.get() + 1);
            Err::<u32, u8>(42)
        })
        .await;
        assert!(matches!(
            result,
            Err(RetryError::Exhausted { attempts: 3, .. })
        ));
        assert_eq!(result.unwrap_err().last(), &42);
    }

    #[tokio::test]
    async fn non_retryable_error_short_circuits() {
        let config = RetryConfig {
            max_attempts: 10,
            base_delay: Duration::from_millis(1),
            ..RetryConfig::default()
        };
        let calls = Cell::new(0u32);
        let result = retry(
            config,
            |e: &u16| *e != 403,
            || async {
                calls.set(calls.get() + 1);
                Err::<u32, u16>(403)
            },
        )
        .await;
        assert!(matches!(
            result,
            Err(RetryError::Exhausted { attempts: 1, .. })
        ));
        assert_eq!(calls.get(), 1);
    }

    #[tokio::test]
    async fn retry_with_stats_succeeds_with_counts() {
        use std::cell::Cell;
        let config = RetryConfig {
            max_attempts: 5,
            base_delay: Duration::from_millis(1),
            ..RetryConfig::default()
        };
        let calls = Cell::new(0u32);
        let (result, stats) = retry_with_stats(config, |_| true, || async {
            calls.set(calls.get() + 1);
            if calls.get() < 3 { Err::<u32, &str>("fail") } else { Ok(42u32) }
        }).await;
        assert_eq!(result.unwrap(), 42);
        assert_eq!(stats.attempts, 3);
        assert_eq!(stats.retries, 2);
    }

    #[test]
    fn full_jitter_is_within_bounds_and_capped() {
        let config = RetryConfig {
            max_attempts: 3,
            base_delay: Duration::from_secs(2),
            max_delay: Duration::from_secs(4),
            factor: 10.0,
            jitter: JitterKind::Full,
        };
        let mut rng = rand::thread_rng();
        for _ in 0..1000 {
            let d = backoff_delay(&config, 2, &mut rng);
            assert!(d <= config.max_delay);
        }
    }
}
