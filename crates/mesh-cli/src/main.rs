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
    /// Check an MPRX file as a component's template and, if it has no
    /// errors, compile it to a template (`template-v1`).
    Compile {
        /// Path to the `.mprx` file to compile.
        file: String,
        /// The component manifest (JSON) to check the file against.
        #[arg(long, value_name = "FILE")]
        model: String,
        /// The manifest component whose template the file is. Defaults to
        /// the file's name without its extension.
        #[arg(long, value_name = "NAME")]
        component: Option<String>,
        /// Where to write the template. Without it, the template goes to
        /// stdout.
        #[arg(long, value_name = "PATH")]
        output: Option<String>,
        /// How to print diagnostics. `json` prints them on stdout, so it
        /// needs `--output`.
        #[arg(long, value_enum, default_value_t = Format::Human, requires_if("json", "output"))]
        format: Format,
    },
    /// Check a program of compiled templates: the manifest, each
    /// template, their fingerprints and the assembly rules. It writes
    /// nothing.
    CheckProgram {
        /// The component manifest (JSON) the templates were compiled
        /// against.
        #[arg(long, value_name = "FILE")]
        model: String,
        /// The root component: the one the program renders.
        #[arg(long, value_name = "NAME")]
        root: String,
        /// The program's templates (`template-v1`), in order.
        #[arg(required = true, value_name = "TEMPLATE")]
        templates: Vec<String>,
        /// How to print diagnostics. `json` prints the runtime
        /// diagnostics document (`schemas/runtime-diagnostics-v1.schema.json`).
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
        Command::Compile {
            file,
            model,
            component,
            output,
            format,
        } => run_compile(
            &file,
            &model,
            component.as_deref(),
            output.as_deref(),
            format,
        ),
        Command::CheckProgram {
            model,
            root,
            templates,
            format,
        } => run_check_program(&model, &root, &templates, format),
    }
}

/// Checks a program as `mesh_compiler::check::program` defines it: this
/// reads the files and prints, and decides nothing else. Every file is
/// read first; one that can't be read is reported and nothing is checked.
///
/// Human: each diagnostic on stderr as `error[<code>]: <message>` and a
/// `-->` line for its location, then `no errors` on stdout if there are
/// none. A template carries no source text, so a `source` location gives
/// the template's path, its component and the span's byte offsets.
/// JSON: the runtime diagnostics document on stdout.
fn run_check_program(model: &str, root: &str, templates: &[String], format: Format) -> ExitCode {
    let Some(manifest) = read(model) else {
        return ExitCode::FAILURE;
    };
    let mut texts = Vec::new();
    for path in templates {
        let Some(text) = read(path) else {
            return ExitCode::FAILURE;
        };
        texts.push(text);
    }
    let texts: Vec<&str> = texts.iter().map(String::as_str).collect();
    let diagnostics = check::program(&manifest, root, &texts);
    let printed = match format {
        Format::Json => writeln!(
            io::stdout().lock(),
            "{}",
            mesh_runtime::to_json(&diagnostics, &manifest)
        ),
        Format::Human => {
            print_program_diagnostics(&diagnostics, model, &manifest, root, templates, &texts)
        }
    };
    if !diagnostics.is_empty() {
        return exit(printed, ExitCode::FAILURE);
    }
    let printed = match format {
        Format::Human => printed.and_then(|()| writeln!(io::stdout().lock(), "no errors")),
        Format::Json => printed,
    };
    exit(printed, ExitCode::SUCCESS)
}

