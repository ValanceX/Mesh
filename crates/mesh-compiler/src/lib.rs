//! Type checking, diagnostics, transformation, optimization, and code
//! generation for MPRX, built on the semantic model in `mesh-semantic`.
//!
//! For v0.1 this crate only orchestrates parse → lower and aggregates
//! diagnostics; type checking against a component model is not yet
//! implemented (see the v0.1 roadmap design spec).

use std::fmt;

/// How serious a [`Diagnostic`] is.
///
/// Only `Error` exists for v0.1 — a compile either fully succeeds or
/// produces exactly one fatal error. Marked `#[non_exhaustive]` because
/// the Pass 4 diagnostics work is expected to add more variants (e.g.
/// `Warning`), and that should not be a breaking change for consumers.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
}

/// A single compile-time diagnostic: a message, its severity, and the
/// source [`mesh_syntax::Span`] it applies to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: mesh_syntax::Span,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}..{})",
            self.message, self.span.start_byte, self.span.end_byte
        )
    }
}

/// The result of compiling one MPRX source file: the Semantic IR, if
/// compilation succeeded, and every diagnostic produced along the way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileResult {
    pub ir: Option<mesh_semantic::Element>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Compiles `source`: parses it, lowers the AST into the Semantic IR, and
/// collects any diagnostics produced along the way.
///
/// Returns a [`CompileResult`] rather than a `Result` because a compile
/// will be able to produce diagnostics without failing outright once
/// warnings exist (Pass 4) — check `diagnostics.is_empty()` or
/// `ir.is_some()` depending on what you need.
pub fn compile(source: &str) -> CompileResult {
    match mesh_parser::parse(source) {
        Ok(ast) => {
            let ir = mesh_semantic::lower(&ast);
            CompileResult {
                ir: Some(ir),
                diagnostics: vec![],
            }
        }
        Err(err) => CompileResult {
            ir: None,
            diagnostics: vec![Diagnostic {
                severity: Severity::Error,
                message: err.message,
                span: err.span,
            }],
        },
    }
}
