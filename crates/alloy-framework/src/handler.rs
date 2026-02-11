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
//!     println!("Message: {}", event.plain_text());
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

use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;

use crate::extractor::FromContext;
use alloy_core::foundation::context::AlloyContext;

/// A type alias for a boxed, pinned future that is `Send`.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

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
/// - Return `()` (the return value is ignored)
///
/// # Example
///
/// ```rust,ignore
/// // These are all valid handlers:
/// async fn no_params() {}
/// async fn one_param(event: EventContext<MessageEvent>) {}
/// async fn two_params(msg: EventContext<MessageEvent>, state: State<AppState>) {}
/// ```
pub trait Handler<T>: Clone + Send + Sync + 'static {
    /// The type of future calling this handler returns.
    type Future: Future<Output = ()> + Send + 'static;

    /// Call the handler with the given context.
    fn call(self, ctx: Arc<AlloyContext>) -> Self::Future;
}

// ============================================================================
// IntoHandler - Convert functions into Handler trait objects
// ============================================================================

/// A wrapper that converts a function into a boxed handler.
///
/// This is used internally to store handlers in collections while maintaining
/// type erasure.
pub struct HandlerFn<F, T> {
    f: F,
    _marker: PhantomData<fn() -> T>,
}

impl<F, T> HandlerFn<F, T> {
    /// Creates a new handler function wrapper.
    pub fn new(f: F) -> Self {
        Self {
            f,
            _marker: PhantomData,
        }
    }
}

impl<F: Clone, T> Clone for HandlerFn<F, T> {
    fn clone(&self) -> Self {
        Self {
            f: self.f.clone(),
            _marker: PhantomData,
        }
    }
}

/// A type-erased handler that can be stored in collections.
pub type BoxedHandler = Arc<dyn ErasedHandler + Send + Sync>;

/// Type-erased handler trait for dynamic dispatch.
pub trait ErasedHandler: Send + Sync {
    /// Execute the handler with the given context.
    fn call(&self, ctx: Arc<AlloyContext>) -> BoxFuture<'static, ()>;
}

impl<F, T> ErasedHandler for HandlerFn<F, T>
where
    F: Handler<T> + Send + Sync,
    T: 'static,
{
    fn call(&self, ctx: Arc<AlloyContext>) -> BoxFuture<'static, ()> {
        let f = self.f.clone();
        Box::pin(async move {
            f.call(ctx).await;
        })
    }
}

/// Convert a handler function into a boxed handler.
pub fn into_handler<F, T>(f: F) -> BoxedHandler
where
    F: Handler<T> + Send + Sync + 'static,
    T: 'static,
{
    Arc::new(HandlerFn::new(f))
}

// ============================================================================
// Handler implementations for functions (Axum-style)
// ============================================================================

// Implementation for functions with no parameters
impl<F, Fut> Handler<()> for F
where
    F: FnOnce() -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    type Future = Fut;

    fn call(self, _ctx: Arc<AlloyContext>) -> Self::Future {
        (self)()
    }
}

/// Macro to generate Handler implementations for functions with different arities.
macro_rules! impl_handler {
    (
        $($ty:ident),*
    ) => {
        #[allow(non_snake_case, unused_mut, unused_variables)]
        impl<F, Fut, $($ty,)*> Handler<($($ty,)*)> for F
        where
            F: FnOnce($($ty,)*) -> Fut + Clone + Send + Sync + 'static,
            Fut: Future<Output = ()> + Send + 'static,
            $( $ty: FromContext + Send + 'static, )*
        {
            type Future = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

            fn call(self, ctx: Arc<AlloyContext>) -> Self::Future {
                Box::pin(async move {
                    $(
                        let Ok($ty) = $ty::from_context(&ctx) else { return };
                    )*

                    (self)($($ty,)*).await;
                })
            }
        }
    };
}

// Generate implementations for 1-16 parameters
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

// ============================================================================
// CanExtract - Check if parameters can be extracted
// ============================================================================

/// Trait for checking if a handler's parameters can be extracted from context.
pub trait CanExtract<T> {
    /// Returns true if all parameters can be extracted from the context.
    fn can_extract(ctx: &AlloyContext) -> bool;
}

// Implementation for no parameters
impl<F> CanExtract<()> for F
where
    F: Clone + Send + Sync + 'static,
{
    fn can_extract(_ctx: &AlloyContext) -> bool {
        true
    }
}

/// Macro to generate CanExtract implementations.
macro_rules! impl_can_extract {
    (
        $($ty:ident),*
    ) => {
        #[allow(non_snake_case, unused_variables)]
        impl<F, $($ty,)*> CanExtract<($($ty,)*)> for F
        where
            F: Clone + Send + Sync + 'static,
            $( $ty: FromContext + 'static, )*
        {
            fn can_extract(ctx: &AlloyContext) -> bool {
                $(
                    if $ty::from_context(ctx).is_err() {
                        return false;
                    }
                )*
                true
            }
        }
    };
}

impl_can_extract!(T1);
impl_can_extract!(T1, T2);
impl_can_extract!(T1, T2, T3);
impl_can_extract!(T1, T2, T3, T4);
impl_can_extract!(T1, T2, T3, T4, T5);
impl_can_extract!(T1, T2, T3, T4, T5, T6);
impl_can_extract!(T1, T2, T3, T4, T5, T6, T7);
impl_can_extract!(T1, T2, T3, T4, T5, T6, T7, T8);
impl_can_extract!(T1, T2, T3, T4, T5, T6, T7, T8, T9);
impl_can_extract!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
impl_can_extract!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
impl_can_extract!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);
impl_can_extract!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13);
impl_can_extract!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14);
impl_can_extract!(
    T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15
);
impl_can_extract!(
    T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16
);