/// The human form of `mesh check-program`'s diagnostics, on stderr.
fn print_program_diagnostics(
    diagnostics: &[mesh_runtime::RuntimeDiagnostic],
    model: &str,
    manifest: &str,
    root: &str,
    templates: &[String],
    texts: &[&str],
) -> io::Result<()> {
    use mesh_runtime::Location;
    let map = mesh_syntax::source_map::SourceMap::new(manifest);
    let mut stderr = io::stderr().lock();
    for diagnostic in diagnostics {
        let at = match &diagnostic.location {
            Location::Model(span) => {
                let at = map.line_column(
                    manifest,
                    span.start_byte,
                    mesh_syntax::source_map::ColumnUnit::Char,
                );
                format!("{model}:{}:{}", at.line + 1, at.column + 1)
            }
            Location::Program => format!("the program, whose root is `{root}`"),
            Location::Template { index, .. } => templates[*index].clone(),
            Location::Source { component, span } => {
                let path = template_paths(component, templates, texts);
                format!(
                    "{path}, the template of `{component}`, source bytes {}..{}",
                    span.start.byte, span.end.byte
                )
            }
            // Program validation never reports these.
            Location::Input(_) | Location::Handler => String::new(),
        };
        writeln!(
            stderr,
            "error[{}]: {}\n  --> {at}\n",
            diagnostic.code, diagnostic.message
        )?;
    }
    Ok(())
}

/// The path of the template of `component`: every one, if the program
/// has several (which it reports as an error of its own).
fn template_paths(component: &str, templates: &[String], texts: &[&str]) -> String {
    let mut paths: Vec<&str> = templates
        .iter()
        .zip(texts)
        .filter(|(_, text)| {
            mesh_template::from_json(text).is_ok_and(|template| template.component == component)
        })
        .map(|(path, _)| path.as_str())
        .collect();
    paths.dedup();
    paths.join(" or ")
}

/// Checks `file` as `mesh_compiler::check` defines a check. This only
/// reads files, derives the component from the file's stem when
/// `--component` isn't given, and prints: the check's two phases decide
/// everything else. The model loads before the file is read, so a broken
/// manifest is reported even when the file can't be read.
fn run_check(file: &str, model: Option<&str>, component: Option<&str>, format: Format) -> ExitCode {
    let model = match model {
        Some(path) => match load_model(file, path, component, format) {
            Ok(model) => Some(model),
            Err(status) => return status,
        },
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

/// The check's first phase: loads the manifest at `path` and looks up the
/// component (`component`, or `file`'s stem). On failure, prints the
/// manifest's diagnostics against it and gives the exit status.
fn load_model(
    file: &str,
    path: &str,
    component: Option<&str>,
    format: Format,
) -> Result<check::Model, ExitCode> {
    let Some(text) = read(path) else {
        return Err(ExitCode::FAILURE);
    };
    let name = component.map_or_else(|| file_stem(file), str::to_string);
    check::Model::load(&text, &name).map_err(|diagnostics| {
        let printed = print_diagnostics(format, &text, path, &diagnostics);
        exit(printed, ExitCode::FAILURE)
    })
}

/// Compiles `file` as `mesh_compiler::check::template` defines it, as
/// `run_check` checks: the same phases, diagnostics and exit status.
/// Only when there's no error is the template written: to `output`, or
/// to stdout. With errors, nothing is written, and an existing `output`
/// is left alone. In JSON, the diagnostics document is always printed on
/// stdout, which is why JSON needs `--output`.
fn run_compile(
    file: &str,
    model: &str,
    component: Option<&str>,
    output: Option<&str>,
    format: Format,
) -> ExitCode {
    let model = match load_model(file, model, component, format) {
        Ok(model) => model,
        Err(status) => return status,
    };
    let Some(source) = read(file) else {
        return ExitCode::FAILURE;
    };
    let compiled = check::template(&source, &model);
    let printed = print_diagnostics(format, &source, file, &compiled.diagnostics);
    let Some(template) = compiled.template else {
        return exit(printed, ExitCode::FAILURE);
    };
    let document = mesh_template::to_json(&template) + "\n";
    let written = match output {
        Some(path) => {
            if let Err(err) = fs::write(path, &document) {
                // If stderr is gone, there's no one left to tell.
                let _ = writeln!(io::stderr().lock(), "error: could not write {path}: {err}");
                return ExitCode::FAILURE;
            }
            printed
        }
        None => printed.and_then(|()| io::stdout().lock().write_all(document.as_bytes())),
    };
    exit(written, ExitCode::SUCCESS)
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
