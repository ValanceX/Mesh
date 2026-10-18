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

/// The deepest that MESH supports nesting elements and expressions. MPRX
/// itself has no limit: this is a resource limit of this implementation
/// (outline D12), and a later version may change it. Every element and
/// every expression inside another counts as one level, a block (`{#if}`,
/// `{#each}`) counts as three, and so does each
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
        let depth = depth + levels(node);
        if depth > MAX_NESTING_DEPTH {
            return Some(ParseError {
                code: DiagnosticCode::NESTING_TOO_DEEP,
                message: format!(
                    "this is nested more than {MAX_NESTING_DEPTH} levels deep, \
                     the deepest MESH supports"
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

/// How many levels of nesting `node` is: an element or an expression is
/// one, a block three (its branch or body, the content inside it, and
/// the block itself), anything else none. A member access's object is a
/// `member_access` node rather than an `expression`, so it is counted on
/// its own.
fn levels(node: Node) -> usize {
    match node.kind() {
        "element" | "expression" | "member_access" => 1,
        "if_block" | "each_block" | "snippet_block" => 3,
        _ => 0,
    }
}

/// Where a level that is too deep is reported: an element at its tag name,
/// since the whole element can span many lines, and an expression as a
/// whole.
pub(crate) fn level_span(node: Node) -> Span {
    if matches!(node.kind(), "if_block" | "each_block" | "snippet_block") {
        return node
            .child_by_field_name("keyword")
            .map_or_else(|| span_of(node), span_of);
    }
    node.child(0)
        .filter(|_| node.kind() == "element")
        .and_then(|element| element.child_by_field_name("name"))
        .map_or_else(|| span_of(node), span_of)
}
