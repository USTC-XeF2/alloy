use std::marker::PhantomData;
use std::task::{Context, Poll};

use amira_core::EventType;
use clap::Parser;
use clap::error::ErrorKind;
use futures::FutureExt;
use futures::future::{self, BoxFuture};
use tower::{BoxError, Layer, Service, ServiceBuilder};
use tower_layer::{Identity, Stack};

use crate::context::HandlerContext;
use crate::error::EventSkipped;
use crate::handler::{FromCtxFn, HandlerResponse, HandlerService, ServiceBuilderExt};

use super::extractor::CommandArgs;
use super::segment::CURRENT_REGISTRY;
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
#[derive(Debug)]
pub struct CommandLayer<T>
where
    T: Parser + Clone + Send + Sync + 'static,
{
    name: String,
    start_tag: Option<String>,
    aliases: Vec<String>,
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
            start_tag: None,
            aliases: Vec::new(),
            reply_help: true,
            reply_error: true,
            block: true,
            _marker: PhantomData,
        }
    }

    /// Override the start tag for this specific command (default: `None`, falls back
    /// to [`CommandConfig::default_start_tag`] from the runtime configuration).
    pub fn start_tag(mut self, tag: impl Into<String>) -> Self {
        self.start_tag = Some(tag.into());
        self
    }

    /// Add an alias for this command.
    pub fn alias(mut self, alias: impl Into<String>) -> Self {
        self.aliases.push(alias.into());
        self
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
    pub fn handler<F, U>(self, handler: F) -> CommandService<T, HandlerService<F, U>>
    where
        F: FromCtxFn<U>,
        F::Response: HandlerResponse,
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
            start_tag: self.start_tag.clone(),
            aliases: self.aliases.clone(),
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
#[derive(Debug, Clone)]
pub struct CommandService<T, S> {
    name: String,
    start_tag: Option<String>,
    aliases: Vec<String>,
    reply_help: bool,
    reply_error: bool,
    block: bool,
    inner: S,
    _marker: PhantomData<T>,
}

fn skip_event() -> BoxFuture<'static, Result<(), BoxError>> {
    future::ready(Err(Box::new(EventSkipped) as BoxError)).boxed()
}

impl<T, S> Service<HandlerContext> for CommandService<T, S>
where
    T: Parser + Clone + Send + Sync + 'static,
    S: Service<HandlerContext, Response = (), Error = BoxError> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = ();
    type Error = BoxError;
    type Future = BoxFuture<'static, Result<(), Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, ctx: HandlerContext) -> Self::Future {
        if ctx.event().event_type() != EventType::Message {
            return skip_event();
        }

        let rich_text = ctx.event().rich_text();
        let Some((args, registry)) = rich_text_shell_split(&rich_text) else {
            return skip_event();
        };

        let start_tag = self
            .start_tag
            .as_ref()
            .unwrap_or_else(|| &ctx.command().config.default_start_tag);

        let Some(cmd_name) = args.first().and_then(|s| s.strip_prefix(start_tag)) else {
            return skip_event();
        };

        if cmd_name != self.name && self.aliases.iter().all(|alias| alias != cmd_name) {
            return skip_event();
        }

        CURRENT_REGISTRY.with(|reg| {
            *reg.borrow_mut() = Some(registry);
        });
        let result = T::try_parse_from(&args);
        CURRENT_REGISTRY.with(|reg| {
            *reg.borrow_mut() = None;
        });

        if self.block {
            ctx.stop_propagation();
        }

        match result {
            Ok(cmd) => {
                ctx.state().set(CommandArgs(cmd));
                self.inner.call(ctx).boxed()
            }
            Err(err) => {
                let should_reply = if err.kind() == ErrorKind::DisplayHelp {
                    self.reply_help
                } else {
                    self.reply_error
                };

                async move {
                    if should_reply {
                        err.to_string().process_response(&ctx).await;
                    }
                    Ok(())
                }
                .boxed()
            }
        }
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
