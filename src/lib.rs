//! arf-cli library surface.
//!
//! The CLI binary in src/main.rs is a thin clap-parsing + dispatch
//! wrapper over the modules here. Re-exporting these from a lib also
//! lets integration tests and downstream tools call into the
//! commands without exec'ing the binary.

pub mod commands;
pub mod record;
pub mod store;
