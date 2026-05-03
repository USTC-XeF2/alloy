//! Milky protocol Bot implementation.
//!
//! This module provides `MilkyBot`, a concrete implementation of the `Bot` trait
//! that provides strongly-typed API methods for all Milky protocol v1.1 APIs.
//!
//! # Usage
//!
//! ```rust,ignore
//! use amira_adapter_milky::MilkyBot;
//! use amira_core::{BoxedBot, EventArc};
//!
//! async fn my_handler(bot: BoxedBot) {
//!     if let Some(milky) = bot.as_any().downcast_ref::<MilkyBot>() {
//!         let info = milky.get_login_info().await.unwrap();
//!         println!("Logged in as: {}", info.nickname);
//!     }
//! }
//! ```

use async_trait::async_trait;
use tracing::debug;

use crate::api::{SendGroupMessageExt, SendPrivateMessageExt};
use crate::model::api::ApiResponse;
use crate::model::message::OutgoingSegment;
use amira_core::error::{ApiError, ApiResult};
use amira_core::transport::{HttpRequestFn, Sender};
use amira_core::{ApiExecutor, ApiPayload, Bot, HttpMethod, Message, Scene, Sendable};

/// A Milky protocol Bot implementation.
///
/// Holds an optional [`HttpRequestFn`] that sends `POST /api/{action}` requests to
/// the Milky server.  If the bot was created from a receive-only connection
/// (e.g. HTTP webhook), `http_request` is `None` and API calls return
/// [`ApiError::NotSupported`].
pub struct MilkyBot {
    /// Bot ID (`self_id` from Milky events).
    id: String,
    /// HTTP request function for `/api/*` calls, or `None` for receive-only bots.
    http_request: Option<HttpRequestFn>,
}

impl MilkyBot {
    /// Creates a new `MilkyBot` from an optional sender.
    pub(crate) fn new(id: &str, sender: Option<Sender>) -> Self {
        let http_request = match sender {
            Some(Sender::HttpClient { http_request }) => Some(http_request),
            _ => None,
        };
        Self {
            id: id.into(),
            http_request,
        }
    }
}

#[async_trait]
impl Bot for MilkyBot {
    fn id(&self) -> &str {
        &self.id
    }

    async fn send(&self, scene: &Scene, message: &dyn Sendable) -> ApiResult<String> {
        let message = Message::<OutgoingSegment>::from_sendable(message).into_owned();

        match scene {
            Scene::Group { group_id, .. } => {
                if let Ok(group_id) = group_id.parse::<i64>() {
                    Some(self.send_group_message(group_id, message).await?)
                } else {
                    None
                }
            }
            Scene::Private { user_id } => {
                if let Ok(user_id) = user_id.parse::<i64>() {
                    Some(self.send_private_message(user_id, message).await?)
                } else {
                    None
                }
            }
            _ => None,
        }
        .map(|id| id.message_seq.to_string())
        .ok_or_else(|| ApiError::Other("unsupported scene for Milky".into()))
    }
}

impl ApiExecutor for MilkyBot {
    type Bot = Self;

    async fn execute<T: ApiPayload<Bot = Self>>(&self, payload: T) -> ApiResult<T::Response> {
        let http_request = self.http_request.as_ref().ok_or(ApiError::NotSupported)?;

        let path = format!("/api/{}", T::NAME);
        let body = serde_json::to_vec(&payload)?;

        debug!(action = %T::NAME, "Calling Milky API");
        let raw_resp = (http_request)(HttpMethod::POST, &path, body.into()).await?;

        match serde_json::from_slice(&raw_resp)? {
            ApiResponse::Ok { data } => Ok(data),
            ApiResponse::Failed { retcode, message } => {
                Err(ApiError::ApiError { retcode, message })
            }
        }
    }
}
