//! Parses MPRX source text via the `tree-sitter-mprx` grammar and lowers the
//! resulting concrete syntax tree into the `mesh-syntax` AST.
//!
//! The Tree-sitter CST is an implementation detail of this crate — it is
//! never exposed as the application's final UI model.

use mesh_syntax::{Attribute, Element, Span, StringLiteral, Text};
use tree_sitter::Node;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

pub fn parse(source: &str) -> Result<Element, ParseError> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_mprx::language())
        .expect("loading the MPRX grammar should never fail");

    let tree = parser.parse(source, None).ok_or_else(|| ParseError {
        message: "failed to parse source".to_string(),
        span: Span { start_byte: 0, end_byte: source.len() },
    })?;

    let root = tree.root_node();
    if root.has_error() {
        return Err(ParseError {
            message: "syntax error".to_string(),
            span: span_of(root),
        });
    }

    let element_node = root
        .named_child(0)
        .and_then(|element| element.child(0))
        .ok_or_else(|| ParseError {
            message: "expected a single root element".to_string(),
            span: Span { start_byte: 0, end_byte: source.len() },
        })?;

    Ok(lower_element(element_node, source))
}

fn lower_element(node: Node, source: &str) -> Element {
    let name = node
        .child_by_field_name("name")
        .map(|n| text_of(n, source))
        .unwrap_or_default();

    let mut cursor = node.walk();
    let attributes = node
        .children(&mut cursor)
        .filter(|n| n.kind() == "attribute")
        .map(|n| lower_attribute(n, source))
        .collect();

    let children = node
        .child_by_field_name("text")
        .map(|n| vec![Text { value: text_of(n, source), span: span_of(n) }])
        .unwrap_or_default();

    Element { name, attributes, children, span: span_of(node) }
}

fn lower_attribute(node: Node, source: &str) -> Attribute {
    let name = node
        .child_by_field_name("name")
        .map(|n| text_of(n, source))
        .unwrap_or_default();

    let value_node = node.child_by_field_name("value");
    let value = StringLiteral {
        value: value_node
            .and_then(|v| v.child_by_field_name("value"))
            .map(|n| text_of(n, source))
            .unwrap_or_default(),
        span: value_node
            .map(span_of)
            .unwrap_or(Span { start_byte: 0, end_byte: 0 }),
    };

    Attribute { name, value, span: span_of(node) }
}

fn text_of(node: Node, source: &str) -> String {
    node.utf8_text(source.as_bytes()).unwrap_or_default().to_string()
}

fn span_of(node: Node) -> Span {
    Span { start_byte: node.start_byte(), end_byte: node.end_byte() }
}
