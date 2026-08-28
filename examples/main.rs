//! Demonstrates `mytheclipse`'s three execution primitives end to end, including
//! automatic recovery from a panicking compute closure.

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let ctx = mytheclipse::init();
    println!(
        "engine context: io_threads={} compute_threads={} bg_concurrency={}",
        ctx.io_threads, ctx.compute_threads, ctx.bg_concurrency
    );

    let io_handle = mytheclipse::spawn_io(async {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        42u64
    });

    let sum_result = mytheclipse::compute(|| (1..=1_000u64).sum::<u64>());

    let panic_result: Result<u64, mytheclipse::CorexError> = mytheclipse::compute(|| {
        panic!("intentional panic to demonstrate isolation");
    });

    let recovery_result = mytheclipse::compute(|| 2u64 + 2u64);

    let bg_handle = mytheclipse::spawn_bg(async {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
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
}
