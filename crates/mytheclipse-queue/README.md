# mytheclipse-queue

A unified job queue abstraction so your background work isn't locked to one
transport. Provides a single `Queue` trait, `Job` type, and `WorkerPool` executor
with configurable retry/backoff, concurrency, and a dead-letter queue — behind
pluggable backends:

- **In-memory** (default) — `tokio::sync::mpsc` + task spawning, no external service.
- **Redis** (`redis`) — LIST-based queue with atomic moves.
- **NATS JetStream** (`nats`) — durable consumer with ACK/NACK.
- **PostgreSQL** (`postgres`) — `SKIP LOCKED` polling.

All backends share the same `WorkerPool` driver; swapping is a one-line change
at construction time.

## Features

| Feature | Default | Backend | Description |
| :--- | :---: | :--- | :--- |
| `in-memory` | yes | `tokio::sync` | In-process queue, no external deps. |
| `redis` | no | `redis` crate (fred) | Redis/Valkey list-based queue. |
| `nats` | no | `async-nats` | NATS JetStream durable consumer. |
| `postgres` | no | `tokio-postgres` | PostgreSQL `SKIP LOCKED` queue. |

## Usage

```rust
use mytheclipse_queue::{InMemoryQueue, WorkerPool, Job, JobHandler, JobFuture};

fn print_handler() -> impl JobHandler {
    struct PrintHandler;
    impl JobHandler for PrintHandler {
        fn handle(&self, job: Job) -> JobFuture {
            Box::pin(async move {
                println!("payload: {:?}", job.payload);
                Ok(())
            })
        }
    }
    PrintHandler
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let queue = InMemoryQueue::new();
    queue.enqueue("email", b"hello".to_vec()).await?;

    let pool = WorkerPool::new(queue, 4);
    pool.start("email", print_handler());

    Ok(())
}
```

Swap `InMemoryQueue::new()` for `RedisQueue::connect("redis://127.0.0.1")` (with
the `redis` feature) or `NatsQueue::connect("nats://127.0.0.1")` (with the `nats`
feature) to move to a distributed broker without touching handler code.
