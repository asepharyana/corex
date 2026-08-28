//! The core [`EventBus`] trait and byte-oriented [`Subscription`] handle.

use async_trait::async_trait;

/// Errors returned by event bus operations.
#[derive(Debug)]
pub enum EventError {
    /// The backend connection failed or was lost.
    Io(String),
    /// A value could not be serialized / deserialized.
    Serialization(String),
    /// The subscription's channel was closed (no more events will arrive).
    Closed,
}

impl std::fmt::Display for EventError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(s) => write!(f, "event bus io: {s}"),
            Self::Serialization(s) => write!(f, "event serialization: {s}"),
            Self::Closed => write!(f, "subscription closed"),
        }
    }
}

impl std::error::Error for EventError {}

/// A byte-oriented publish/subscribe bus.
///
/// Implementations back this with an in-memory broadcast channel or a
/// distributed broker (RabbitMQ, NATS, ...). Topics are opaque strings;
/// backends may map them onto exchanges/subjects as appropriate.
#[async_trait]
pub trait EventBus: Send + Sync {
    /// Publishes `payload` to `topic`. Whether publishing with zero
    /// subscribers is an error is backend-defined (the in-memory backend
    /// treats it as a no-op success).
    async fn publish(&self, topic: &str, payload: Vec<u8>) -> Result<(), EventError>;

    /// Subscribes to `topic`, returning a handle to receive future messages.
    async fn subscribe(&self, topic: &str) -> Result<Subscription, EventError>;
}

/// A handle to receive messages from a subscribed topic.
///
/// Wraps the backend-specific receiver behind one `recv` method so callers
/// don't need to know which [`EventBus`] implementation produced it.
pub struct Subscription {
    inner: Box<dyn SubscriptionInner>,
}

impl Subscription {
    /// Wraps a backend-specific receiver.
    ///
    /// Unused when no backend feature (`mem`/`amqp`/`nats`) is enabled.
    #[allow(dead_code)]
    pub(crate) fn new(inner: Box<dyn SubscriptionInner>) -> Self {
        Self { inner }
    }

    /// Awaits the next message on this subscription.
    pub async fn recv(&mut self) -> Result<Vec<u8>, EventError> {
        self.inner.recv().await
    }
}

/// Object-safe receiver behind [`Subscription`].
#[async_trait]
pub(crate) trait SubscriptionInner: Send {
    async fn recv(&mut self) -> Result<Vec<u8>, EventError>;
}
