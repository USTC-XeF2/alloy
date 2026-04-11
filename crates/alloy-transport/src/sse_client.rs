//! SSE (Server-Sent Events) client capability implementation.

use std::sync::Arc;

use eventsource_client::{Client, ClientBuilder, ReconnectOptions, SSE};
use futures::StreamExt;
use launchdarkly_sdk_transport::HyperTransport;
use tokio::sync::watch;
use tracing::{info, trace, warn};

use alloy_core::{ConnectionHandler, SseClientConfig, TransportError, TransportResult};
use alloy_macros::register_capability;

// =============================================================================
// SSE event loop
// =============================================================================

/// Persistent SSE client loop.
///
/// Opens an SSE connection to the configured URL, processes incoming events,
/// and forwards each `data` payload to [`ConnectionHandler::on_message`].
///
/// Reconnection is delegated to `eventsource-client` via [`ReconnectOptions`].
/// The loop exits when `shutdown_token` is cancelled.
async fn run_sse_loop(
    bot_id: String,
    config: SseClientConfig,
    client: impl Client,
    handler: Arc<dyn ConnectionHandler>,
    shutdown_tx: watch::Sender<()>,
) {
    info!(bot_id = %bot_id, url = %config.url, "SSE client starting");

    let mut retry_count = 0u32;
    let mut stream = client.stream();

    // ── Event loop ──────────────────────────────────────────────────────────
    loop {
        tokio::select! {
            _ = shutdown_tx.closed() => {
                info!(bot_id = %bot_id, "SSE client shutting down");
                break;
            }

            event = stream.next() => {
                match event {
                    None => {
                        info!(bot_id = %bot_id, "SSE stream ended");
                        break;
                    }
                    Some(Err(e)) => {
                        warn!(bot_id = %bot_id, error = %e, "SSE stream error");
                        if !config.auto_reconnect {
                            break;
                        }

                        // Check max retries
                        if let Some(max) = config.max_retries {
                            retry_count += 1;
                            if retry_count > max {
                                warn!(bot_id = %bot_id, retries = retry_count, max = max, "Max retries exceeded, shutting down");
                                break;
                            }
                        }

                        continue;
                    }
                    Some(Ok(sse)) => {
                        if let SSE::Event(ev) = sse {
                            trace!(
                                bot_id = %bot_id,
                                event_type = %ev.event_type,
                                len = ev.data.len(),
                                "SSE event received"
                            );
                            handler.on_message(&bot_id, ev.data.into()).await;
                        }
                        retry_count = 0;
                    }
                }
            }
        }
    }

    handler.on_disconnect(&bot_id).await;
    info!(bot_id = %bot_id, "SSE client stopped");
}

// =============================================================================
// Capability registration
// =============================================================================

/// Registers an SSE client bot.
///
/// Calls [`ConnectionHandler::register_connection`] to register the bot
/// (receive-only — no [`Sender`] is attached), then spawns a background task
/// that continuously reads SSE events from the configured URL and forwards each
/// `data` payload to [`ConnectionHandler::on_message`].
///
/// Reconnection behaviour is controlled by [`SseClientConfig::auto_reconnect`]
/// and is handled transparently by the `eventsource-client` library.
///
/// This function is registered as the `SseClientFn` capability.
#[register_capability(sse_client)]
pub async fn sse_start_client(
    config: SseClientConfig,
    handler: Arc<dyn ConnectionHandler>,
    bot_id: String,
) -> TransportResult<String> {
    let builder = ClientBuilder::for_url(&config.url)
        .map_err(|e| TransportError::Serialization(e.to_string()))?;

    let builder = if let Some(token) = &config.access_token {
        builder
            .header("Authorization", &format!("Bearer {token}"))
            .map_err(|e| TransportError::Serialization(e.to_string()))?
    } else {
        builder
    };

    let mut reconnect_opts =
        ReconnectOptions::reconnect(config.auto_reconnect).retry_initial(false);

    if let Some(delay) = config.initial_delay {
        reconnect_opts = reconnect_opts.delay(delay);
    }
    if let Some(delay) = config.max_delay {
        reconnect_opts = reconnect_opts.delay_max(delay);
    }
    if let Some(m) = config.backoff_multiplier {
        reconnect_opts = reconnect_opts.backoff_factor(m as u32);
    }
    let reconnect_opts = reconnect_opts.build();
    let builder = builder.reconnect(reconnect_opts);

    let transport = HyperTransport::builder().build_http()?;
    let client = builder.build_with_transport(transport);

    // Register the bot (SSE is receive-only, no Sender needed).
    let shutdown_token = handler.register_connection(&bot_id, None);

    // Spawn the persistent SSE loop.
    tokio::spawn(run_sse_loop(
        bot_id.clone(),
        config,
        client,
        handler,
        shutdown_token,
    ));

    Ok(bot_id)
}
