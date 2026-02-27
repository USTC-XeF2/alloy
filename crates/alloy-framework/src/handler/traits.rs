//! Handler system for the Alloy framework.
//!
//! This module defines the [`Handler`] trait that forms the foundation of event
//! handling in Alloy. Unlike the previous macro-based approach, handlers are now
//! implemented via blanket implementations for functions with different arities,
//! similar to Axum's handler system.
//!
//! # Example
//!
//! ```rust,ignore
//! use alloy_core::{AlloyContext, FromContext, EventContext};
//!
//! // Simple handler with no parameters
//! async fn simple_handler() {
//!     println!("Handling event");
//! }
//!
//! // Handler with extractor
//! async fn echo_handler(event: EventContext<MessageEvent>) {
//!     println!("Message: {}", event.get_plain_text());
//! }
//!
//! // Handler with multiple extractors
//! async fn complex_handler(
//!     msg: EventContext<MessageEvent>,
//!     state: State<AppState>,
//! ) {
//!     // ...
//! }
//! ```

use std::sync::Arc;

use async_trait::async_trait;
use futures::future::BoxFuture;
use tracing::error;

use crate::context::AlloyContext;
use crate::extractor::FromContext;
use alloy_core::{Message, MessageSegment};

// ============================================================================
// HandleResponse - Handle handler return values
// ============================================================================

/// A trait for types that can handle handler return values.
#[async_trait]
pub trait HandleResponse: Send {
    /// Convert this value into a response.
    async fn into_response(self, ctx: Arc<AlloyContext>);
}

/// Implementation for `()` - no response needed.
#[async_trait]
impl HandleResponse for () {
    async fn into_response(self, _ctx: Arc<AlloyContext>) {
        // No action needed
    }
}

/// Implementation for `String` - send message on Ok, log errors on Err.
#[async_trait]
impl HandleResponse for String {
    async fn into_response(self, ctx: Arc<AlloyContext>) {
        let bot = ctx.bot_arc();
        let event = ctx.event();
        if let Err(e) = bot.send(&**event, &self).await {
            error!("Failed to send message: {e}");
        }
    }
}

/// Implementation for `Message<S>` - sends the message using `send_message`.
#[async_trait]
impl<S: MessageSegment> HandleResponse for Message<S> {
    async fn into_response(self, ctx: Arc<AlloyContext>) {
        let bot = ctx.bot_arc();
        let event = ctx.event();
        if let Err(e) = bot.send_message(&**event, &self).await {
            error!("Failed to send message: {e}");
        }
    }
}

/// Implementation for `Option<T>` where T implements HandleResponse.
///
/// On Some, the inner value's response is handled. On None, no action is taken.
#[async_trait]
impl<T: HandleResponse> HandleResponse for Option<T> {
    async fn into_response(self, ctx: Arc<AlloyContext>) {
        if let Some(t) = self {
            t.into_response(ctx).await;
        }
    }
}

/// Implementation for `Result<T, E>` where T implements HandleResponse.
///
/// On Ok, the inner value's response is handled. On Err, the error is logged.
#[async_trait]
impl<T: HandleResponse, E: std::fmt::Display + Send> HandleResponse for Result<T, E> {
    async fn into_response(self, ctx: Arc<AlloyContext>) {
        match self {
            Ok(t) => t.into_response(ctx).await,
            Err(e) => {
                error!("Handler error: {e}");
            }
        }
    }
}

// ============================================================================
// Handler Trait
// ============================================================================

/// The core trait for event handlers in the Alloy framework.
///
/// Handlers process events. Unlike the previous design, handlers no longer
/// control event propagation - that's managed by [`Matcher`](super::matcher::Matcher).
///
/// # Blanket Implementation
///
/// This trait is automatically implemented for async functions that:
/// - Take 0-16 parameters that implement [`FromContext`]
/// - Return a type that implements [`HandleResponse`]
///
/// # Example
///
/// ```rust,ignore
/// // No return value
/// async fn simple_handler(event: EventContext<MessageEvent>) {
///     println!("Message: {}", event.get_plain_text());
/// }
///
/// // Return Result<(), Error>
/// async fn result_handler(event: EventContext<MessageEvent>) -> Result<(), anyhow::Error> {
///     // Process event...
///     Ok(())
/// }
///
/// // Return Result<String, Error> - automatically sends the message
/// async fn reply_handler(event: EventContext<MessageEvent>) -> Result<String, anyhow::Error> {
///     Ok(format!("You said: {}", event.get_plain_text()))
/// }
/// ```
#[async_trait]
pub trait Handler<T>: Clone + Send + Sync + 'static {
    /// Call the handler with the given context.
    async fn call(self, ctx: Arc<AlloyContext>);
}

// ============================================================================
// BoxedHandler - Type-erased handler stored in collections
// ============================================================================

/// A type-erased handler that can be stored in collections.
///
/// Internally a closure that captures the original handler and calls it
/// with a cloned copy on each invocation.
pub type BoxedHandler = Arc<dyn Fn(Arc<AlloyContext>) -> BoxFuture<'static, ()> + Send + Sync>;

/// Convert a handler function into a boxed handler.
pub fn into_handler<F, T>(f: F) -> BoxedHandler
where
    F: Handler<T> + Send + Sync + 'static,
    T: 'static,
{
    Arc::new(move |ctx| f.clone().call(ctx))
}

// ============================================================================
// Handler implementations for functions (Axum-style)
// ============================================================================

/// Macro to generate Handler implementations for functions with different arities.
macro_rules! impl_handler {
    (
        $($ty:ident),*
    ) => {
        #[allow(non_snake_case)]
        #[async_trait]
        impl<F, Fut, Res, $($ty,)*> Handler<($($ty,)*)> for F
        where
            F: FnOnce($($ty,)*) -> Fut + Clone + Send + Sync + 'static,
            Fut: Future<Output = Res> + Send + 'static,
            Res: HandleResponse + 'static,
            $( $ty: FromContext + Send + 'static, )*
        {
            async fn call(self, ctx: Arc<AlloyContext>) {
                $(
                    let Ok($ty) = $ty::from_context(&ctx) else { return };
                )*

                let res = (self)($($ty,)*).await;
                res.into_response(ctx).await;
            }
        }
    };
}

// Generate implementations for 0-16 parameters
impl_handler!();
impl_handler!(T1);
impl_handler!(T1, T2);
impl_handler!(T1, T2, T3);
impl_handler!(T1, T2, T3, T4);
impl_handler!(T1, T2, T3, T4, T5);
impl_handler!(T1, T2, T3, T4, T5, T6);
impl_handler!(T1, T2, T3, T4, T5, T6, T7);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8, T9);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14);
impl_handler!(
    T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15
);
impl_handler!(
    T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16
);
