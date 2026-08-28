//! # mytheclipse-event
//!
//! A unified events & message bus abstraction so component-to-component (or
//! service-to-service) communication doesn't get locked to one transport.
//!
//! - **In-memory dispatcher** ([`memory::InMemoryEventBus`], feature `mem`,
//!   default): a `tokio::sync::broadcast`-backed pub/sub bus for
//!   intra-process communication.
//! - **Distributed broker backends**: [`amqp::AmqpEventBus`] (RabbitMQ,
//!   feature `amqp`) and [`nats::NatsEventBus`] (NATS, feature `nats`) behind
//!   the same [`EventBus`] trait, so swapping the backend does not change
//!   handler code.
//! - **Typed convenience**: [`typed::TypedEventBus`] layers JSON
//!   (de)serialization on top of the byte-oriented [`EventBus`].
//!
//! ## Example
//!
//! The in-memory + typed bus (`mem` feature, on by default):
//!
//! ```no_run
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Debug, Serialize, Deserialize, PartialEq)]
//! struct OrderCreated { id: u64 }
//!
//! #[cfg(feature = "mem")]
//! #[tokio::main]
//! async fn main() {
//!     use mytheclipse_event::{EventBus, InMemoryEventBus, TypedEventBus};
//!     let bus = TypedEventBus::new(InMemoryEventBus::default());
//!     let mut sub = bus.subscribe::<OrderCreated>("orders").await.unwrap();
//!     bus.publish("orders", &OrderCreated { id: 42 }).await.unwrap();
//!     let event = sub.recv().await.unwrap();
//!     assert_eq!(event, OrderCreated { id: 42 });
//! }
//!
//! #[cfg(not(feature = "mem"))]
//! fn main() {}
//! ```

pub mod traits;

#[cfg(feature = "mem")]
pub mod memory;

#[cfg(feature = "mem")]
pub mod typed;

#[cfg(feature = "amqp")]
pub mod amqp;

#[cfg(feature = "nats")]
pub mod nats;

pub use traits::{EventBus, EventError, Subscription};

#[cfg(feature = "mem")]
pub use memory::InMemoryEventBus;

#[cfg(feature = "mem")]
pub use typed::{Event, TypedEventBus, TypedSubscription};

#[cfg(feature = "amqp")]
pub use amqp::AmqpEventBus;

#[cfg(feature = "nats")]
pub use nats::NatsEventBus;
