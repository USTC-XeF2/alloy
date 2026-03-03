//! SSE (Server-Sent Events) client capability implementation.

use std::sync::Arc;

use eventsource_client::{Client, ClientBuilder, ReconnectOptions, SSE};
use futures::StreamExt;
use tokio_util::sync::CancellationToken;
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
    handler: Arc<dyn ConnectionHandler>,
    shutdown_token: CancellationToken,
) {
    info!(bot_id = %bot_id, url = %config.url, "SSE client starting");

    // ── Build eventsource client ─────────────────────────────────────────────
    let builder = match ClientBuilder::for_url(&config.url) {
        Ok(b) => b,
        Err(e) => {
            warn!(bot_id = %bot_id, error = %e, "Invalid SSE URL");
            handler.on_disconnect(&bot_id).await;
            return;
        }
    };

    let builder = if let Some(token) = &config.access_token {
        match builder.header("Authorization", &format!("Bearer {token}")) {
            Ok(b) => b,
            Err(e) => {
                warn!(bot_id = %bot_id, error = %e, "Failed to set Authorization header");
                handler.on_disconnect(&bot_id).await;
                return;
            }
        }
    } else {
        builder
    };

    let reconnect_opts = ReconnectOptions::reconnect(config.auto_reconnect)
        .retry_initial(false)
        .delay(config.initial_delay)
        .backoff_factor(2)
        .delay_max(config.max_delay)
        .build();

    let client = builder.reconnect(reconnect_opts).build();
    let mut stream = client.stream();

    // ── Event loop ──────────────────────────────────────────────────────────
    loop {
        tokio::select! {
            () = shutdown_token.cancelled() => {
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
                        // eventsource-client handles reconnect internally; a
                        // transient error here means it will retry on its own.
                        continue;
                    }
                    Some(Ok(SSE::Comment(_))) => {
                        // Ignore SSE comments.
                    }
                    Some(Ok(SSE::Connected(_))) => {
                        info!(bot_id = %bot_id, "SSE connection established");
                    }
                    Some(Ok(SSE::Event(ev))) => {
                        trace!(
                            bot_id = %bot_id,
                            event_type = %ev.event_type,
                            len = ev.data.len(),
                            "SSE event received"
                        );
                        handler.on_message(&bot_id, ev.data.as_bytes()).await;
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
    bot_id: String,
    config: SseClientConfig,
    handler: Arc<dyn ConnectionHandler>,
) -> TransportResult<()> {
    // Validate URL early.
    if config.url.is_empty() {
        return Err(TransportError::Io("SSE URL must not be empty".into()));
    }

    // Register the bot (SSE is receive-only, no Sender needed).
    let shutdown_token = handler.register_connection(&bot_id, None);

    // Spawn the persistent SSE loop.
    tokio::spawn(run_sse_loop(
        bot_id.clone(),
        config,
        handler,
        shutdown_token,
    ));

    Ok(())
}
