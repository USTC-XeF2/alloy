use std::marker::PhantomData;
use std::sync::Arc;
use std::task::{Context, Poll};

use clap::Parser;
use clap::error::ErrorKind;
use futures::FutureExt;
use futures::future::BoxFuture;
use tower::{BoxError, Layer, Service, ServiceBuilder};
use tower_layer::{Identity, Stack};

use crate::context::AlloyContext;
use crate::error::EventSkipped;
use crate::handler::{Handler, HandlerService, ServiceBuilderExt};
use alloy_core::EventType;

use super::CURRENT_REGISTRY;
use super::extractor::ParsedCommand;
use super::split::rich_text_shell_split;

/// Creates a tower [`Layer`] that parses messages as the given clap command.
///
/// `on_command::<T>(name)` returns a [`CommandLayer<T>`] which can be used in two ways:
/// 1. Call `.handler(f)` directly for the common case
/// 2. Call `.build()` to get a `ServiceBuilder` for more advanced configurations
///
/// # Type Parameters
///
/// - `T`: A type that implements `clap::Parser`
///
/// # Arguments
///
/// - `name`: The command name without "/" prefix (e.g., `"echo"` matches `/echo`)
///
/// # Example
///
/// ```rust,ignore
/// // Simple usage with handler
/// runtime.register_service(
///     on_command::<EchoCommand>("echo").handler(echo_handler)
/// ).await;
///
/// // Adjust reply behaviour then use handler
/// runtime.register_service(
///     on_command::<EchoCommand>("echo")
///         .reply_error(false)
///         .handler(echo_handler)
/// ).await;
///
/// // Advanced: build with additional layers
/// runtime.register_service(
///     on_command::<EchoCommand>("echo")
///         .build()
///         .layer(some_other_layer)
///         .handler(echo_handler)
/// ).await;
/// ```
pub fn on_command<T>(name: impl Into<String>) -> CommandLayer<T>
where
    T: Parser + Clone + Send + Sync + 'static,
{
    CommandLayer::new(name)
}

/// A tower [`Layer`] that parses messages as a clap command before calling the
/// inner service.
///
/// Produced by [`on_command`]. Builder methods adjust error-reply behaviour;
/// finalise by calling `.layer(HandlerService::new(my_handler))`.
#[derive(Clone)]
pub struct CommandLayer<T>
where
    T: Parser + Clone + Send + Sync + 'static,
{
    name: String,
    reply_help: bool,
    reply_error: bool,
    block: bool,
    _marker: PhantomData<T>,
}

impl<T> CommandLayer<T>
where
    T: Parser + Clone + Send + Sync + 'static,
{
    /// Creates a new [`CommandLayer`] with `reply_help` and `reply_error` both
    /// enabled by default.
    ///
    /// Prefer [`on_command`] for the common case. Use this constructor directly
    /// when you need to adjust reply behaviour before stacking the layer:
    /// ```rust,ignore
    /// CommandLayer::new("echo").reply_error(false).handler(echo_handler)
    /// ```
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            reply_help: true,
            reply_error: true,
            block: true,
            _marker: PhantomData,
        }
    }

    /// Enable/disable automatic help replies (default: `true`).
    pub fn reply_help(mut self, enabled: bool) -> Self {
        self.reply_help = enabled;
        self
    }

    /// Enable/disable automatic error replies (default: `true`).
    pub fn reply_error(mut self, enabled: bool) -> Self {
        self.reply_error = enabled;
        self
    }

    /// Enable/disable event propagation blocking (default: `true`).
    ///
    /// When enabled, the command layer will call `ctx.stop_propagation()` after
    /// successfully parsing the command, preventing other handlers from running.
    pub fn block(mut self, enabled: bool) -> Self {
        self.block = enabled;
        self
    }

    /// Convert to a [`ServiceBuilder`] for more advanced configurations.
    pub fn build(self) -> ServiceBuilder<Stack<CommandLayer<T>, Identity>> {
        ServiceBuilder::new().layer(self)
    }

    /// Wrap a handler function with this command layer.
    ///
    /// This is equivalent to `.build().handler(handler)` but more concise.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// on_command::<MyCmd>("cmd")
    ///     .reply_error(false)
    ///     .handler(my_handler)
    /// ```
    pub fn handler<H, U>(self, handler: H) -> CommandService<T, HandlerService<H, U>>
    where
        H: Handler<U>,
    {
        self.build().handler(handler)
    }
}

