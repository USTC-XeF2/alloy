//! Bot trait and related types.
//!
//! This module defines the `Bot` trait which represents an active bot instance
//! that can receive events and send messages.

use std::future::{Future, IntoFuture};
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use downcast_rs::{DowncastSync, impl_downcast};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::ApiResult;
use crate::event::Scene;
use crate::message::Sendable;

/// The core Bot trait.
///
/// A Bot is an active instance that:
/// - Receives events from the runtime
/// - Processes events through handlers
/// - Sends messages back through the transport
///
/// Each bot instance is associated with an adapter that defines
/// how protocol-specific messages are parsed and serialized.
///
/// # API Design
///
/// - `call_api`: Raw API call with action name and JSON parameters
/// - `send`: Unified message sending that extracts session from event
///
/// Concrete implementations (e.g., `OneBotBot`) should provide
/// strongly-typed API methods on top of `call_api`.
#[async_trait]
pub trait Bot: DowncastSync + ApiExecutor + 'static {
    /// Returns the bot's unique identifier.
    fn id(&self) -> &str;

    /// Sends a message in a given scene.
    ///
    /// # Arguments
    ///
    /// * `scene` - The scene to send the message to
    /// * *`message` - The message content to send
    ///
    /// # Returns
    ///
    /// The message ID if successful.
    async fn send(&self, scene: &Scene, message: &dyn Sendable) -> ApiResult<String>;

    /// Called when the transport connection is lost.
    ///
    /// Implementations should clean up any pending state, such as:
    /// - Pending API call responses (notify waiters of disconnection)
    /// - Cached session data
    /// - Protocol-specific cleanup
    ///
    /// This is called by [`AdapterBridge`](crate::adapter::AdapterBridge)
    /// before the bot is unregistered from the [`BotManager`].
    ///
    /// The default implementation does nothing.
    async fn on_disconnect(&self) {}
}

impl_downcast!(sync Bot);

/// A boxed Bot trait object.
pub type BoxedBot = Arc<dyn Bot>;

// ----------------------------------------------------------------------------
// API Interface
// ----------------------------------------------------------------------------

/// The `ApiExecutor` trait defines the capability to execute API requests.
pub trait ApiExecutor {
    /// Executes an API request with the given payload.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The type of the API payload, which must implement [`ApiPayload`].
    ///
    /// # Returns
    ///
    /// A future that resolves to an [`ApiResult`] containing the response.
    fn execute<T>(&self, payload: T) -> impl Future<Output = ApiResult<T::Response>> + Send
    where
        Self: Sized,
        T: ApiPayload<Client = Self>;
}

pub trait ApiPayload: Sized + Send + Serialize {
    const NAME: &'static str;

    type Client: ApiExecutor;
    type Response: DeserializeOwned;

    fn build(self, client: &Self::Client) -> ApiRequest<'_, Self> {
        ApiRequest::new(client, self)
    }
}

/// A wrapper for an API request that combines a client and a payload.
///
/// This structure implements [`IntoFuture`], allowing it to be awaited directly.
pub struct ApiRequest<'a, T>
where
    T: ApiPayload,
{
    client: &'a T::Client,
    payload: T,
}

impl<T> ApiRequest<'_, T>
where
    T: ApiPayload,
{
    pub fn new(client: &T::Client, payload: T) -> ApiRequest<'_, T> {
        ApiRequest { client, payload }
    }

    pub fn payload_mut(&mut self) -> &mut T {
        &mut self.payload
    }
}

impl<'a, T> IntoFuture for ApiRequest<'a, T>
where
    T: ApiPayload + 'a,
{
    type Output = ApiResult<T::Response>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.client.execute(self.payload))
    }
}
