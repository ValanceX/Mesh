//! Native CLI entry point for the MESH toolchain.

use clap::{Parser, Subcommand, ValueEnum};
use mesh_compiler::check;
use mesh_syntax::Diagnostic;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

/// The MESH command-line toolchain: parse, check, and (eventually) compile
/// MPRX source files.
#[derive(Parser)]
#[command(name = "mesh", version)]
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
        /// A component manifest (JSON) to check the file against.
        #[arg(long, value_name = "FILE")]
        model: Option<String>,
        /// The manifest component whose template the file is. Defaults to
        /// the file's name without its extension.
        #[arg(long, value_name = "NAME", requires = "model")]
        component: Option<String>,
        /// How to print diagnostics.
        #[arg(long, value_enum, default_value_t = Format::Human)]
        format: Format,
    },
}

/// How `mesh check` prints what it found. (The `///` lines on the
/// variants are their `--help` text.)
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Format {
    /// rustc-style blocks on stderr, and `no errors` on stdout if there are none
    Human,
    /// one JSON document on stdout (see `schemas/diagnostics-v1.schema.json`)
    Json,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Check {
            file,
            model,
            component,
            format,
        } => run_check(&file, model.as_deref(), component.as_deref(), format),
    }
}

/// Checks `file` as `mesh_compiler::check` defines a check. This only
/// reads files, derives the component from the file's stem when
/// `--component` isn't given, and prints: the check's two phases decide
/// everything else. The model loads before the file is read, so a broken
/// manifest is reported even when the file can't be read.
fn run_check(file: &str, model: Option<&str>, component: Option<&str>, format: Format) -> ExitCode {
    let model = match model {
        Some(path) => {
            let Some(text) = read(path) else {
                return ExitCode::FAILURE;
            };
            let name = component.map_or_else(|| file_stem(file), str::to_string);
            match check::Model::load(&text, &name) {
                Ok(model) => Some(model),
                Err(diagnostics) => {
                    let printed = print_diagnostics(format, &text, path, &diagnostics);
                    return exit(printed, ExitCode::FAILURE);
                }
            }
        }
        None => None,
    };

    let Some(source) = read(file) else {
        return ExitCode::FAILURE;
    };
    let diagnostics = check::source(&source, model.as_ref());
    let printed = print_diagnostics(format, &source, file, &diagnostics);

    if check::has_errors(&diagnostics) {
        return exit(printed, ExitCode::FAILURE);
    }

    // In JSON, the document is the whole answer: an empty (or
    // warnings-only) list and exit status 0 already say "no errors".
    let printed = match format {
        Format::Human => printed.and_then(|()| writeln!(io::stdout().lock(), "no errors")),
        Format::Json => printed,
    };
    exit(printed, ExitCode::SUCCESS)
}

/// The exit status once output is written: `status`, the check's own
/// result, unless writing failed.
///
/// A reader that goes away early (`mesh check ... | head`) closes the
/// pipe, and every write after that fails with `BrokenPipe`. That isn't a
/// failure of the check, so the check's status stands and nothing more
/// is printed. Any other write error is reported, if stderr still works,
/// and fails the run.
fn exit(printed: io::Result<()>, status: ExitCode) -> ExitCode {
    match printed {
        Ok(()) => status,
        Err(err) if err.kind() == io::ErrorKind::BrokenPipe => status,
        Err(err) => {
            // Nothing more can be done if stderr is gone too.
            let _ = writeln!(io::stderr().lock(), "error: could not write output: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Reads `path`, or reports why it can't be read.
fn read(path: &str) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(err) => {
            // If stderr is gone, there's no one left to tell.
            let _ = writeln!(io::stderr().lock(), "error: could not read {path}: {err}");
            None
        }
    }
}

/// The component a file is the template of when `--component` isn't
/// given: its name without the extension, so `users-page.mprx` is
/// `users-page`.
fn file_stem(file: &str) -> String {
    Path::new(file)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Prints diagnostics in the order given (never re-sorted), all reported
/// against `source` at `path`.
///
/// Human: on stderr. The renderer returns a block with no trailing
/// newline; this loop owns the separation: each block is followed by
/// exactly one blank line.
///
/// JSON: one document on stdout, ending in a newline, even when there are
/// no diagnostics. Every run that checks anything prints exactly one, so
/// several runs' output is JSON Lines.
///
/// Stops at the first write that fails; see [`exit`].
fn print_diagnostics(
    format: Format,
    source: &str,
    path: &str,
    diagnostics: &[Diagnostic],
) -> io::Result<()> {
    if format == Format::Json {
        let document = mesh_compiler::render_json(source, path, diagnostics);
        return writeln!(io::stdout().lock(), "{document}");
    }
    let mut stderr = io::stderr().lock();
    for diagnostic in diagnostics {
        let block = mesh_compiler::render_diagnostic(source, path, diagnostic);
        writeln!(stderr, "{block}\n")?;
    }
    Ok(())
}
