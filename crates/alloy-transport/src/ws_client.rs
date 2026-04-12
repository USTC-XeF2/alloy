//! WebSocket client capability implementation.

use std::sync::Arc;
use std::time::Duration;

use backon::{ExponentialBuilder, Retryable};
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::{
    Message,
    client::IntoClientRequest,
    http::header::{AUTHORIZATION, HeaderValue},
};
use tracing::{error, info, trace, warn};

use alloy_core::error::TransportResult;
use alloy_core::transport::{ConnectionHandler, Sender, WsClientConfig};
use alloy_macros::register_capability;

/// Builds a `backon::ExponentialBuilder` from a `WsClientConfig`.
fn build_backoff(config: &WsClientConfig) -> ExponentialBuilder {
    let mut builder = ExponentialBuilder::default()
        .with_min_delay(config.initial_delay.unwrap_or(Duration::from_secs(1)))
        .with_max_delay(config.max_delay.unwrap_or(Duration::from_secs(60)));

    if let Some(multiplier) = config.backoff_multiplier {
        builder = builder.with_factor(multiplier as f32);
    }

    if let Some(max_retries) = config.max_retries {
        builder = builder.with_max_times(max_retries as usize);
    } else {
        builder = builder.without_max_times();
    }

    builder
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

    // Register the bot with its send capability; get back the shutdown sender.
    let shutdown_tx = handler.register_connection(&bot_id, Some(Sender::Ws { message_tx }));

    let bot_id_cloned = bot_id.clone();

    // Spawn connection manager task
    tokio::spawn(async move {
        let backoff = build_backoff(&config);
        let url = config.url.clone();

        'connection_loop: loop {
            let bot_id_inner = bot_id_cloned.clone();
            let url_inner = url.clone();

            let connect_fn = async || {
                let mut request = url_inner.clone().into_client_request()?;
                if let Some(token) = &config.access_token {
                    let mut header_value = HeaderValue::from_str(&format!("Bearer {}", token))?;
                    header_value.set_sensitive(true);
                    request.headers_mut().insert(AUTHORIZATION, header_value);
                }
                connect_async(request).await
            };

            // Use backon to handle the initial connection and its retries.
            let connect_result = connect_fn
                .retry(backoff)
                .sleep(tokio::time::sleep)
                .notify(move |e, delay| {
                    warn!(bot_id = %bot_id_inner, error = %e, delay = ?delay, "Reconnecting...");
                })
                .await;

            if let Ok((ws_stream, _)) = connect_result {
                info!(bot_id = %bot_id_cloned, "WebSocket connected successfully");
                let (mut ws_tx, mut ws_rx) = ws_stream.split();

                loop {
                    tokio::select! {
                        // Check for shutdown
                        () = shutdown_tx.closed() => {
                            info!(bot_id = %bot_id_cloned, "WebSocket client shutting down");
                            let _ = ws_tx.close().await;
                            break 'connection_loop;
                        }

                        // Receive messages to send
                        Some(data) = message_rx.recv() => {
                            let msg = Message::Binary(data);
                            if let Err(e) = ws_tx.send(msg).await {
                                warn!(bot_id = %bot_id_cloned, error = %e, "Failed to send message, trigger reconnect");
                                break;
                            }
                        }

                        // Receive messages from server
                        msg = ws_rx.next() => {
                            match msg {
                                Some(Ok(Message::Text(text))) => {
                                    handler.on_message(&bot_id_cloned, text.into()).await;
                                }
                                Some(Ok(Message::Binary(data))) => {
                                    handler.on_message(&bot_id_cloned, data).await;
                                }
                                Some(Ok(Message::Ping(_))) => {
                                    trace!(bot_id = %bot_id_cloned, "Received ping");
                                }
                                Some(Ok(Message::Pong(_))) => {
                                    trace!(bot_id = %bot_id_cloned, "Received pong");
                                }
                                Some(Ok(Message::Close(_) | Message::Frame(_))) => {
                                    info!(bot_id = %bot_id_cloned, "Server closed connection");
                                    break;
                                }
                                Some(Err(e)) => {
                                    warn!(bot_id = %bot_id_cloned, error = %e, "WebSocket error");
                                    break;
                                }
                                None => {
                                    info!(bot_id = %bot_id_cloned, "WebSocket stream ended");
                                    break;
                                }
                            }
                        }
                    }
                }

                if !config.auto_reconnect {
                    break;
                }
            } else {
                error!(bot_id = %bot_id_cloned, "Reconnection failed after maximum retries, giving up");
                break;
            }
        }

        handler.on_disconnect(&bot_id_cloned).await;
    });

    Ok(bot_id)
}
