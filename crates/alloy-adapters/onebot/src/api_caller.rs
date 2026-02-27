//! Transport-specific [`ApiCaller`] implementations for OneBot v11.
//!
//! # Overview
//!
//! OneBot v11 supports two transport families, each with a different
//! request/response pattern:
//!
//! | Transport | Caller | Strategy |
//! |-----------|--------|---------|
//! | WebSocket (server & client) | [`WsApiCaller`] | Async echo matching — request is tagged with a numeric echo; response arrives on the shared channel and is routed to the waiting future. |
//! | HTTP client | [`HttpApiCaller`] | Synchronous POST — request body is sent as the HTTP body; the HTTP response body is the API response. No echo is needed. |
//!
//! [`OneBotBot`](crate::bot::OneBotBot) holds an `Arc<dyn ApiCaller>` and is
//! completely unaware of which transport is in use.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;
use tracing::{debug, warn};

use alloy_core::{ApiError, ApiResult, PostJsonFn, TransportError};

// =============================================================================
// ApiCaller trait — internal abstraction for transport-specific call strategies
// =============================================================================

/// Transport-specific API call mechanism.
///
/// Decouples how an API call is made and its response received from the bot and
/// protocol logic. Implemented differently per transport:
///
/// | Transport | Strategy |
/// |-----------|----------|
/// | WebSocket (server/client) | Echo-ID async matching — request tagged with numeric `echo`; response routed to waiting future by [`on_incoming_response`](ApiCaller::on_incoming_response). |
/// | HTTP client | Synchronous POST — one request produces one response; no in-band routing needed. |
#[async_trait]
pub trait ApiCaller: Send + Sync {
    /// Makes an API call and returns the response data.
    ///
    /// # Arguments
    /// * `action` – Protocol action name (e.g. `"send_private_msg"`).
    /// * `params` – JSON parameters for the action.
    ///
    /// # Errors
    /// Returns an [`ApiError`] if the call fails, times out, or the
    /// connection is lost.
    async fn call(&self, _action: &str, _params: Value) -> ApiResult<Value> {
        Err(ApiError::NotSupported)
    }

    /// Routes an incoming protocol message that is an API response.
    ///
    /// Called when an incoming message carries an `echo` field (WebSocket).
    /// The implementation locates the matching pending call and sends the
    /// response to the waiting future.
    ///
    /// Returns `true` if consumed as a response (adapter skips event parsing).
    ///
    /// The **default returns `false`** — correct for HTTP where responses
    /// arrive synchronously inside [`call`](ApiCaller::call).
    fn on_incoming_response(&self, _data: &Value) -> bool {
        false
    }

    /// Called when the underlying transport connection is closed.
    ///
    /// Implementations should unblock any pending [`call`](ApiCaller::call)
    /// futures with a [`ApiError::NotConnected`] error.
    ///
    /// The default implementation is a no-op.
    fn on_disconnect(&self) {}
}

// =============================================================================
// DisabledApiCaller — placeholder for transports that cannot issue API calls
// =============================================================================

/// [`ApiCaller`] for transports that do not support API calls.
///
/// Used by HTTP server connections (which are receive-only).
/// Any attempt to call an API will return an error.
pub struct DisabledApiCaller;

