# mytheclipse-http

HTTP client/server abstraction with built-in mytheclipse resilience primitives
(retry, circuit breaker, timeout, rate limit).

## Features

| Feature | Default | Backend | Description |
| :--- | :---: | :--- | :--- |
| `client` | yes | `reqwest` | HTTP client with timeout + tracing. |
| `server-axum` | no | `axum` | Axum-based server with health/metrics. |
| `server-hyper` | no | `hyper` | Low-level hyper server. |

## Usage

```toml
[dependencies]
mytheclipse-http = "0.2"
```

### Client with timeout

```rust
use mytheclipse_http::HttpClient;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    let client = HttpClient::new().with_timeout(Duration::from_secs(10));
    let resp = client.get("https://httpbin.org/get").await?;
    println!("status: {}", resp.status());
    Ok(())
}
```

### Server (axum)

```toml
[dependencies]
mytheclipse-http = { version = "0.2", features = ["server-axum"] }
```

```rust
use mytheclipse_http::HttpServer;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let server = HttpServer::new("0.0.0.0:3000".parse().unwrap());
    server.run().await;
}
```
