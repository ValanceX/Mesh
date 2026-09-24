//! Type checking, diagnostics, transformation, optimization, and code
//! generation for MPRX, built on the semantic model in `mesh-semantic`.
//!
//! This crate orchestrates parse → lower and aggregates diagnostics.
//! [`compile_with`] also takes the component model a template is checked
//! against ([`CompileOptions`]); no check uses it yet.

mod render;

pub use render::render_diagnostic;

/// The result of compiling one MPRX source file: the Semantic IR, if
/// compilation succeeded, and every diagnostic produced along the way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileResult {
    pub ir: Option<mesh_semantic::Element>,
    pub diagnostics: Vec<mesh_syntax::Diagnostic>,
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
/// A template in `options` is accepted but not yet used: no check reads
/// the component model until name resolution lands, so the result is
/// the same as [`compile`]'s.
pub fn compile_with(source: &str, options: &CompileOptions<'_>) -> CompileResult {
    // Destructured so that a new option can't be silently ignored here.
    let CompileOptions { template: _ } = options;
    let parsed = mesh_parser::parse(source);
    let mut diagnostics: Vec<mesh_syntax::Diagnostic> = parsed
        .errors
        .into_iter()
        .map(|err| mesh_syntax::Diagnostic {
            severity: mesh_syntax::Severity::Error,
            code: err.code,
            message: err.message,
            span: err.span,
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

    CompileResult { ir, diagnostics }
}
