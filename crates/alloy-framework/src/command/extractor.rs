use clap::Parser;
use derive_more::Deref;

use crate::context::AlloyContext;
use crate::error::{ExtractError, ExtractResult};
use crate::extractor::FromContext;

/// A wrapper type for extracting parsed clap commands from context.
///
/// This extractor retrieves the command that was parsed during the matcher's
/// check phase. It requires that `on_command::<T>()` was used as the
/// matcher, which parses the command and stores it in the context.
///
/// # Example
///
/// ```rust,ignore
/// use alloy_framework::CommandArgs;
///
/// async fn echo_handler(cmd: CommandArgs<BotCommand>) {
///     println!("Got command: {:?}", cmd.0);
/// }
/// ```
#[derive(Debug, Clone, Deref)]
pub struct CommandArgs<T: Parser>(pub T);

impl<T: Parser> CommandArgs<T> {
    /// Unwraps the command value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T: Parser + Clone + Send + 'static> FromContext for CommandArgs<T> {
    async fn from_context(ctx: &AlloyContext) -> ExtractResult<Self> {
        ctx.state()
            .get::<Self>()
            .ok_or_else(|| ExtractError::StateNotFound(std::any::type_name::<Self>()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_args_deref() {
        #[derive(Parser, Clone, Debug, PartialEq)]
        struct TestCmd {
            arg: String,
        }

        let cmd = CommandArgs(TestCmd {
            arg: "test".to_string(),
        });
        assert_eq!(cmd.arg, "test");
    }

    #[test]
    fn test_command_args_into_inner() {
        #[derive(Parser, Clone)]
        struct TestCmd {
            arg: String,
        }

        let cmd = CommandArgs(TestCmd {
            arg: "value".to_string(),
        });
        let inner = cmd.into_inner();
        assert_eq!(inner.arg, "value");
    }
}
