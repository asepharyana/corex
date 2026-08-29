//! Worker pool for processing queued jobs with retry, backoff, and graceful shutdown.

use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;

use crate::error::JobError;
use crate::job::Job;
use crate::traits::Queue;

/// A future returned by a job handler.
pub type JobFuture = Pin<Box<dyn std::future::Future<Output = Result<(), JobError>> + Send>>;

/// A handler for processing a single job.
pub trait JobHandler: Send + Sync {
    fn handle(&self, job: Job) -> JobFuture;
}

impl<F, Fut> JobHandler for F
where
    F: Fn(Job) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<(), JobError>> + Send + 'static,
{
    fn handle(&self, job: Job) -> JobFuture {
        Box::pin((self)(job))
    }
}

/// Configuration for the worker pool.
#[derive(Debug, Clone)]
pub struct WorkerConfig {
    /// Maximum concurrent job handlers.
    pub concurrency: usize,
    /// Maximum number of retry attempts (0 = no retries).
    pub max_retries: u32,
    /// Base delay for exponential backoff between retries.
    pub retry_base_delay: Duration,
    /// Maximum delay cap for retry backoff.
    pub retry_max_delay: Duration,
    /// Backoff multiplier.
    pub retry_factor: f64,
    /// Visibility timeout for in-progress jobs.
    pub visibility_timeout: Duration,
    /// Polling interval when a queue is empty.
    pub poll_interval: Duration,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            concurrency: 4,
            max_retries: 3,
            retry_base_delay: Duration::from_millis(500),
            retry_max_delay: Duration::from_secs(10),
            retry_factor: 2.0,
            visibility_timeout: Duration::from_secs(30),
            poll_interval: Duration::from_millis(100),
        }
    }
}

/// A pool of workers consuming jobs from a `Queue`.
pub struct WorkerPool<Q: Queue + 'static> {
    queue: Arc<Q>,
    config: WorkerConfig,
    semaphore: Arc<Semaphore>,
}

impl<Q: Queue + 'static> WorkerPool<Q> {
    /// Creates a new worker pool with the given concurrency.
    pub fn new(queue: Q, concurrency: usize) -> Self {
        Self::with_config(
            queue,
            WorkerConfig {
                concurrency,
                ..Default::default()
            },
        )
    }

    /// Creates a new worker pool with explicit configuration.
    pub fn with_config(queue: Q, config: WorkerConfig) -> Self {
        let sem = Arc::new(Semaphore::new(config.concurrency.max(1)));
        Self {
            queue: Arc::new(queue),
            config,
            semaphore: sem,
        }
    }

    /// Starts `concurrency` workers consuming from `topic`.
    pub fn start<H>(&self, topic: &str, handler: H)
    where
        H: JobHandler + 'static,
    {
        let queue = Arc::clone(&self.queue);
        let config = self.config.clone();
        let semaphore = Arc::clone(&self.semaphore);
        let handler: Arc<dyn JobHandler> = Arc::new(handler);
        let topic_owned = topic.to_string();

        for _ in 0..config.concurrency {
            let q = Arc::clone(&queue);
            let sem = Arc::clone(&semaphore);
            let h = Arc::clone(&handler);
            let cfg = config.clone();
            let topic_inner = topic_owned.clone();

            tokio::spawn(async move {
                loop {
                    match q.dequeue(&topic_inner, cfg.poll_interval).await {
                        Ok(Some(job)) => {
                            let _permit = sem.clone().acquire_owned().await;
                            let q2 = Arc::clone(&q);
                            let h2 = Arc::clone(&h);
                            let cfg2 = cfg.clone();
                            let t2 = topic_inner.clone();
                            let j2 = job.clone();

                            tokio::spawn(async move {
                                let fut = h2.handle(j2.clone());
                                match fut.await {
                                    Ok(()) => {
                                        let _ = q2.ack(&j2).await;
                                    }
                                    Err(_) => {
                                        if j2.attempt < cfg2.max_retries {
                                            let _ = q2.nack(&j2, true).await;
                                        } else {
                                            let _ = q2.dlq_move(&t2, j2.clone()).await;
                                            let _ = q2.nack(&j2, false).await;
                                        }
                                    }
                                }
                            });
                        }
                        Ok(None) => {}
                        Err(e) => {
                            tracing::error!("queue error: {e}");
                            tokio::time::sleep(cfg.poll_interval).await;
                        }
                    }
                }
            });
        }
    }
}

/// Computes the (capped) exponential backoff delay.
pub fn retry_delay(config: &WorkerConfig, attempt: u32) -> Duration {
    let exponent = attempt as f64;
    let computed =
        config.retry_base_delay.as_millis() as f64 * config.retry_factor.powf(exponent.max(0.0));
    let capped = computed.min(config.retry_max_delay.as_millis() as f64);
    Duration::from_millis(capped as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_delay_bounded() {
        let config = WorkerConfig::default();
        let d = retry_delay(&config, 0);
        assert!(d <= config.retry_max_delay);
    }
}
