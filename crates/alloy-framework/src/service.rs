//! Core handler service for the Alloy framework.
//!
//! [`HandlerService<H, T>`] is the fundamental building block: it wraps a
//! single handler and implements `tower::Service<Arc<AlloyContext>>`. All
//! filtering and other cross-cutting concerns are expressed as ordinary tower
//! [`Layer`]s stacked *on top*.
//!
//! # Design
//!
//! The `on_xxx()` helpers return a [`tower::ServiceBuilder`] with the relevant
//! layer pre-stacked. Call `.handler(f)` (from [`ServiceBuilderHandlerExt`]) to
//! attach a handler and obtain the final service:
//!
//! ```text
//! on_message()           ← ServiceBuilder<Stack<FilterLayer, Identity>>
//!     .handler(my_fn)    ← applies FilterLayer to HandlerService<F, T>
//! ```
//!
//! The result is type-erased into [`BoxedHandlerService`] by
//! `runtime.register_service(svc)`.
//!
//! # Example
//!
//! ```rust,ignore
//! use alloy::prelude::*;
//!
//! runtime.register_service(on_message().handler(my_handler)).await;
//! ```

use std::marker::PhantomData;
use std::sync::Arc;
use std::task::{Context, Poll};

use tower::filter::{FilterLayer, Predicate};
use tower::util::BoxCloneSyncService;
use tower::{BoxError, Layer, Service, ServiceBuilder};
use tower_layer::Stack;

use crate::context::AlloyContext;
use crate::error::EventSkipped;
use crate::handler::{BoxFuture, Handler};

/// A wrapper that blocks event propagation if the inner service succeeds.
///
/// If the inner service returns `Ok`, this calls `ctx.stop_propagation()` to prevent
/// further handlers from processing the event. The handler itself completes normally.
/// If the inner service returns an error, it is passed through unchanged.
#[derive(Clone)]
pub struct BlockLayer;

impl<S> Layer<S> for BlockLayer {
    type Service = BlockService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        BlockService { inner }
    }
}

pub struct BlockService<S> {
    inner: S,
}

impl<S> Clone for BlockService<S>
where
    S: Clone,
{
    fn clone(&self) -> Self {
        BlockService {
            inner: self.inner.clone(),
        }
    }
}

impl<S> Service<Arc<AlloyContext>> for BlockService<S>
where
    S: Service<Arc<AlloyContext>, Response = (), Error = BoxError> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = ();
    type Error = BoxError;
    type Future = BoxFuture<Result<(), Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, ctx: Arc<AlloyContext>) -> Self::Future {
        let mut inner = self.inner.clone();
        Box::pin(async move {
            match inner.call(ctx.clone()).await {
                Ok(()) => {
                    ctx.stop_propagation();
                    Ok(())
                }
                Err(e) => Err(e),
            }
        })
    }
}

// ============================================================================
// EventPredicate
// ============================================================================

/// A type-erased, [`Predicate`]-implementing wrapper for synchronous closures.
///
/// When the inner predicate returns `false` the request is rejected with [`EventSkipped`].
#[derive(Clone)]
pub struct EventPredicate(Arc<dyn Fn(&AlloyContext) -> bool + Send + Sync>);

impl EventPredicate {
    /// Creates a new `EventPredicate` from a synchronous closure.
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&AlloyContext) -> bool + Send + Sync + 'static,
    {
        Self(Arc::new(f))
    }
}

impl Predicate<Arc<AlloyContext>> for EventPredicate {
    type Request = Arc<AlloyContext>;

    fn check(&mut self, request: Arc<AlloyContext>) -> Result<Arc<AlloyContext>, BoxError> {
        if (self.0)(&request) {
            Ok(request)
        } else {
            Err(Box::new(EventSkipped))
        }
    }
}

// ============================================================================
// Type alias for the boxed handler service stored by the runtime
// ============================================================================

