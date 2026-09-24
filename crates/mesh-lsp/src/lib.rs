//! The MESH language server.
//!
//! It serves canonical diagnostics and quick fixes for MPRX documents and
//! their component manifest, and hover, go to definition and completion
//! for MPRX documents, also on files with syntax errors. It is a client of `mesh-compiler`, the single
//! semantic authority: the diagnostics it publishes are exactly the ones
//! `mesh check` reports for the same text, manifest and component, and it
//! has no parser or type system of its own.
//!
//! [`run`] serves one client over any [`lsp_server::Connection`]: the
//! `mesh-lsp` binary passes stdio, and tests pass an in-memory connection.
//! See `docs/manual/mesh-lsp.md` for configuring it.
#![deny(clippy::unwrap_used, clippy::expect_used)]

mod config;
mod convert;
mod server;
mod uri;
mod worker;

#[doc(hidden)]
pub mod testing;

use crossbeam_channel::Receiver;
use std::time::Duration;

/// How long after the latest change to a document it's recompiled.
pub const DEBOUNCE: Duration = Duration::from_millis(150);

/// How [`run`] behaves. The binary uses the default.
#[derive(Debug, Clone)]
pub struct Options {
    /// How long after the latest `didChange` a document is recompiled.
    pub debounce: Duration,
    /// For tests only: the compile thread waits for one message on this
    /// before each compile, so a test can hold a compile back.
    #[doc(hidden)]
    pub compile_gate: Option<Receiver<()>>,
    /// For tests only: enables the `mesh/panicForTest` request.
    #[doc(hidden)]
    pub test_hooks: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            debounce: DEBOUNCE,
            compile_gate: None,
            test_hooks: false,
        }
    }
}

/// How a session ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The client sent `exit`, after `shutdown` or not.
    Exited { after_shutdown: bool },
    /// The transport stopped: stdin closed, stdout closed, or a message
    /// that isn't JSON-RPC.
    Disconnected,
    /// The server couldn't start.
    Failed(String),
}

impl Outcome {
    /// The process exit status LSP prescribes: 0 after `shutdown` then
    /// `exit`, 1 otherwise.
    pub fn exit_code(&self) -> u8 {
        match self {
            Outcome::Exited {
                after_shutdown: true,
            } => 0,
            _ => 1,
        }
    }
}

/// Serves one client over `connection` until it exits or disconnects.
pub fn run(connection: lsp_server::Connection, options: Options) -> Outcome {
    server::run(connection, options)
}
