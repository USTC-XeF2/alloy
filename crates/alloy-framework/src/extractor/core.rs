use crate::context::HandlerContext;
use crate::error::ExtractResult;

/// A trait for types that can be extracted from an [`HandlerContext`].
///
/// This is the core abstraction that enables the Alloy framework's parameter
/// injection system. Types implementing this trait can be used directly as
/// handler function parameters.
///
/// # Error Handling
///
/// The extraction can fail (returning `Err`) if the required data is not
/// available in the context. In this case, the handler will be skipped.
pub trait FromContext: Sized + Send {
    /// Attempts to extract this type from the given context.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The context to extract from.
    ///
    /// # Returns
    ///
    /// `Ok(Self)` if extraction succeeds, `Err(ExtractError)` otherwise.
    fn from_context(ctx: &HandlerContext) -> impl Future<Output = ExtractResult<Self>> + Send;
}

/// Implementation for `Option<T>` where `T: FromContext`.
///
/// This allows handlers to have optional parameters that may or may not
/// be extractable from the context.
impl<T: FromContext> FromContext for Option<T> {
    async fn from_context(ctx: &HandlerContext) -> ExtractResult<Self> {
        Ok(T::from_context(ctx).await.ok())
    }
}

/// Implementation for `ExtractResult<T>` where `T: FromContext`.
///
/// This allows handlers to have parameters that can return detailed
/// extraction errors.
impl<T: FromContext> FromContext for ExtractResult<T> {
    async fn from_context(ctx: &HandlerContext) -> ExtractResult<Self> {
        Ok(T::from_context(ctx).await)
    }
}

/// Implementation for `HandlerContext` itself, allowing it to be directly injected
/// into handler functions.
///
/// This is a fundamental implementation that allows handlers to access the full
/// context when needed.
impl FromContext for HandlerContext {
    async fn from_context(ctx: &HandlerContext) -> ExtractResult<Self> {
        Ok(ctx.clone())
    }
}
