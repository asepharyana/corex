//! A RabbitMQ-backed [`EventBus`] (feature `amqp`), via the pure-Rust `lapin`
//! AMQP 0.9.1 client.
//!
//! Topics map to routing keys on a single topic [`lapin::ExchangeKind::Topic`]
//! exchange (default name `corex.events`); each subscription declares its own
//! exclusive, auto-delete queue bound to that routing key, matching the
//! common "fanout via topic exchange" pattern.

use async_trait::async_trait;
use futures_lite::StreamExt;
use lapin::options::{
    BasicAckOptions, BasicConsumeOptions, BasicPublishOptions, ExchangeDeclareOptions,
    QueueBindOptions, QueueDeclareOptions,
};
use lapin::types::FieldTable;
use lapin::{BasicProperties, Channel, Connection, ConnectionProperties, ExchangeKind};

use crate::traits::{EventBus, EventError, Subscription, SubscriptionInner};

fn map_err(e: lapin::Error) -> EventError {
    EventError::Io(e.to_string())
}

/// An [`EventBus`] backed by a RabbitMQ topic exchange.
#[derive(Clone)]
pub struct AmqpEventBus {
    channel: Channel,
    exchange: String,
}

impl AmqpEventBus {
    /// Connects to `addr` (an AMQP URL, e.g. `amqp://guest:guest@localhost:5672/%2f`)
    /// and declares the topic exchange `exchange`.
    pub async fn connect(addr: &str, exchange: &str) -> Result<Self, EventError> {
        let conn = Connection::connect(addr, ConnectionProperties::default())
            .await
            .map_err(map_err)?;
        let channel = conn.create_channel().await.map_err(map_err)?;
        channel
            .exchange_declare(
                exchange,
                ExchangeKind::Topic,
                ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(map_err)?;
        Ok(Self {
            channel,
            exchange: exchange.to_string(),
        })
    }

    /// Wraps an already-open channel with a pre-declared exchange (advanced
    /// use: sharing a channel/connection across multiple buses).
    pub fn from_channel(channel: Channel, exchange: String) -> Self {
        Self { channel, exchange }
    }
}

#[async_trait]
impl EventBus for AmqpEventBus {
    async fn publish(&self, topic: &str, payload: Vec<u8>) -> Result<(), EventError> {
        self.channel
            .basic_publish(
                &self.exchange,
                topic,
                BasicPublishOptions::default(),
                &payload,
                BasicProperties::default(),
            )
            .await
            .map_err(map_err)?
            .await
            .map_err(map_err)?;
        Ok(())
    }

    async fn subscribe(&self, topic: &str) -> Result<Subscription, EventError> {
        let queue = self
            .channel
            .queue_declare(
                "", // server-generated name
                QueueDeclareOptions {
                    exclusive: true,
                    auto_delete: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(map_err)?;
        self.channel
            .queue_bind(
                queue.name().as_str(),
                &self.exchange,
                topic,
                QueueBindOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(map_err)?;
        let consumer = self
            .channel
            .basic_consume(
                queue.name().as_str(),
                "corex-event-consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(map_err)?;
        Ok(Subscription::new(Box::new(AmqpSubscription { consumer })))
    }
}

struct AmqpSubscription {
    consumer: lapin::Consumer,
}

#[async_trait]
impl SubscriptionInner for AmqpSubscription {
    async fn recv(&mut self) -> Result<Vec<u8>, EventError> {
        match self.consumer.next().await {
            Some(Ok(delivery)) => {
                let data = delivery.data.clone();
                delivery
                    .ack(BasicAckOptions::default())
                    .await
                    .map_err(map_err)?;
                Ok(data)
            }
            Some(Err(e)) => Err(map_err(e)),
            None => Err(EventError::Closed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Requires a live RabbitMQ at `AMQP_URL` (e.g.
    /// `amqp://guest:guest@127.0.0.1:5672/%2f`). Run with:
    /// `AMQP_URL=... cargo test -p corex-event --features amqp -- --ignored`.
    #[tokio::test]
    #[ignore = "requires a live RabbitMQ instance (AMQP_URL)"]
    async fn publish_subscribe_roundtrip_live() {
        let url = std::env::var("AMQP_URL").expect("set AMQP_URL");
        let bus = AmqpEventBus::connect(&url, "corex_event_test")
            .await
            .unwrap();
        let mut sub = bus.subscribe("orders.created").await.unwrap();
        bus.publish("orders.created", b"hello".to_vec())
            .await
            .unwrap();
        let msg = sub.recv().await.unwrap();
        assert_eq!(msg, b"hello");
    }
}
