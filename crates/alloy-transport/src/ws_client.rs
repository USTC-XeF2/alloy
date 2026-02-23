//! WebSocket client capability implementation.

use std::sync::Arc;
use std::time::Duration;

use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite::{Error, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};
use tracing::{error, info, trace, warn};

use alloy_core::{
    ConnectionHandle, ConnectionHandler, ConnectionInfo, TransportError, TransportResult,
    WsClientConfig,
};
use alloy_macros::register_capability;

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsSink = SplitSink<WsStream, Message>;
type WsSource = SplitStream<WsStream>;

/// State for managing WebSocket client loop interactions.
struct ClientLoopState {
    handler: Arc<dyn ConnectionHandler>,
    bot_id: String,
    config: WsClientConfig,
    retry_count: u32,
    current_delay: Duration,
    ws_tx: WsSink,
    ws_rx: WsSource,
}

impl ClientLoopState {
    /// Creates a new client loop state.
    fn new(
        handler: Arc<dyn ConnectionHandler>,
        bot_id: String,
        config: WsClientConfig,
        ws_stream: WsStream,
    ) -> Self {
        let initial_delay = config.initial_delay;
        let (ws_tx, ws_rx) = ws_stream.split();

        Self {
            handler,
            bot_id,
            config,
            retry_count: 0,
            current_delay: initial_delay,
            ws_tx,
            ws_rx,
        }
    }

    /// Handles incoming message and resets retry counters on success.
    async fn handle_message_received(&mut self, message_type: &str, data: &[u8]) {
        trace!(bot_id = %self.bot_id, len = data.len(), message_type = message_type, "Received");
        self.handler.on_message(&self.bot_id, data).await;
        self.retry_count = 0;
        self.current_delay = self.config.initial_delay;
    }

    /// Handles reconnection logic when connection is lost or error occurs.
    /// Returns true if should continue loop, false if should break.
    async fn handle_reconnect(&mut self) -> bool {
        if !self.config.auto_reconnect {
            self.handler.on_disconnect(&self.bot_id).await;
            return false;
        }

        // Check max retries
        if let Some(max) = self.config.max_retries {
            if self.retry_count >= max {
                error!(bot_id = %self.bot_id, "Max retries reached, giving up");
                self.handler.on_disconnect(&self.bot_id).await;
                return false;
            }
        }

        warn!(bot_id = %self.bot_id, delay = ?self.current_delay, "Reconnecting...");
        tokio::time::sleep(self.current_delay).await;

        match connect_async(&self.config.url).await {
            Ok((new_stream, _)) => {
                let (new_tx, new_rx) = new_stream.split();
                info!(bot_id = %self.bot_id, "Reconnected successfully");
                self.retry_count = 0;
                self.current_delay = self.config.initial_delay;
                self.ws_tx = new_tx;
                self.ws_rx = new_rx;
                true
            }
            Err(e) => {
                warn!(bot_id = %self.bot_id, error = %e, "Reconnection failed");
                self.retry_count += 1;
                self.current_delay = std::cmp::min(
                    Duration::from_secs_f64(
                        self.current_delay.as_secs_f64() * self.config.backoff_multiplier,
                    ),
                    self.config.max_delay,
                );
                false
            }
        }
    }

    /// Handles incoming WebSocket messages.
    /// Returns true if should continue loop, false if should break.
    async fn handle_message(&mut self, msg: Option<Result<Message, Error>>) -> bool {
        match msg {
            Some(Ok(Message::Text(text))) => {
                self.handle_message_received("text", text.as_bytes()).await;
                true
            }
            Some(Ok(Message::Binary(data))) => {
                self.handle_message_received("binary", &data).await;
                true
            }
            Some(Ok(Message::Ping(data))) => {
                trace!(bot_id = %self.bot_id, "Received ping, sending pong");
                let _ = self.ws_tx.send(Message::Pong(data)).await;
                true
            }
            Some(Ok(Message::Pong(_))) => {
                trace!(bot_id = %self.bot_id, "Received pong");
                true
            }
            Some(Ok(Message::Close(_))) | Some(Ok(Message::Frame(_))) => {
                info!(bot_id = %self.bot_id, "Server closed connection");
                self.handle_reconnect().await
            }
            Some(Err(e)) => {
                warn!(bot_id = %self.bot_id, error = %e, "WebSocket error");
                self.handle_reconnect().await
            }
            None => {
                info!(bot_id = %self.bot_id, "WebSocket stream ended");
                self.handle_reconnect().await
            }
        }
    }
}

/// Connects to a WebSocket server.
///
/// Creates channels, performs the initial connection, spawns a background loop
/// that handles send/receive and automatic reconnect per `config`.
///
/// This function is registered as the `WsConnectFn` capability.
#[register_capability(ws_client)]
pub async fn ws_connect(
    config: WsClientConfig,
    handler: Arc<dyn ConnectionHandler>,
) -> TransportResult<ConnectionHandle> {
    // Create channels
    let (message_tx, mut message_rx) = mpsc::channel::<Vec<u8>>(256);
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

    // Initial connection
    let conn_info = ConnectionInfo::new("websocket").with_metadata("url", &config.url);

    info!(url = %config.url, "Connecting to WebSocket server");

    let (ws_stream, _response) =
        connect_async(&config.url)
            .await
            .map_err(|e| TransportError::ConnectionFailed {
                url: config.url.clone(),
                reason: format!("WebSocket connection failed: {}", e),
            })?;

    // Get bot ID from handler
    let bot_id = handler.get_bot_id(conn_info).await?;

    info!(bot_id = %bot_id, url = %config.url, "WebSocket client connected");

    let handle = ConnectionHandle::new_ws(bot_id.clone(), message_tx, shutdown_tx);

    // Create and register the bot
    handler.create_bot(&bot_id, handle.clone()).await;

    let mut state = ClientLoopState::new(handler, bot_id, config, ws_stream);

    // Spawn connection manager task
    tokio::spawn(async move {
        loop {
            tokio::select! {
                // Check for shutdown
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        info!(bot_id = %state.bot_id, "WebSocket client shutting down");
                        let _ = state.ws_tx.close().await;
                        state.handler.on_disconnect(&state.bot_id).await;
                        break;
                    }
                }

                // Receive messages to send
                Some(data) = message_rx.recv() => {
                    let msg = Message::Text(String::from_utf8_lossy(&data).to_string().into());
                    if let Err(e) = state.ws_tx.send(msg).await {
                        warn!(bot_id = %state.bot_id, error = %e, "Failed to send message");
                    }
                }

                // Receive messages from server
                msg = state.ws_rx.next() => {
                    if !state.handle_message(msg).await {
                        break;
                    }
                }
            }
        }
    });

    Ok(handle)
}
