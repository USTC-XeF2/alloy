//! Command parsing system using clap.
//!
//! This module provides a clap-based command parsing system that integrates
//! with the Alloy matcher system. It supports:
//!
//! - Automatic command parsing from message rich text
//! - Type-safe command extraction via `Command<T>` extractor
//! - Help message generation on parse errors
//! - Rich text segments: [`ImageSegment`] and [`AtSegment`] for accessing non-text
//!   segments that appear as command arguments
//!
//! # Rich Text Parsing
//!
//! When a message contains images or mentions mixed with text, the parser
//! replaces them with unique placeholder tokens before shell-splitting.
//! After clap parsing, handlers can use `ImageSegment` and `AtSegment` to
//! retrieve the original rich content:
//!
//! ```rust,ignore
//! use clap::Parser;
//! use alloy_framework::{on_command, CommandArgs, ImageSegment, AtSegment};
//!
//! #[derive(Parser, Clone)]
//! struct SendCommand {
//!     /// The user to send to
//!     target: AtSegment,
//!     /// An image to send
//!     image: ImageSegment,
//! }
//!
//! let matcher = on_command::<SendCommand>("send")
//!     .handler(|cmd: CommandArgs<SendCommand>| async move {
//!         let user_id: &str = &cmd.target; // "12345"
//!         let image_ref: &str = &cmd.image; // "abc.jpg"
//!     });
//! ```

use std::cell::RefCell;

pub mod extractor;
pub mod layer;
pub mod segment;
pub mod split;

pub use extractor::CommandArgs;
pub use layer::{CommandLayer, CommandService, on_command};
pub use segment::{AtSegment, HandleRegistry, ImageSegment};

// Thread-local registry for resolving handles during clap's FromStr parsing.
thread_local! {
    pub(crate) static CURRENT_REGISTRY: RefCell<Option<segment::HandleRegistry>> = const { RefCell::new(None) };
}
