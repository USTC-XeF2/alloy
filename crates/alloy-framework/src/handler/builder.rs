//! Extension trait for tower service builder.
//!
//! Provides convenience methods for building handler services with filtering
//! and other cross-cutting concerns.

use std::sync::Arc;
use std::task::{Context, Poll};

use futures::FutureExt;
use futures::future::BoxFuture;
use tower::filter::{AsyncFilterLayer, AsyncPredicate, FilterLayer, Predicate};
use tower::{BoxError, Layer, Service, ServiceBuilder};
use tower_layer::Stack;

use super::service::{HandlerResponse, HandlerService};
use super::traits::FromCtxFn;
use crate::context::AlloyContext;
use crate::error::EventSkipped;

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
        BlockService(inner)
    }
}

pub struct BlockService<S>(S);

impl<S> Clone for BlockService<S>
where
    S: Clone,
{
    fn clone(&self) -> Self {
        BlockService(self.0.clone())
    }
}

impl<S> Service<Arc<AlloyContext>> for BlockService<S>
where
    S: Service<Arc<AlloyContext>, Response = (), Error = BoxError> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = ();
    type Error = BoxError;
    type Future = BoxFuture<'static, Result<(), Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready(cx)
    }

    fn call(&mut self, ctx: Arc<AlloyContext>) -> Self::Future {
        let mut inner = self.0.clone();
        async move {
            match inner.call(ctx.clone()).await {
                Ok(()) => {
                    ctx.stop_propagation();
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
        .boxed()
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

/// A type-erased, [`AsyncPredicate`]-implementing wrapper for asynchronous closures.
#[derive(Clone)]
pub struct AsyncEventPredicate(
    Arc<dyn Fn(Arc<AlloyContext>) -> BoxFuture<'static, bool> + Send + Sync>,
);

impl AsyncEventPredicate {
    /// Creates a new `AsyncEventPredicate` from an asynchronous closure.
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(Arc<AlloyContext>) -> BoxFuture<'static, bool> + Send + Sync + 'static,
    {
        Self(Arc::new(f))
    }
}

impl AsyncPredicate<Arc<AlloyContext>> for AsyncEventPredicate {
    type Future = BoxFuture<'static, Result<Arc<AlloyContext>, BoxError>>;
    type Request = Arc<AlloyContext>;

    fn check(&mut self, request: Arc<AlloyContext>) -> Self::Future {
        let f = self.0.clone();
        async move {
            if f(request.clone()).await {
                Ok(request)
            } else {
                Err(EventSkipped.into())
            }
        }
        .boxed()
    }
}

/// Extension trait for [`tower::ServiceBuilder`] that adds convenience methods
/// for building handler services with filtering and other cross-cutting concerns.
///
/// This trait is automatically available via `use alloy::prelude::*`.
pub trait ServiceBuilderExt<L> {
    /// Wrap `handler` in a [`HandlerService`] and apply all stacked layers,
    /// returning the final composed service.
    ///
    /// Equivalent to `.service(HandlerService::new(handler))`.
    fn handler<F, R, T>(self, handler: F) -> L::Service
    where
        F: FromCtxFn<R, T>,
        R: HandlerResponse,
        L: Layer<HandlerService<F, R, T>>;

    /// Attaches a synchronous filter predicate directly to the service builder.
    ///
    /// Equivalent to `.filter(EventPredicate::new(predicate))` but more concise.
    fn rule_sync<F>(self, predicate: F) -> ServiceBuilder<Stack<FilterLayer<EventPredicate>, L>>
    where
        F: Fn(&AlloyContext) -> bool + Send + Sync + 'static;

    /// Attaches an asynchronous filter predicate directly to the service builder.
    fn rule<F, T>(
        self,
        predicate: F,
    ) -> ServiceBuilder<Stack<AsyncFilterLayer<AsyncEventPredicate>, L>>
    where
        F: FromCtxFn<bool, T>;

    /// Adds a blocking layer that prevents event propagation if the inner service succeeds.
    ///
    /// When the inner service completes with `Ok`, this calls `ctx.stop_propagation()` to stop
    /// further handlers from processing the event. The current handler executes normally and
    /// completes successfully. If the inner service returns an error, it passes through unchanged.
    fn block(self) -> ServiceBuilder<Stack<BlockLayer, L>>;
}

impl<L> ServiceBuilderExt<L> for ServiceBuilder<L> {
    fn handler<F, R, T>(self, handler: F) -> L::Service
    where
        F: FromCtxFn<R, T>,
        R: HandlerResponse,
        L: Layer<HandlerService<F, R, T>>,
    {
        self.service(HandlerService::new(handler))
    }

    fn rule_sync<F>(self, predicate: F) -> ServiceBuilder<Stack<FilterLayer<EventPredicate>, L>>
    where
        F: Fn(&AlloyContext) -> bool + Send + Sync + 'static,
    {
        self.filter(EventPredicate::new(predicate))
    }

    fn rule<F, T>(
        self,
        predicate: F,
    ) -> ServiceBuilder<Stack<AsyncFilterLayer<AsyncEventPredicate>, L>>
    where
        F: FromCtxFn<bool, T>,
    {
        self.filter_async(AsyncEventPredicate::new(move |ctx| {
            predicate
                .clone()
                .call(ctx)
                .map(|f| f.unwrap_or(false))
                .boxed()
        }))
    }

    fn block(self) -> ServiceBuilder<Stack<BlockLayer, L>> {
        self.layer(BlockLayer)
    }
}
