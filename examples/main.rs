//! Demonstrates `mytheclipse`'s capabilities end to end:
//! execution primitives, resiliency & fault tolerance, traffic control,
//! lifecycle management, and observability.

use std::time::Duration;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    println!("=== 1. Execution Primitives & Context ===");
    let ctx = mytheclipse::init();
    println!(
        "engine context: io_threads={} compute_threads={} bg_concurrency={}",
        ctx.io_threads, ctx.compute_threads, ctx.bg_concurrency
    );

    let io_handle = mytheclipse::spawn_io(async {
        tokio::time::sleep(Duration::from_millis(10)).await;
        42u64
    });

    let sum_result = mytheclipse::compute(|| (1..=1_000u64).sum::<u64>());

    let panic_result: Result<u64, mytheclipse::MytheclipseError> = mytheclipse::compute(|| {
        panic!("intentional panic to demonstrate compute isolation");
    });

    let recovery_result = mytheclipse::compute(|| 2u64 + 2u64);

    let bg_handle = mytheclipse::spawn_bg(async {
        tokio::time::sleep(Duration::from_millis(10)).await;
        "bg-task-done"
    })
    .await;

    let io_value = io_handle.await.expect("io task panicked");
    let bg_value = bg_handle.await.expect("bg task panicked");

    println!("spawn_io result: {io_value}");
    println!("compute sum result: {sum_result:?}");
    println!("compute panic-isolation result: {panic_result:?}");
    println!("compute pool still usable after panic: {recovery_result:?}");
    println!("spawn_bg result: {bg_value}");

    println!("\n=== 2. Resiliency & Fault Tolerance ===");
    // Auto-Retry with Exponential Backoff & Full Jitter
    let attempts = std::sync::atomic::AtomicU32::new(0);
    let retry_res = mytheclipse::retry(
        mytheclipse::RetryConfig {
            max_attempts: 4,
            base_delay: Duration::from_millis(10),
            ..Default::default()
        },
        |_| true,
        || async {
            let count = attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            if count < 3 {
                Err("temporary network failure")
            } else {
                Ok("connected successfully!")
            }
        },
    )
    .await;
    println!(
        "retry result (succeeded on attempt {}): {retry_res:?}",
        attempts.load(std::sync::atomic::Ordering::SeqCst)
    );

    // Circuit Breaker
    let breaker = mytheclipse::CircuitBreaker::new(mytheclipse::CircuitBreakerConfig {
        failure_threshold: 2,
        open_timeout: Duration::from_millis(50),
        ..Default::default()
    });
    let _ = breaker.call(|| Err::<(), _>("service unavailable"));
    let _ = breaker.call(|| Err::<(), _>("service unavailable"));
    println!(
        "circuit breaker state after 2 failures: {:?}",
        breaker.state()
    );
    let call_when_open: Result<(), mytheclipse::CircuitError<&str>> =
        breaker.call(|| Ok::<(), &str>(()));
    println!("call blocked by circuit breaker: {call_when_open:?}");

    // Timeout & Deadlines
    let timeout_res = mytheclipse::with_timeout(Duration::from_millis(20), async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        "finished"
    })
    .await;
    println!("with_timeout result on slow task: {timeout_res:?}");

    println!("\n=== 3. Traffic & Resource Control ===");
    // Rate Limiter (Token Bucket)
    let limiter = mytheclipse::RateLimiter::new(100.0, 2);
    println!(
        "rate limiter initial tokens: {}",
        limiter.available_tokens()
    );
    let _ = limiter.try_acquire();
    println!(
        "rate limiter tokens after 1 acquire: {}",
        limiter.available_tokens()
    );

    // Concurrency Limiter (Sync Semaphore Wrapper)
    let conc = mytheclipse::ConcurrencyLimiter::new(2);
    let permit1 = conc.try_acquire().expect("permit 1");
    let permit2 = conc.try_acquire().expect("permit 2");
    println!(
        "concurrency limiter in use: {}/{}",
        conc.in_use(),
        conc.max()
    );
    assert!(conc.try_acquire().is_none());
    drop(permit1);
    drop(permit2);
    println!(
        "concurrency limiter in use after drop: {}/{}",
        conc.in_use(),
        conc.max()
    );

    // Backpressure Queue (Graceful Degradation)
    let queue = mytheclipse::BackpressureQueue::new(2, mytheclipse::OverflowPolicy::DropOldest);
    queue.push("item-1").await.unwrap();
    queue.push("item-2").await.unwrap();
    queue.push("item-3").await.unwrap(); // item-1 will be dropped
    println!(
        "backpressure queue: len={}, dropped={}, next={:?}",
        queue.len(),
        queue.dropped_count(),
        queue.pop().await
    );

    println!("\n=== 4. Lifecycle & State Management ===");
    // Cron Scheduler (Minimal 5-field parser without external crate)
    let cron = mytheclipse::CronSchedule::parse("0 1 * * *").expect("valid cron expression");
    let now = mytheclipse::cron::CronTime::now();
    let next_runs = cron.next_five(now);
    println!("cron '0 1 * * *' next 3 scheduled fire times:");
    for t in next_runs.iter().take(3) {
        println!(
            "  -> {:04}-{:02}-{:02} {:02}:{:02}:00 UTC",
            t.year, t.month, t.day, t.hour, t.minute
        );
    }

    // Graceful Shutdown Manager
    let shutdown = mytheclipse::ShutdownManager::new();
    let sig = shutdown.handle();
    println!(
        "shutdown signal status before request: is_shutdown={}",
        sig.is_shutdown()
    );
    shutdown.request();
    println!(
        "shutdown signal status after request: is_shutdown={}",
        sig.is_shutdown()
    );

    println!("\n=== 5. Telemetry & Observability ===");
    // Centralized Metrics Collector (Prometheus text exposition format)
    let metrics = mytheclipse::MetricsCollector::new();
    metrics.record_task(Duration::from_millis(15));
    metrics.record_task(Duration::from_millis(25));
    metrics.set_active_threads(ctx.compute_threads);
    metrics.set_queue_capacity(100);
    metrics.set_queue_remaining(98);
    metrics.inc_counter("http_requests_total", 42);
    metrics.set_gauge("memory_usage_mb", 128.5);

    println!("Prometheus Export Output:\n---");
    print!("{}", metrics.export_prometheus());
    println!("---");

    // Panic & Span Isolation Tracker
    let isolated_panic = mytheclipse::PanicTracker::catch(|| {
        panic!("worker routine isolated error");
    });
    println!("PanicTracker::catch result: {isolated_panic:?}");
    println!("\nAll mytheclipse features executed successfully!");
}
