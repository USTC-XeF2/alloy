//! WebSocket client capability implementation.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};
use tracing::{error, info, trace, warn};

use alloy_core::{
    ClientConfig, ConnectionHandle, ConnectionHandler, ConnectionInfo, TransportError,
    TransportResult, WsClientCapability,
};

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsSink = SplitSink<WsStream, Message>;
type WsSource = SplitStream<WsStream>;

/// WebSocket client capability implementation.
pub struct WsClientCapabilityImpl;

impl WsClientCapabilityImpl {
    /// Creates a new WebSocket client capability.
    pub fn new() -> Self {
        Self
    }
}

impl Default for WsClientCapabilityImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl WsClientCapability for WsClientCapabilityImpl {
    async fn connect(
        &self,
        url: &str,
        handler: Arc<dyn ConnectionHandler>,
        config: ClientConfig,
    ) -> TransportResult<ConnectionHandle> {
        let url = url.to_string();

        // Create channels
        let (message_tx, message_rx) = mpsc::channel::<Vec<u8>>(256);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        // Initial connection
        let conn_info = ConnectionInfo::new("websocket").with_metadata("url", &url);

        info!(url = %url, "Connecting to WebSocket server");

        let (ws_stream, _response) =
            connect_async(&url)
                .await
                .map_err(|e| TransportError::ConnectionFailed {
                    url: url.clone(),
                    reason: format!("WebSocket connection failed: {}", e),
                })?;
        let (ws_tx, ws_rx) = ws_stream.split();

        // Get bot ID from handler
        let bot_id = handler.get_bot_id(conn_info).await?;

        info!(bot_id = %bot_id, url = %url, "WebSocket client connected");

        let handle = ConnectionHandle::new_ws(bot_id.clone(), message_tx, shutdown_tx);

        // Create and register the bot
        handler.create_bot(&bot_id, handle.clone()).await;

        // Spawn connection manager task
        tokio::spawn(run_client_loop(
            ws_tx,
            ws_rx,
            message_rx,
            shutdown_rx,
            handler,
            bot_id,
            url,
            config,
        ));

        Ok(handle)
    }
}

/// Runs the WebSocket client loop with reconnection support.
async fn run_client_loop(
    ws_tx: WsSink,
    ws_rx: WsSource,
    mut message_rx: mpsc::Receiver<Vec<u8>>,
    mut shutdown_rx: watch::Receiver<bool>,
    handler: Arc<dyn ConnectionHandler>,
    bot_id: String,
    url: String,
    config: ClientConfig,
) {
    let mut current_ws_tx = ws_tx;
    let mut current_ws_rx = ws_rx;
    let mut retry_count = 0u32;
    let mut current_delay = config.initial_delay;

    loop {
        tokio::select! {
            // Check for shutdown
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    info!(bot_id = %bot_id, "WebSocket client shutting down");
                    let _ = current_ws_tx.close().await;
                    handler.on_disconnect(&bot_id).await;
                    break;
                }
            }

            // Receive messages to send
            Some(data) = message_rx.recv() => {
                let msg = Message::Text(String::from_utf8_lossy(&data).to_string().into());
                if let Err(e) = current_ws_tx.send(msg).await {
                    warn!(bot_id = %bot_id, error = %e, "Failed to send message");
                }
            }

            // Receive messages from server
            msg = current_ws_rx.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        trace!(bot_id = %bot_id, len = text.len(), "Received text");
                        handler.on_message(&bot_id, text.as_bytes()).await;
                        // Reset retry on successful message
                        retry_count = 0;
                        current_delay = config.initial_delay;
                    }
                    Some(Ok(Message::Binary(data))) => {
                        trace!(bot_id = %bot_id, len = data.len(), "Received binary");
                        handler.on_message(&bot_id, &data).await;
                        retry_count = 0;
                        current_delay = config.initial_delay;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        trace!(bot_id = %bot_id, "Received ping, sending pong");
                        let _ = current_ws_tx.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Pong(_))) => {
                        trace!(bot_id = %bot_id, "Received pong");
                    }
                    Some(Ok(Message::Close(_))) | Some(Ok(Message::Frame(_))) => {
                        info!(bot_id = %bot_id, "Server closed connection");

                        if !config.auto_reconnect {
                            handler.on_disconnect(&bot_id).await;
                            break;
                        }

                        // Try to reconnect
                        if let Some(result) = try_reconnect(
                            &url,
                            &handler,
                            &bot_id,
                            &config,
                            &mut retry_count,
                            &mut current_delay,
                        ).await {
                            match result {
                                Ok((new_tx, new_rx)) => {
                                    current_ws_tx = new_tx;
                                    current_ws_rx = new_rx;
                                }
                                Err(_) => break,
                            }
                        } else {
                            break;
                        }
                    }
                    Some(Err(e)) => {
                        warn!(bot_id = %bot_id, error = %e, "WebSocket error");

                        if !config.auto_reconnect {
                            handler.on_disconnect(&bot_id).await;
                            break;
                        }

                        // Try to reconnect on error
                        if let Some(result) = try_reconnect(
                            &url,
                            &handler,
                            &bot_id,
                            &config,
                            &mut retry_count,
                            &mut current_delay,
                        ).await {
                            match result {
                                Ok((new_tx, new_rx)) => {
                                    current_ws_tx = new_tx;
                                    current_ws_rx = new_rx;
                                }
                                Err(_) => break,
                            }
                        } else {
                            break;
                        }
                    }
                    None => {
                        info!(bot_id = %bot_id, "WebSocket stream ended");

                        if !config.auto_reconnect {
                            handler.on_disconnect(&bot_id).await;
                            break;
                        }

                        // Try reconnect
                        if let Some(result) = try_reconnect(
                            &url,
                            &handler,
                            &bot_id,
                            &config,
                            &mut retry_count,
                            &mut current_delay,
                        ).await {
                            match result {
                                Ok((new_tx, new_rx)) => {
                                    current_ws_tx = new_tx;
                                    current_ws_rx = new_rx;
                                }
                                Err(_) => break,
                            }
                        } else {
                            break;
                        }
                    }
                }
            }
        }
    }
}

/// Attempts to reconnect with exponential backoff.
/// Returns None if max retries exceeded, otherwise returns the result of reconnection attempt.
async fn try_reconnect(
    url: &str,
    handler: &Arc<dyn ConnectionHandler>,
    bot_id: &str,
    config: &ClientConfig,
    retry_count: &mut u32,
    current_delay: &mut Duration,
) -> Option<Result<(WsSink, WsSource), ()>> {
    // Check max retries
    if let Some(max) = config.max_retries {
        if *retry_count >= max {
            error!(bot_id = %bot_id, "Max retries reached, giving up");
            handler.on_disconnect(bot_id).await;
            return None;
        }
    }

    warn!(bot_id = %bot_id, delay = ?current_delay, "Reconnecting...");
    tokio::time::sleep(*current_delay).await;

    match connect_async(url).await {
        Ok((new_stream, _)) => {
            let (new_tx, new_rx) = new_stream.split();
            info!(bot_id = %bot_id, "Reconnected successfully");
            *retry_count = 0;
            *current_delay = config.initial_delay;
            Some(Ok((new_tx, new_rx)))
        }
        Err(e) => {
            warn!(bot_id = %bot_id, error = %e, "Reconnection failed");
            *retry_count += 1;
            *current_delay = std::cmp::min(
                Duration::from_secs_f64(current_delay.as_secs_f64() * config.backoff_multiplier),
                config.max_delay,
            );

            // Return error but allow retry on next iteration
            Some(Err(()))
        }
    }
}