impl<T, S> Layer<S> for CommandLayer<T>
where
    T: Parser + Clone + Send + Sync + 'static,
{
    type Service = CommandService<T, S>;

    fn layer(&self, inner: S) -> CommandService<T, S> {
        CommandService {
            name: self.name.clone(),
            reply_help: self.reply_help,
            reply_error: self.reply_error,
            block: self.block,
            inner,
            _marker: PhantomData,
        }
    }
}

/// The [`Service`] produced by [`CommandLayer`].
///
/// Parses the command from the event on every call. If parsing succeeds the
/// parsed value is stored in context (via [`CommandArgs`]) and the inner
/// service is called; otherwise the event is dropped (or an error/help reply
/// is sent if the corresponding option is enabled).
pub struct CommandService<T, S> {
    name: String,
    reply_help: bool,
    reply_error: bool,
    block: bool,
    inner: S,
    _marker: PhantomData<T>,
}

impl<T, S: Clone> Clone for CommandService<T, S> {
    fn clone(&self) -> Self {
        CommandService {
            name: self.name.clone(),
            reply_help: self.reply_help,
            reply_error: self.reply_error,
            block: self.block,
            inner: self.inner.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T, S> Service<Arc<AlloyContext>> for CommandService<T, S>
where
    T: Parser + Clone + Send + Sync + 'static,
    S: Service<Arc<AlloyContext>, Response = (), Error = BoxError> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = ();
    type Error = BoxError;
    type Future = BoxFuture<'static, Result<(), Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, ctx: Arc<AlloyContext>) -> Self::Future {
        let name = self.name.clone();
        let reply_help = self.reply_help;
        let reply_error = self.reply_error;
        let block = self.block;
        let mut inner = self.inner.clone();

        async move {
            if ctx.event().event_type() != EventType::Message {
                return Err(Box::new(EventSkipped) as BoxError);
            }

            let rich_text = ctx.event().get_rich_text();
            let (args, registry) = rich_text_shell_split(&rich_text);

            let expected_cmd = format!("/{name}");
            if args.is_empty() || args[0].to_lowercase() != expected_cmd.to_lowercase() {
                return Err(Box::new(EventSkipped) as BoxError);
            }

            CURRENT_REGISTRY.with(|reg| {
                *reg.borrow_mut() = Some(registry);
            });
            let result = T::try_parse_from(&args);
            CURRENT_REGISTRY.with(|reg| {
                *reg.borrow_mut() = None;
            });

            if block {
                ctx.stop_propagation();
            }

            match result {
                Ok(cmd) => {
                    ctx.set_state(ParsedCommand(cmd));
                    inner.call(ctx).await
                }
                Err(err) => {
                    let should_reply = if err.kind() == ErrorKind::DisplayHelp {
                        reply_help
                    } else {
                        reply_error
                    };
                    if should_reply {
                        let bot = ctx.bot_arc();
                        let event = ctx.event().clone();
                        let msg = err.to_string();
                        let _ = bot.send(event.as_ref(), &msg).await;
                    }
                    Ok(())
                }
            }
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_layer_creation() {
        #[derive(Parser, Clone)]
        struct TestCmd {
            arg: String,
        }

        let layer = CommandLayer::<TestCmd>::new("test");
        assert!(layer.reply_help);
        assert!(layer.reply_error);
    }

    #[test]
    fn test_command_layer_builder() {
        #[derive(Parser, Clone)]
        struct TestCmd {
            arg: String,
        }

        let layer = CommandLayer::<TestCmd>::new("test")
            .reply_help(false)
            .reply_error(false);
        assert!(!layer.reply_help);
        assert!(!layer.reply_error);
    }
}