impl DisabledApiCaller {
    /// Creates a new disabled caller with the given reason message.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ApiCaller for DisabledApiCaller {}

// =============================================================================
// WsApiCaller — echo-based async request/response for WebSocket
// =============================================================================

/// [`ApiCaller`] for WebSocket transports (both server and client mode).
///
/// Each [`call`](ApiCaller::call) invocation:
/// 1. Generates a unique numeric echo ID.
/// 2. Registers a one-shot channel keyed on that ID in the pending map.
/// 3. Sends the JSON request (with the `echo` field) through the WebSocket
///    write channel.
/// 4. Awaits the one-shot receiver, which is resolved by
///    [`on_incoming_response`](ApiCaller::on_incoming_response) when the
///    matching response arrives from the peer.
pub struct WsApiCaller {
    /// WebSocket write channel — sends serialized JSON bytes to the WS loop.
    message_tx: mpsc::Sender<Vec<u8>>,
    /// Pending call map: echo_id → sender half of the response channel.
    pending_calls: Arc<Mutex<HashMap<u64, oneshot::Sender<Value>>>>,
    /// Monotonically increasing echo counter.
    echo_counter: AtomicU64,
    /// How long to wait for a response before giving up.
    api_timeout: Duration,
}

impl WsApiCaller {
    /// Creates a new `WsApiCaller` from the WebSocket write channel.
    pub fn new(message_tx: mpsc::Sender<Vec<u8>>) -> Self {
        Self {
            message_tx,
            pending_calls: Arc::new(Mutex::new(HashMap::new())),
            echo_counter: AtomicU64::new(1),
            api_timeout: Duration::from_secs(30),
        }
    }
}

#[async_trait]
impl ApiCaller for WsApiCaller {
    async fn call(&self, action: &str, params: Value) -> ApiResult<Value> {
        let echo = self.echo_counter.fetch_add(1, Ordering::SeqCst);

        // Register pending response channel before sending so we never miss a
        // response that arrives before we start awaiting.
        let (tx, rx) = oneshot::channel();
        self.pending_calls.lock().unwrap().insert(echo, tx);

        // Serialize and send the request.
        let request = json!({
            "action": action,
            "params": params,
            "echo": echo
        });

        debug!(action = %action, echo = %echo, "Calling OneBot API via WebSocket");

        let request_bytes = serde_json::to_vec(&request)?;
        if let Err(e) = self.message_tx.send(request_bytes).await {
            // Remove the pending entry so it doesn't dangle.
            self.pending_calls.lock().unwrap().remove(&echo);
            return Err(TransportError::SendFailed(e.to_string()).into());
        }

        // Await the response with a timeout.
        match timeout(self.api_timeout, rx).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(_)) => {
                // Channel closed — transport was shut down.
                Err(ApiError::NotConnected)
            }
            Err(_) => {
                // Timed out — remove the pending entry.
                self.pending_calls.lock().unwrap().remove(&echo);
                Err(ApiError::Timeout)
            }
        }
    }

    fn on_incoming_response(&self, data: &Value) -> bool {
        let Some(echo) = data.get("echo").and_then(Value::as_u64) else {
            return false;
        };
        let mut pending = self.pending_calls.lock().unwrap();
        if let Some(tx) = pending.remove(&echo) {
            let _ = tx.send(data.clone());
            true
        } else {
            // Echo arrived but no waiter — was probably already timed out.
            warn!(echo = %echo, "Received WS API response for unknown echo (timed out?)");
            false
        }
    }

    fn on_disconnect(&self) {
        let mut pending = self.pending_calls.lock().unwrap();
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

// =============================================================================
// HttpApiCaller — direct synchronous POST for HTTP transport
// =============================================================================

/// [`ApiCaller`] for HTTP client transport.
///
/// Delegates every call to the `post_json` closure supplied by the transport
/// layer.  The closure already encapsulates the target URL and any
/// authentication, so `HttpApiCaller` itself is stateless apart from the fn.
pub struct HttpApiCaller {
    /// Transport-supplied closure: captures URL + auth, accepts a JSON body.
    post_json: PostJsonFn,
}

impl HttpApiCaller {
    /// Creates a new `HttpApiCaller`.
    pub fn new(post_json: PostJsonFn) -> Self {
        Self { post_json }
    }
}

#[async_trait]
impl ApiCaller for HttpApiCaller {
    async fn call(&self, action: &str, params: Value) -> ApiResult<Value> {
        let body = json!({
            "action": action,
            "params": params,
        });

        debug!(action = %action, "Calling OneBot API via HTTP");

        let response_json = (self.post_json)(body)
            .await
            .map_err(|e| ApiError::Other(format!("HTTP request failed: {e}")))?;

        Ok(response_json)
    }
}
