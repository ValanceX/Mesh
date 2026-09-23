//! Type checking, diagnostics, transformation, optimization, and code
//! generation for MPRX, built on the semantic model in `mesh-semantic`.
//!
//! For v0.1 this crate only orchestrates parse → lower and aggregates
//! diagnostics; type checking against a component model is not yet
//! implemented (see the v0.1 roadmap design spec).

/// The result of compiling one MPRX source file: the Semantic IR, if
/// compilation succeeded, and every diagnostic produced along the way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileResult {
    pub ir: Option<mesh_semantic::Element>,
    pub diagnostics: Vec<mesh_syntax::Diagnostic>,
}

/// Compiles `source`: parses it, lowers the AST into the Semantic IR, and
/// collects any diagnostics produced along the way.
///
/// Returns a [`CompileResult`] rather than a `Result` because a compile
/// will be able to produce diagnostics without failing outright once
/// warnings exist (Pass 4) — check `diagnostics.is_empty()` or
/// `ir.is_some()` depending on what you need.
pub fn compile(source: &str) -> CompileResult {
    let parsed = mesh_parser::parse(source);
    let mut diagnostics: Vec<mesh_syntax::Diagnostic> = parsed
        .errors
        .into_iter()
        .map(|err| mesh_syntax::Diagnostic {
            severity: mesh_syntax::Severity::Error,
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
