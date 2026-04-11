//! OneBot v11 Bot implementation.
//!
//! This module provides `OneBotBot`, a concrete implementation of the `Bot` trait
//! that provides strongly-typed API methods for all OneBot v11 APIs.
//!
//! # Overview
//!
//! Transport-specific strategies for OneBot v11 API calls:
//!
//! | Transport | Strategy |
//! |-----------|---------|
//! | WebSocket (server & client) | Async echo matching — request is tagged with a numeric echo; response arrives on the shared channel and is routed to the waiting future. |
//! | HTTP client | Synchronous POST — request body is sent as the HTTP body; the HTTP response body is the API response. No echo is needed. |
//! | Receive-only (`kind == None`) | Disabled — connections without a send capability cannot issue API calls. |
//!
//! # Usage
//!
//! ```rust,ignore
//! use alloy_adapter_onebot::OneBotBot;
//! use alloy_core::{BoxedBot, EventArc, FromContext};
//!
//! async fn my_handler(bot: BoxedBot, event: EventArc<MessageEvent>) {
//!     // Downcast to OneBotBot for strongly-typed APIs
//!     if let Ok(onebot) = bot.clone().downcast_arc::<OneBotBot>() {
//!         // Send a private message
//!         onebot.send_private_msg(12345678, "Hello!", false).await.ok();
//!         
//!         // Or use the generic send method (passes event directly)
//!         bot.send(event.as_ref(), "Reply!").await.ok();
//!     }
//! }
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use parking_lot::Mutex;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;
use tracing::{debug, warn};

use crate::model::message::OneBotMessage;
use alloy_core::{
    ApiError, ApiExecutor, ApiPayload, ApiResult, Bot, Bytes, HttpMethod, HttpRequestFn, Scene,
    Sendable, Sender,
};

#[derive(Debug, Deserialize)]
struct WsApiResponse {
    echo: u64,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
enum ApiResponse<T> {
    Ok {
        data: T,
    },
    Failed {
        retcode: i64,
        #[serde(default)]
        message: Option<String>,
        #[serde(default)]
        wording: Option<String>,
    },
}

// =============================================================================
// ApiCallStrategy
// =============================================================================

enum ApiCallStrategy {
    /// WebSocket caller with echo-based async routing
    Ws {
        message_tx: mpsc::Sender<Bytes>,
        pending_calls: Mutex<HashMap<u64, oneshot::Sender<Bytes>>>,
        echo_counter: AtomicU64,
        api_timeout: Duration,
    },
    /// HTTP client caller with direct request/response
    HttpClient { http_request: HttpRequestFn },
    /// Disabled caller for receive-only connections
    Disabled,
}

impl ApiCallStrategy {
    /// Creates a new strategy from a sender.
    fn new(sender: Option<Sender>) -> Self {
        match sender {
            Some(Sender::HttpClient { http_request }) => Self::HttpClient { http_request },
            Some(Sender::Ws { message_tx }) => Self::Ws {
                message_tx,
                pending_calls: Mutex::new(HashMap::new()),
                echo_counter: AtomicU64::new(1),
                api_timeout: Duration::from_secs(30),
            },
            None => Self::Disabled,
        }
    }

    /// Makes an API call and returns the response data.
    async fn call<T: ApiPayload>(&self, payload: T) -> ApiResult<ApiResponse<T::Response>> {
        match self {
            Self::Ws {
                message_tx,
                pending_calls,
                echo_counter,
                api_timeout,
            } => {
                let echo = echo_counter.fetch_add(1, Ordering::SeqCst);

                // Register pending response channel before sending so we never miss a
                // response that arrives before we start awaiting.
                let (tx, rx) = oneshot::channel();
                pending_calls.lock().insert(echo, tx);

                // Serialize and send the request.
                let request = json!({
                    "action": T::NAME,
                    "params": payload,
                    "echo": echo
                });

                debug!(action = %T::NAME, echo = %echo, "Calling OneBot API via WebSocket");

                let request_bytes = serde_json::to_vec(&request)?;
                if let Err(e) = message_tx.send(request_bytes.into()).await {
                    // Remove the pending entry so it doesn't dangle.
                    pending_calls.lock().remove(&echo);
                    return Err(ApiError::Other(format!("WebSocket send failed: {e}")));
                }

                // Await the response with a timeout.
                match timeout(*api_timeout, rx).await {
                    Ok(Ok(data)) => Ok(serde_json::from_slice(&data)?),
                    _ => {
                        // Timed out — remove the pending entry.
                        pending_calls.lock().remove(&echo);
                        Err(ApiError::Timeout)
                    }
                }
            }
            Self::HttpClient { http_request } => {
                let body = serde_json::to_vec(&json!({
                    "action": T::NAME,
                    "params": payload,
                }))?;

                debug!(action = %T::NAME, "Calling OneBot API via HTTP");

                let resp = (http_request)(HttpMethod::POST, "", body.into()).await?;

                Ok(serde_json::from_slice(&resp)?)
            }
            Self::Disabled => Err(ApiError::NotSupported),
        }
    }

