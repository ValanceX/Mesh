//! The nesting limit: how deep elements and expressions may nest.
//!
//! Lowering, validation and analysis all walk the tree recursively, one
//! stack frame (or several) per level, and so do the derived `Clone`,
//! `PartialEq` and `Drop` of the tree types. Without a limit, a valid file
//! nested a few thousand levels deep overflows the stack and aborts the
//! process, which no caller can catch. The limit is checked here, before
//! anything recursive runs, by an iterative walk of the concrete syntax
//! tree.

use mesh_syntax::{DiagnosticCode, Span};
use tree_sitter::Node;

use crate::{span_of, ParseError};

/// The deepest that elements and expressions may nest. Every element and
/// every expression inside another counts as one level, and so does each
/// pair of parentheses: `<a><b x={-(y)} /></a>` is 5 levels deep at `y`.
/// That is deeper than any handwritten UI goes, and shallow enough that
/// the recursive stages after parsing fit in a small thread stack.
pub const MAX_NESTING_DEPTH: usize = 128;

/// The first element or expression, in source order, that is nested more
/// than [`MAX_NESTING_DEPTH`] levels deep, reported as a
/// `nesting-too-deep` error. Only called on a tree without syntax errors.
pub(crate) fn check(root: Node) -> Option<ParseError> {
    // Children are pushed in reverse so they pop in source order, as in
    // `syntax_errors::collect`. Each entry carries its nesting depth.
    let mut stack = vec![(root, 0)];
    while let Some((node, depth)) = stack.pop() {
        let depth = depth + usize::from(is_level(node));
        if depth > MAX_NESTING_DEPTH {
            return Some(ParseError {
                code: DiagnosticCode::NESTING_TOO_DEEP,
                message: format!(
                    "this is nested more than {MAX_NESTING_DEPTH} levels deep; \
                     elements and expressions can't nest any deeper"
                ),
                span: level_span(node),
            });
        }
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();
        stack.extend(children.into_iter().rev().map(|child| (child, depth)));
    }
    None
}

/// Whether `node` is one level of nesting: an element or an expression.
/// A member access's object is a `member_access` node rather than an
/// `expression`, so it is counted on its own.
fn is_level(node: Node) -> bool {
    matches!(node.kind(), "element" | "expression" | "member_access")
}

/// Where a level that is too deep is reported: an element at its tag name,
/// since the whole element can span many lines, and an expression as a
/// whole.
fn level_span(node: Node) -> Span {
    node.child(0)
        .filter(|_| node.kind() == "element")
        .and_then(|element| element.child_by_field_name("name"))
        .map_or_else(|| span_of(node), span_of)
}
