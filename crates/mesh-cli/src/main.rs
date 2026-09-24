//! Native CLI entry point for the MESH toolchain.

use clap::{Parser, Subcommand, ValueEnum};
use mesh_compiler::CompileOptions;
use mesh_syntax::{Diagnostic, Severity};
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

fn run_check(file: &str, model: Option<&str>, component: Option<&str>, format: Format) -> ExitCode {
    // The manifest is loaded and validated completely before the file is
    // read or checked. If it has errors, they are the only ones reported.
    let manifest = match model {
        Some(path) => {
            let Some(text) = read(path) else {
                return ExitCode::FAILURE;
            };
            match mesh_manifest::load(&text) {
                Ok(manifest) => Some((manifest, text, path)),
                Err(diagnostics) => {
                    let printed = print_diagnostics(format, &text, path, &diagnostics);
                    return exit(printed, ExitCode::FAILURE);
                }
            }
        }
        None => None,
    };

    let options = match &manifest {
        Some((manifest, text, path)) => {
            let name = component.map_or_else(|| file_stem(file), str::to_string);
            match manifest.template(&name) {
                Ok(template) => CompileOptions::with_template(template),
                Err(diagnostic) => {
                    let printed = print_diagnostics(format, text, path, &[diagnostic]);
                    return exit(printed, ExitCode::FAILURE);
                }
            }
        }
        None => CompileOptions::default(),
    };

    let Some(source) = read(file) else {
        return ExitCode::FAILURE;
    };
    let result = mesh_compiler::compile_with(&source, &options);
    let printed = print_diagnostics(format, &source, file, &result.diagnostics);

    // Only errors fail the check — warnings are reported but non-fatal.
    let has_errors = result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error);
    if has_errors {
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
