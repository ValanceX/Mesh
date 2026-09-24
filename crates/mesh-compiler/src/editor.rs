//! **Editor-only** answers about files with syntax errors.
//!
//! [`crate::compile_with`] gives a file with a syntax error its syntax
//! errors and nothing else: no IR, no analysis. That is the right answer,
//! and it stays the only answer about what a file *means* (outline
//! invariant I2). But while someone is typing, a file is broken most of
//! the time, and an editor still wants hover and go-to-definition on the
//! parts that parse.
//!
//! [`recover`] gives it those parts, analyzed by the same analysis code
//! as a compile, against the same template. Its [`Recovery`] answers only
//! "what name, or what typed expression, is at this offset". It has no
//! facts and no diagnostics, on purpose, so nothing recovered can be
//! reported as a mistake, cached as the compiler's verdict, or compared
//! with it. `compile`, `compile_with` and `mesh check` never call it.
//!
//! The rules for what is recovered are in
//! `docs/superpowers/specs/2026-09-24-mesh-v0.3-editor-recovery.md`.

use mesh_analysis::{Analysis, Resolution, Typed};
use mesh_manifest::Template;
use mesh_syntax::Span;

/// What the parts of a broken file that parse mean: for each part, the
/// resolutions and types analysis gave it.
///
/// It has no facts and no diagnostics, so it can't report anything:
///
/// ```compile_fail
/// fn report(recovery: &mesh_compiler::editor::Recovery) {
///     let _ = recovery.facts();
/// }
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Recovery {
    /// One analysis per recovered part, in source order. Their facts are
    /// never read.
    parts: Vec<Analysis>,
}

impl Recovery {
    /// The resolved name at byte `offset`, by [`Analysis::resolution_at`]'s
    /// rule across every part: the innermost wins.
    pub fn resolution_at(&self, offset: usize) -> Option<&Resolution> {
        innermost(
            self.parts
                .iter()
                .filter_map(|part| part.resolution_at(offset)),
            |resolution| resolution.span,
        )
    }

    /// The innermost typed expression at byte `offset`, by
    /// [`Analysis::typed_at`]'s rule across every part.
    pub fn typed_at(&self, offset: usize) -> Option<&Typed> {
        innermost(
            self.parts.iter().filter_map(|part| part.typed_at(offset)),
            |typed| typed.span,
        )
    }

    /// How many parts were recovered.
    pub fn len(&self) -> usize {
        self.parts.len()
    }

    /// Whether nothing was recovered.
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }
}

/// The shortest span, the later start on a tie: `Analysis`'s own rule.
fn innermost<'a, T>(
    items: impl Iterator<Item = &'a T>,
    span: impl Fn(&T) -> Span,
) -> Option<&'a T> {
    items.min_by_key(|item| {
        let span = span(item);
        (
            span.end_byte - span.start_byte,
            std::cmp::Reverse(span.start_byte),
        )
    })
}

/// Recovers what parses of `source` and analyzes it as the template of
/// `template` (see the module documentation). Every element part is
/// analyzed as [`crate::compile_with`] would analyze it inside the whole
/// file, and every expression part as an unconstrained value. Lowering
/// diagnostics and facts are dropped here.
pub fn recover(source: &str, template: Template<'_>) -> Recovery {
    let recovered = mesh_parser::recover(source);
    let mut parts: Vec<(usize, Analysis)> = Vec::new();
    for element in &recovered.elements {
        if let Some(ir) = mesh_semantic::lower(element).ir {
            parts.push((ir.span.start_byte, mesh_analysis::analyze(&ir, template)));
        }
    }
    for part in &recovered.expressions {
        let expression = mesh_semantic::lower_expression(&part.expression);
        parts.push((
            part.span.start_byte,
            mesh_analysis::analyze_expression(&expression, template),
        ));
    }
    parts.sort_by_key(|(start, _)| *start);
    Recovery {
        parts: parts.into_iter().map(|(_, analysis)| analysis).collect(),
    }
}
