//! The `mesh-lsp` binary: the MESH language server over stdio.
#![deny(clippy::unwrap_used, clippy::expect_used)]

use mesh_lsp::{Options, Outcome};
use std::io::{self, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let (connection, io_threads) = lsp_server::Connection::stdio();
    let outcome = mesh_lsp::run(connection, Options::default());
    match &outcome {
        // After `exit` the transport's reader has stopped, so its threads
        // can be joined; this flushes the last response.
        Outcome::Exited { .. } => {
            let _ = io_threads.join();
        }
        // Otherwise the reader may still be blocked on stdin: exit without
        // waiting for it.
        Outcome::Disconnected => {}
        Outcome::Failed(message) => {
            let _ = writeln!(io::stderr().lock(), "mesh-lsp: {message}");
        }
    }
    ExitCode::from(outcome.exit_code())
}
