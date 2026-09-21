//! Native CLI entry point for the MESH toolchain.

use clap::{Parser, Subcommand};
use std::fs;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "mesh")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Parse and validate an MPRX file, printing any diagnostics.
    Check { file: String },
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

    if result.diagnostics.is_empty() {
        println!("no errors");
        return ExitCode::SUCCESS;
    }

    for diagnostic in &result.diagnostics {
        eprintln!(
            "error: {} ({}..{})",
            diagnostic.message, diagnostic.span.start_byte, diagnostic.span.end_byte
        );
    }
    ExitCode::FAILURE
}
