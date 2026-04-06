//! Core handler service for the Alloy framework.
//!
//! [`HandlerService<F, R, T>`] is the fundamental building block: it wraps a
//! single handler and implements `tower::Service<HandlerContext>`. All
//! filtering and other cross-cutting concerns are expressed as ordinary tower
//! [`Layer`]s stacked *on top*.

use std::marker::PhantomData;
use std::task::{Context, Poll};

use futures::FutureExt;
use futures::future::BoxFuture;
use tower::{BoxError, Service};
use tracing::error;

use super::traits::FromCtxFn;
use crate::context::HandlerContext;
use alloy_core::{Message, MessageSegment, Sendable};

// ============================================================================
// HandlerResponse
// ============================================================================

/// A trait for types that can be returned from handlers.
pub trait HandlerResponse: Send + 'static {
    /// Process the handler response, performing any necessary side effects (e.g. sending messages).
    fn process_response(self, ctx: &HandlerContext) -> impl Future<Output = ()> + Send;
}

/// Implementation for `()` - no response needed.
impl HandlerResponse for () {
    async fn process_response(self, _ctx: &HandlerContext) {
        // No action needed
    }
}

/// Helper function to send a message using the bot from the context.
async fn send_message(ctx: &HandlerContext, message: &dyn Sendable) {
    let bot = ctx.bot();
    let event = ctx.event();
    if let Some(scene) = event.scene() {
        if let Err(e) = bot.send(&scene, message).await {
            error!("Failed to send message: {e}");
        }
    } else {
        error!("Event has no scene, cannot send message");
    }
}

/// Implementation for `String` - send message on Ok, log errors on Err.
impl HandlerResponse for String {
    async fn process_response(self, ctx: &HandlerContext) {
        send_message(ctx, &self).await;
    }
}

/// Implementation for `Message<S>` - sends the message using `send_message`.
impl<S: MessageSegment> HandlerResponse for Message<S> {
    async fn process_response(self, ctx: &HandlerContext) {
        send_message(ctx, &self).await;
    }
}

/// Implementation for `Option<T>` where T implements HandlerResponse.
///
/// On Some, the inner value's response is handled. On None, no action is taken.
impl<T: HandlerResponse> HandlerResponse for Option<T> {
    async fn process_response(self, ctx: &HandlerContext) {
        if let Some(t) = self {
            t.process_response(ctx).await;
        }
    }
}

/// Implementation for `Result<T, E>` where T implements HandlerResponse.
///
/// On Ok, the inner value's response is handled. On Err, the error is logged.
impl<T: HandlerResponse, E: std::fmt::Display + Send + 'static> HandlerResponse for Result<T, E> {
    async fn process_response(self, ctx: &HandlerContext) {
        match self {
            Ok(t) => t.process_response(ctx).await,
            Err(e) => {
                error!("Handler error: {e}");
            }
        }
    }
}

// ============================================================================
// HandlerService
// ============================================================================

/// A tower [`Service`] that calls a single generic handler.
///
/// Holds the handler directly with no heap allocation. Implement cloning via
/// `H: Clone` (guaranteed by the [`Handler`] bound).
///
/// # Example
///
/// ```rust,ignore
/// let svc = HandlerService::new(my_handler);
/// // Apply a filter layer on top:
/// let filtered = on_message().layer(svc);
/// ```
pub struct HandlerService<F, T> {
    handler: F,
    _marker: PhantomData<T>,
}

impl<F, T> HandlerService<F, T> {
    pub fn new(handler: F) -> Self {
        Self {
            handler,
            _marker: PhantomData,
        }
    }
}

impl<F: Clone, T> Clone for HandlerService<F, T> {
    fn clone(&self) -> Self {
        HandlerService {
            handler: self.handler.clone(),
            _marker: PhantomData,
        }
    }
}

/// Allows `HandlerService::new(f)` to be omitted in favour of `f.into()` when
/// the target type can be inferred from context.
impl<F, T> From<F> for HandlerService<F, T> {
    fn from(handler: F) -> Self {
        HandlerService::new(handler)
    }
}

impl<F, T> Service<HandlerContext> for HandlerService<F, T>
where
    F: FromCtxFn<T>,
    F::Response: HandlerResponse,
{
    type Response = ();
    type Error = BoxError;
    type Future = BoxFuture<'static, Result<(), Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, ctx: HandlerContext) -> Self::Future {
        let handler = self.handler.clone();
        async move {
            if let Ok(r) = handler.call(&ctx).await {
                r.process_response(&ctx).await;
            }
            Ok(())
        }
        .boxed()
    }
}
