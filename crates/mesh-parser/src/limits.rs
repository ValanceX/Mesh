//! The input limits (decision G5): how large a source, how many elements and
//! how many attributes on one element MESH reads.
//!
//! They bound the work any later stage can be given, and so what a host that
//! checks text it didn't write (generated UI) has to budget for. Like the
//! nesting limit they are resource limits of this implementation, not rules of
//! MPRX, and a later version may change them. The size is checked before
//! parsing; the counts are checked by one iterative walk of the concrete
//! syntax tree, like the nesting limit.

use mesh_syntax::{DiagnosticCode, Span};
use tree_sitter::Node;

use crate::{nesting, span_of, ParseError};

/// The longest source, in bytes (64 KB).
pub const MAX_SOURCE_BYTES: usize = 64 * 1024;

/// The most elements in one source.
pub const MAX_ELEMENTS: usize = 5_000;

/// The most attributes on one element. An event binding is one.
pub const MAX_ATTRIBUTES: usize = 64;

/// A source longer than [`MAX_SOURCE_BYTES`], reported as `input-too-large` at
/// the first byte past the limit (on a character boundary), before anything
/// reads it.
pub(crate) fn check_size(source: &str) -> Option<ParseError> {
    if source.len() <= MAX_SOURCE_BYTES {
        return None;
    }
    let start = source.floor_char_boundary(MAX_SOURCE_BYTES);
    Some(ParseError {
        code: DiagnosticCode::INPUT_TOO_LARGE,
        message: format!(
            "this source is {} bytes, more than the {MAX_SOURCE_BYTES} MESH reads; \
             split it into components",
            source.len()
        ),
        span: Span {
            start_byte: start,
            end_byte: source.len(),
        },
    })
}

/// The first element, in source order, that is the one after
/// [`MAX_ELEMENTS`] (`too-many-elements`, at its tag name), or the first
/// attribute that is the one after [`MAX_ATTRIBUTES`] on its element
/// (`too-many-attributes`). Only called on a tree without syntax errors.
pub(crate) fn check_tree(root: Node) -> Option<ParseError> {
    let mut elements = 0;
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == "element" {
            elements += 1;
            if elements > MAX_ELEMENTS {
                return Some(ParseError {
                    code: DiagnosticCode::TOO_MANY_ELEMENTS,
                    message: format!(
                        "this is element {elements}, past the {MAX_ELEMENTS} MESH reads in one \
                         source; split it into components"
                    ),
                    span: nesting::level_span(node),
                });
            }
        }
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();
        let mut attributes = 0;
        for child in &children {
            if matches!(child.kind(), "attribute" | "event_binding") {
                attributes += 1;
                if attributes > MAX_ATTRIBUTES {
                    return Some(ParseError {
                        code: DiagnosticCode::TOO_MANY_ATTRIBUTES,
                        message: format!(
                            "this is attribute {attributes} on one element, past the \
                             {MAX_ATTRIBUTES} MESH reads; pass a record in one prop instead"
                        ),
                        span: span_of(*child),
                    });
                }
            }
        }
        stack.extend(children.into_iter().rev());
    }
    None
}
