//! One module per `arf` subcommand. Each exposes a `pub fn run(...)`
//! that main.rs dispatches to after clap parses arguments.

pub mod browse;
pub mod diff;
pub mod export;
pub mod graph;
pub mod html;
pub mod init;
pub mod log;
pub mod record;
pub mod spec;
pub mod sync;
pub mod why;
