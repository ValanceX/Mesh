//! Type checking, diagnostics, transformation, optimization, and code
//! generation for MPRX, built on the semantic model in `mesh-semantic`.
//!
//! For v0.1 this crate only orchestrates parse → lower and aggregates
//! diagnostics; type checking against a component model is not yet
//! implemented (see the v0.1 roadmap design spec).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: mesh_syntax::Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileResult {
    pub ir: Option<mesh_semantic::Element>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn compile(source: &str) -> CompileResult {
    match mesh_parser::parse(source) {
        Ok(ast) => {
            let ir = mesh_semantic::lower(&ast);
            CompileResult { ir: Some(ir), diagnostics: vec![] }
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
