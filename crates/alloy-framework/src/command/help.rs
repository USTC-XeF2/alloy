use std::collections::HashMap;
use std::fmt::Write;
use std::marker::PhantomData;

use clap::{Command, Parser};
use futures::future::BoxFuture;
use futures::{FutureExt, future};

use super::extractor::CommandArgs;
use super::layer::{CommandService, on_command};
use crate::context::HandlerContext;
use crate::error::ExtractResult;
use crate::handler::{FromCtxFn, HandlerService};

pub(crate) trait HelpProvider: Send + Sync {
    fn call<'a>(&'a self, ctx: &'a HandlerContext) -> BoxFuture<'a, ExtractResult<CommandMap>>;
}

impl<F, T> HelpProvider for (F, PhantomData<T>)
where
    F: FromCtxFn<T, Response = CommandMap>,
    T: Send + Sync,
{
    fn call<'a>(&'a self, ctx: &'a HandlerContext) -> BoxFuture<'a, ExtractResult<CommandMap>> {
        self.0.clone().call(ctx).boxed()
    }
}

/// Displays help information for available commands.
#[derive(Parser, Debug, Clone)]
pub struct HelpCommand {
    command: Option<String>,

    /// Use short format.
    #[arg(short, long)]
    short: bool,
}

/// A map of command names to their corresponding clap `Command` definitions.
#[derive(Debug, Clone, Default)]
pub struct CommandMap(HashMap<String, Command>);

impl CommandMap {
    /// Creates a new empty command map.
    pub fn new() -> Self {
        CommandMap(HashMap::new())
    }

    /// Inserts a command to the map with the given name and clap definition derived from `T`.
    pub fn insert<T: Parser>(mut self, name: &'static str) -> Self {
        self.0.insert(name.into(), T::command().name(name));
        self
    }
}

impl FromIterator<(String, Command)> for CommandMap {
    fn from_iter<I: IntoIterator<Item = (String, Command)>>(iter: I) -> Self {
        CommandMap(iter.into_iter().collect())
    }
}

impl Extend<(String, Command)> for CommandMap {
    fn extend<I: IntoIterator<Item = (String, Command)>>(&mut self, iter: I) {
        self.0.extend(iter);
    }
}

#[derive(Debug, Clone)]
pub struct HelpCommandHandler;

impl FromCtxFn<()> for HelpCommandHandler {
    type Response = String;

    async fn call(self, ctx: &HandlerContext) -> ExtractResult<Self::Response> {
        let providers = ctx
            .command()
            .help_provider
            .lock()
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let results = future::join_all(providers.iter().map(|provider| provider.call(ctx))).await;

        let mut available = CommandMap::new();
        for cmds in results.into_iter().flatten() {
            available.0.extend(cmds.0);
        }
        let mut available = available.insert::<HelpCommand>("help").0;

        let cmd: CommandArgs<HelpCommand> = ctx.state().get().unwrap();

        let result = if let Some(name) = &cmd.command {
            if let Some(command) = available.get_mut(name) {
                if cmd.short {
                    command.render_help().to_string()
                } else {
                    command.render_long_help().to_string()
                }
            } else {
                format!("Command not found: {name}. Use /help to list available commands.")
            }
        } else {
            let mut list = String::from("Available Commands:\n");
            let mut entries: Vec<_> = available.into_iter().collect();
            entries.sort_by(|(a, _), (b, _)| a.cmp(b));
            for (name, cmd) in entries {
                writeln!(list, "• {}: {}", name, cmd.get_about().unwrap_or_default()).ok();
            }
            list.push_str("\nUse /help <command> for detailed usage of a specific command.");
            list
        };

        Ok(result)
    }
}

/// Creates a help command service that aggregates all runtime-registered plugin commands.
#[inline]
pub fn help_command() -> CommandService<HelpCommand, HandlerService<HelpCommandHandler, ()>> {
    on_command::<HelpCommand>("help").handler(HelpCommandHandler)
}
