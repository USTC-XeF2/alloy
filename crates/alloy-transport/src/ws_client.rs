//! WebSocket client capability implementation.

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::{Error, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};
use tracing::{error, info, trace, warn};

use alloy_core::{ConnectionHandler, Sender, TransportError, TransportResult, WsClientConfig};
use alloy_macros::register_capability;

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// State for managing WebSocket client loop interactions.
struct ClientLoopState {
    handler: Arc<dyn ConnectionHandler>,
    bot_id: String,
    config: WsClientConfig,
    retry_count: u32,
    current_delay: Duration,
    ws_tx: SplitSink<WsStream, Message>,
    ws_rx: SplitStream<WsStream>,
}

impl ClientLoopState {
    /// Creates a new client loop state.
    fn new(
        handler: Arc<dyn ConnectionHandler>,
        bot_id: String,
        config: WsClientConfig,
        ws_stream: WsStream,
    ) -> Self {
        let mut current_delay = Duration::from_secs(1);
        if let Some(delay) = config.initial_delay {
            current_delay = delay;
        }
        let (ws_tx, ws_rx) = ws_stream.split();

        Self {
            handler,
            bot_id,
            config,
            retry_count: 0,
            current_delay,
            ws_tx,
            ws_rx,
        }
    }

    /// Handles incoming message and resets retry counters on success.
    async fn handle_message_received(&mut self, data: Bytes) {
        self.handler.on_message(&self.bot_id, data).await;
        self.retry_count = 0;
        self.current_delay = Duration::from_secs(1);
        if let Some(delay) = self.config.initial_delay {
            self.current_delay = delay;
        }
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
                self.current_delay = Duration::from_secs(1);
                if let Some(delay) = self.config.initial_delay {
                    self.current_delay = delay;
                }
                self.ws_tx = new_tx;
                self.ws_rx = new_rx;
                true
            }
            Err(e) => {
                warn!(bot_id = %self.bot_id, error = %e, "Reconnection failed");
                self.retry_count += 1;
                let mut multiplier = 2.0;
                if let Some(m) = self.config.backoff_multiplier {
                    multiplier = m;
                }
                let mut max_delay = Duration::from_secs(60);
                if let Some(delay) = self.config.max_delay {
                    max_delay = delay;
                }
                self.current_delay = std::cmp::min(
                    Duration::from_secs_f64(self.current_delay.as_secs_f64() * multiplier),
                    max_delay,
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
                self.handle_message_received(text.into()).await;
                true
            }
            Some(Ok(Message::Binary(data))) => {
                self.handle_message_received(data).await;
                true
            }
            Some(Ok(Message::Ping(_))) => {
                trace!(bot_id = %self.bot_id, "Received ping");
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
    bot_id: String,
) -> TransportResult<String> {
    // Create channels
    let (message_tx, mut message_rx) = mpsc::channel::<Bytes>(256);

    let (ws_stream, _response) =
        connect_async(&config.url)
            .await
            .map_err(|e| TransportError::ConnectionFailed {
                url: config.url.clone(),
                reason: format!("WebSocket connection failed: {}", e),
            })?;

    info!(bot_id = %bot_id, url = %config.url, "WebSocket client connected");

    // Register the bot with its send capability; get back the shutdown sender.
    let shutdown_tx = handler.register_connection(&bot_id, Some(Sender::Ws { message_tx }));

    let mut state = ClientLoopState::new(handler, bot_id.clone(), config, ws_stream);

    // Spawn connection manager task
    tokio::spawn(async move {
        loop {
            tokio::select! {
                // Check for shutdown
                _ = shutdown_tx.closed() => {
                    info!(bot_id = %state.bot_id, "WebSocket client shutting down");
                    let _ = state.ws_tx.close().await;
                    state.handler.on_disconnect(&state.bot_id).await;
                    break;
                }

                // Receive messages to send
                Some(data) = message_rx.recv() => {
                    let msg = Message::Binary(data);
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

    Ok(bot_id)
}
