//! Demonstrates `corex`'s three execution primitives end to end, including
//! automatic recovery from a panicking compute closure.

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let ctx = corex::init();
    println!(
        "engine context: io_threads={} compute_threads={} bg_concurrency={}",
        ctx.io_threads, ctx.compute_threads, ctx.bg_concurrency
    );

    let io_handle = corex::spawn_io(async {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        42u64
    });

    let sum_result = corex::compute(|| (1..=1_000u64).sum::<u64>());

    let panic_result: Result<u64, corex::CorexError> = corex::compute(|| {
        panic!("intentional panic to demonstrate isolation");
    });

    let recovery_result = corex::compute(|| 2u64 + 2u64);

    let bg_handle = corex::spawn_bg(async {
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