/// A type-erased, `Clone + Send + Sync` tower service that processes
/// `Arc<AlloyContext>`.
///
/// Created by `runtime.register_service(svc)` via
/// `BoxCloneSyncService::new(svc)`.
///
/// The error type is [`BoxError`]: filter layers return [`crate::error::EventSkipped`]
/// on mismatch, which the runtime silently ignores.
pub type BoxedHandlerService = BoxCloneSyncService<Arc<AlloyContext>, (), BoxError>;

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
pub struct HandlerService<H, T> {
    handler: H,
    // PhantomData<fn() -> T> is Send + Sync regardless of T's variance,
    // and avoids implying ownership of T.
    _marker: PhantomData<fn() -> T>,
}

impl<H: Clone, T> Clone for HandlerService<H, T> {
    fn clone(&self) -> Self {
        HandlerService {
            handler: self.handler.clone(),
            _marker: PhantomData,
        }
    }
}

impl<H, T> HandlerService<H, T>
where
    H: Handler<T>,
{
    /// Wraps `handler` in a `HandlerService`.
    pub fn new(handler: H) -> Self {
        Self {
            handler,
            _marker: PhantomData,
        }
    }
}

/// Allows `HandlerService::new(f)` to be omitted in favour of `f.into()` when
/// the target type can be inferred from context.
impl<H, T> From<H> for HandlerService<H, T>
where
    H: Handler<T>,
{
    fn from(handler: H) -> Self {
        HandlerService::new(handler)
    }
}

impl<H, T> Service<Arc<AlloyContext>> for HandlerService<H, T>
where
    H: Handler<T> + Clone + Send + Sync + 'static,
    T: 'static,
{
    type Response = ();
    type Error = BoxError;
    type Future = BoxFuture<Result<(), Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, ctx: Arc<AlloyContext>) -> Self::Future {
        let handler = self.handler.clone();
        Box::pin(async move {
            handler.call(ctx).await;
            Ok(())
        })
    }
}

// ============================================================================
// ServiceBuilderExt
// ============================================================================

/// Extension trait for [`tower::ServiceBuilder`] that adds convenience methods
/// for building handler services with filtering and other cross-cutting concerns.
///
/// This trait is automatically available via `use alloy::prelude::*`.
pub trait ServiceBuilderExt<L> {
    /// Wrap `handler` in a [`HandlerService`] and apply all stacked layers,
    /// returning the final composed service.
    ///
    /// Equivalent to `.service(HandlerService::new(handler))`.
    fn handler<H, T>(self, handler: H) -> L::Service
    where
        H: Handler<T>,
        L: Layer<HandlerService<H, T>>;

    /// Attaches a synchronous filter predicate directly to the service builder.
    ///
    /// Equivalent to `.filter(EventPredicate::new(predicate))` but more concise.
    fn rule<F>(self, predicate: F) -> ServiceBuilder<Stack<FilterLayer<EventPredicate>, L>>
    where
        F: Fn(&AlloyContext) -> bool + Send + Sync + 'static;

    /// Adds a blocking layer that prevents event propagation if the inner service succeeds.
    ///
    /// When the inner service completes with `Ok`, this calls `ctx.stop_propagation()` to stop
    /// further handlers from processing the event. The current handler executes normally and
    /// completes successfully. If the inner service returns an error, it passes through unchanged.
    fn block(self) -> ServiceBuilder<Stack<BlockLayer, L>>;
}

impl<L> ServiceBuilderExt<L> for ServiceBuilder<L> {
    fn handler<H, T>(self, handler: H) -> L::Service
    where
        H: Handler<T>,
        L: Layer<HandlerService<H, T>>,
    {
        self.service(HandlerService::new(handler))
    }

    fn rule<F>(self, predicate: F) -> ServiceBuilder<Stack<FilterLayer<EventPredicate>, L>>
    where
        F: Fn(&AlloyContext) -> bool + Send + Sync + 'static,
    {
        self.filter(EventPredicate::new(predicate))
    }

    fn block(self) -> ServiceBuilder<Stack<BlockLayer, L>> {
        self.layer(BlockLayer)
    }
}
