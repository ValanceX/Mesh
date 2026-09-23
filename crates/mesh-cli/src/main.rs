//! Native CLI entry point for the MESH toolchain.

use clap::{Parser, Subcommand};
use mesh_syntax::Severity;
use std::fs;
use std::process::ExitCode;

/// The MESH command-line toolchain: parse, check, and (eventually) compile
/// MPRX source files.
#[derive(Parser)]
#[command(name = "mesh")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Parse and validate an MPRX file, printing any diagnostics.
    Check {
        /// Path to the `.mprx` file to check.
        file: String,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Check { file } => run_check(&file),
    }
}

fn run_check(file: &str) -> ExitCode {
    let source = match fs::read_to_string(file) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("error: could not read {file}: {err}");
            return ExitCode::FAILURE;
        }
    };

    let result = mesh_compiler::compile(&source);

    // Diagnostics print in `CompileResult` order (never re-sorted). The
    // renderer returns a block with no trailing newline; this loop owns
    // the separation: each block is followed by exactly one blank line.
    for diagnostic in &result.diagnostics {
        eprintln!(
            "{}\n",
            mesh_compiler::render_diagnostic(&source, file, diagnostic)
        );
    }

    // Only errors fail the check — warnings are reported but non-fatal.
    let has_errors = result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error);
    if has_errors {
        return ExitCode::FAILURE;
    }

    println!("no errors");
    ExitCode::SUCCESS
}
