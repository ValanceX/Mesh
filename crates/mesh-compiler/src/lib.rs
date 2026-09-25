//! The compiler entry point for MPRX: source text to Semantic IR, with
//! every diagnostic found along the way, rendered rustc-style or as JSON.
//!
//! This crate orchestrates parse → lower → analyze and aggregates
//! diagnostics. [`compile_with`] also takes the component model a
//! template is checked against ([`CompileOptions`]); with one, the IR is
//! analyzed against it (`mesh-analysis`), and this crate's diagnostic
//! layer turns what analysis found into diagnostics.

pub mod check;
mod diagnose;
pub mod editor;
mod emit;
mod json;
mod position;
mod render;
/// Positions in a source: [`mesh_syntax::source_map`], re-exported here
/// where it has always been.
pub use mesh_syntax::source_map;

pub use json::render_json;
pub use render::render_diagnostic;
pub use source_map::{ColumnUnit, LineColumn, SourceMap};

/// The result of compiling one MPRX source file: the Semantic IR, if
/// compilation succeeded, every diagnostic produced along the way, and,
/// when the file was checked against a component model, what analysis
/// found.
///
/// It is `#[non_exhaustive]`: only this crate builds one, so a later
/// field doesn't break code that reads it.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileResult {
    pub ir: Option<mesh_semantic::Element>,
    pub diagnostics: Vec<mesh_syntax::Diagnostic>,
    /// The analysis of `ir` against the template in [`CompileOptions`]:
    /// its resolutions, types and facts. The analysis diagnostics in
    /// `diagnostics` are exactly its facts, in order. `None` without a
    /// template, or without IR (a file with a syntax error).
    pub analysis: Option<mesh_analysis::Analysis>,
}

/// What a compile checks `source` against, beyond the MPRX language.
///
/// The default is no component model: the compile checks syntax and
/// structure exactly as v0.1 did.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default)]
pub struct CompileOptions<'m> {
    /// The component manifest, and the component whose template `source`
    /// is. `None` checks without a model.
    pub template: Option<mesh_manifest::Template<'m>>,
}

impl<'m> CompileOptions<'m> {
    /// Options that check `source` as the template of `template`.
    pub fn with_template(template: mesh_manifest::Template<'m>) -> Self {
        CompileOptions {
            template: Some(template),
        }
    }
}

/// Compiles `source` with no component model: parses it, lowers the AST
/// into the Semantic IR, and collects any diagnostics produced along the
/// way. The same as [`compile_with`] and default [`CompileOptions`].
///
/// Returns a [`CompileResult`] rather than a `Result` because a compile
/// can produce diagnostics without failing outright — check
/// `diagnostics.is_empty()` or `ir.is_some()` depending on what you need.
pub fn compile(source: &str) -> CompileResult {
    compile_with(source, &CompileOptions::default())
}

/// Compiles `source` as [`compile`] does, with `options`.
///
/// With a template in `options`, the IR is also analyzed as that
/// component's template, and analysis diagnostics follow the parse and
/// lowering ones, in source order. Analysis needs IR, so a file with a
/// syntax error gets no analysis diagnostics.
pub fn compile_with(source: &str, options: &CompileOptions<'_>) -> CompileResult {
    // Destructured so that a new option can't be silently ignored here.
    let CompileOptions { template } = options;
    let parsed = mesh_parser::parse(source);
    let mut diagnostics: Vec<mesh_syntax::Diagnostic> = parsed
        .errors
        .into_iter()
        .map(|err| mesh_syntax::Diagnostic {
            severity: mesh_syntax::Severity::Error,
            code: err.code,
            message: err.message,
            span: err.span,
            suggestions: Vec::new(),
        })
        .collect();

    let ir = match parsed.ast {
        Some(ast) => {
            let lowered = mesh_semantic::lower(&ast);
            diagnostics.extend(lowered.diagnostics);
            lowered.ir
        }
        None => None,
    };

    let analysis = match (&ir, template) {
        (Some(ir), Some(template)) => {
            let analysis = mesh_analysis::analyze(ir, *template);
            diagnostics.extend(analysis.facts().iter().map(diagnose::diagnostic));
            Some(analysis)
        }
        _ => None,
    };

    CompileResult {
        ir,
        diagnostics,
        analysis,
    }
}
