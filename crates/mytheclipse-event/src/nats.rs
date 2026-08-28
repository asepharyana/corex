//! A NATS-backed [`EventBus`] (feature `nats`), via the pure-Rust
//! `async-nats` client.
//!
//! Topics map directly onto NATS subjects.

use async_trait::async_trait;
use futures_util::StreamExt;

use crate::traits::{EventBus, EventError, Subscription, SubscriptionInner};

fn map_connect_err(e: async_nats::ConnectError) -> EventError {
    EventError::Io(e.to_string())
}

fn map_publish_err(e: async_nats::PublishError) -> EventError {
    EventError::Io(e.to_string())
}

fn map_subscribe_err(e: async_nats::SubscribeError) -> EventError {
    EventError::Io(e.to_string())
}

/// An [`EventBus`] backed by a NATS client.
#[derive(Clone)]
pub struct NatsEventBus {
    client: async_nats::Client,
}

impl NatsEventBus {
    /// Connects to a NATS server at `addr` (e.g. `nats://127.0.0.1:4222`).
    pub async fn connect(addr: &str) -> Result<Self, EventError> {
        let client = async_nats::connect(addr).await.map_err(map_connect_err)?;
        Ok(Self { client })
    }

    /// Wraps an already-connected client.
    pub fn from_client(client: async_nats::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl EventBus for NatsEventBus {
    async fn publish(&self, topic: &str, payload: Vec<u8>) -> Result<(), EventError> {
        self.client
            .publish(topic.to_string(), payload.into())
            .await
            .map_err(map_publish_err)
    }

    async fn subscribe(&self, topic: &str) -> Result<Subscription, EventError> {
        let subscriber = self
            .client
            .subscribe(topic.to_string())
            .await
            .map_err(map_subscribe_err)?;
        Ok(Subscription::new(Box::new(NatsSubscription { subscriber })))
    }
}

struct NatsSubscription {
    subscriber: async_nats::Subscriber,
}

#[async_trait]
impl SubscriptionInner for NatsSubscription {
    async fn recv(&mut self) -> Result<Vec<u8>, EventError> {
        match self.subscriber.next().await {
            Some(msg) => Ok(msg.payload.to_vec()),
            None => Err(EventError::Closed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Requires a live NATS server at `NATS_URL` (e.g. `nats://127.0.0.1:4222`).
    /// Run with: `NATS_URL=... cargo test -p corex-event --features nats -- --ignored`.
    #[tokio::test]
    #[ignore = "requires a live NATS instance (NATS_URL)"]
    async fn publish_subscribe_roundtrip_live() {
        let url = std::env::var("NATS_URL").expect("set NATS_URL");
        let bus = NatsEventBus::connect(&url).await.unwrap();
        let mut sub = bus.subscribe("orders.created").await.unwrap();
        // Give the subscription a moment to register server-side.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        bus.publish("orders.created", b"hello".to_vec())
            .await
            .unwrap();
        let msg = sub.recv().await.unwrap();
        assert_eq!(msg, b"hello");
    }
}
