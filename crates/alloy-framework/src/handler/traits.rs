//! Function handler trait system for the Alloy framework.
//!
//! This module defines the [`FromCtxFn`] trait for functions that extract
//! parameters from the application context. Handlers are implemented via blanket
//! implementations for async functions with different arities, providing a flexible
//! and ergonomic API similar to Axum's handler system.
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

use crate::context::AlloyContext;
use crate::error::ExtractResult;
use crate::extractor::FromContext;

// ============================================================================
// FromCtxFn Trait
// ============================================================================

/// A generic trait for async functions that extract parameters from the application context.
///
/// This trait enables handlers to be implemented as ordinary async functions with
/// 0-16 parameters, where each parameter implements [`FromContext`] for automatic extraction.
/// This design is inspired by Axum's handler system and provides a clean, elegant API
/// for defining event handlers.
///
/// # Type Parameters
///
/// - `R`: The return type of the function (typically implements [`HandlerResponse`])
/// - `T`: A tuple type parameter representing the function's parameters (used for specialization)
///
/// # Blanket Implementation
///
/// This trait is automatically implemented for async functions that:
/// - Take 0-16 parameters, each implementing [`FromContext`]
/// - Return a type implementing [`HandlerResponse`]
/// - Are `Clone + Send + Sync + 'static`
///
/// # Example
///
/// ```rust,ignore
/// use alloy_core::{AlloyContext, FromContext, EventContext};
///
/// // Simple handler with no context extraction
/// async fn simple_handler() {
///     println!("Handling event");
/// }
///
/// // Handler with message event extraction
/// async fn echo_handler(event: EventContext<MessageEvent>) {
///     println!("Message: {}", event.get_plain_text());
/// }
///
/// // Handler with multiple extractors
/// async fn complex_handler(
///     msg: EventContext<MessageEvent>,
///     state: State<AppState>,
/// ) {
///     // Process message and state...
/// }
/// ```
#[async_trait]
pub trait FromCtxFn<R, T>: Clone + Send + Sync + 'static {
    /// Call this function with the given context, extracting all parameters.
    ///
    /// Returns an error if any parameter extraction fails (e.g., required context is missing).
    async fn call(self, ctx: Arc<AlloyContext>) -> ExtractResult<R>;
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
        #[allow(unused_variables)]
        #[async_trait]
        impl<F, Fut, Res, $($ty,)*> FromCtxFn<Res, ($($ty,)*)> for F
        where
            F: FnOnce($($ty,)*) -> Fut + Clone + Send + Sync + 'static,
            Fut: Future<Output = Res> + Send + 'static,
            Res: Send + 'static,
            $( $ty: FromContext + Send + 'static, )*
        {
            async fn call(self, ctx: Arc<AlloyContext>) -> ExtractResult<Res> {
                let ($($ty,)*) = futures::try_join!($($ty::from_context(&ctx),)*)?;

                Ok((self)($($ty,)*).await)
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