    fn try_handle_response(&self, data: Bytes) -> Result<(), Bytes> {
        if let Self::Ws { pending_calls, .. } = self
            && let Ok(WsApiResponse { echo }) = serde_json::from_slice(&data)
        {
            let mut pending = pending_calls.lock();
            if let Some(tx) = pending.remove(&echo) {
                return tx.send(data);
            } else {
                // Echo arrived but no waiter — was probably already timed out.
                warn!(echo = %echo, "Received WS API response for unknown echo (timed out?)");
            }
        }

        Err(data)
    }

    /// Called when the underlying transport connection is closed.
    fn on_disconnect(&self) {
        if let Self::Ws { pending_calls, .. } = self {
            let mut pending = pending_calls.lock();
            let count = pending.len();
            if count > 0 {
                debug!(
                    count = count,
                    "Clearing pending WS API calls due to disconnect"
                );
                pending.clear();
            }
        }
    }
}

// =============================================================================
// OneBotBot
// =============================================================================

/// A OneBot v11 Bot implementation.
///
/// Wraps an [`ApiCallStrategy`] that handles the transport-specific request/response
/// strategy (WebSocket echo-matching or direct HTTP POST).
pub struct OneBotBot {
    /// Bot ID (self_id from events).
    id: String,
    /// Transport-specific API call mechanism (enum-based, no dyn dispatch).
    call_strategy: ApiCallStrategy,
}

impl OneBotBot {
    /// Creates a new `OneBotBot` from an optional sender.
    pub(crate) fn new(id: &str, sender: Option<Sender>) -> Self {
        Self {
            id: id.into(),
            call_strategy: ApiCallStrategy::new(sender),
        }
    }

    pub(crate) fn try_handle_response(&self, data: Bytes) -> Result<(), Bytes> {
        self.call_strategy.try_handle_response(data)
    }
}

#[async_trait]
impl Bot for OneBotBot {
    fn id(&self) -> &str {
        &self.id
    }

    async fn send(&self, scene: &Scene, message: &dyn Sendable) -> ApiResult<String> {
        let onebot_msg = OneBotMessage::from_erased_message(message).into_owned();

        let id: Option<i32> = match scene {
            Scene::Group { group_id, .. } => {
                if let Ok(group_id) = group_id.parse::<i64>() {
                    Some(self.send_group_msg(group_id, onebot_msg).await?.into())
                } else {
                    None
                }
            }
            Scene::Private { user_id } => {
                if let Ok(user_id) = user_id.parse::<i64>() {
                    Some(self.send_private_msg(user_id, onebot_msg).await?.into())
                } else {
                    None
                }
            }
            _ => None,
        };

        id.map(|id| id.to_string())
            .ok_or_else(|| ApiError::Other("unsupported scene for OneBot".into()))
    }

    async fn on_disconnect(&self) {
        self.call_strategy.on_disconnect();
    }
}

impl ApiExecutor for OneBotBot {
    async fn execute<T: ApiPayload<Client = Self>>(&self, payload: T) -> ApiResult<T::Response> {
        match self.call_strategy.call(payload).await? {
            ApiResponse::Ok { data } => Ok(data),
            ApiResponse::Failed {
                retcode,
                message,
                wording,
            } => {
                let message = message
                    .or(wording)
                    .unwrap_or_else(|| "Unknown error".into());
                Err(ApiError::ApiError { retcode, message })
            }
        }
    }
}
