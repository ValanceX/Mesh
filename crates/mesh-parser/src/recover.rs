//! Editor recovery: what an editor may still use of a file with syntax
//! errors.
//!
//! **Editor-only.** This is never part of [`crate::parse`], or of
//! `mesh-compiler`'s `compile` and `compile_with`, and never a source of
//! diagnostics (outline invariant I2). It exists so hover and
//! go-to-definition keep working on the parts of a file that parse while
//! another part is being typed. Its rules, and the Tree-sitter recovery
//! trees they were written against, are in
//! `docs/superpowers/specs/2026-09-24-mesh-v0.3-editor-recovery.md`
//! (R2–R5 here).
//!
//! It keeps only clean subtrees whose delimiters are real: an element
//! whose tag name was written, and an expression block whose `{` and `}`
//! are both there. After a mistake, Tree-sitter often builds clean-looking
//! subtrees out of what follows, such as a closing tag read as a name, so
//! nothing inside an `ERROR` is kept except whole elements.

use mesh_syntax::{Child, Element, Expression, Span, Text};
use tree_sitter::Node;

use crate::{
    field_text, lower_attribute, lower_event_binding, lower_expression, lower_expression_block,
    nesting, span_of, text_of,
};

/// What [`recover`] kept of a file: the elements that parse, pruned of
/// what doesn't, and the clean expressions of the expression blocks it
/// pruned. None overlaps another, except that an expression part lies
/// inside the element it was pruned from.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Recovered {
    /// Element parts, in source order.
    pub elements: Vec<Element>,
    /// Expression parts, in source order.
    pub expressions: Vec<RecoveredExpression>,
}

/// An expression kept from an expression block that was pruned.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveredExpression {
    /// The expression's span.
    pub span: Span,
    pub expression: Expression,
}

/// Recovers what parses of `source`, for an editor (see the module
/// documentation). Total: any text gives an answer, and a file with no
/// syntax error gives exactly one element, its whole tree.
pub fn recover(source: &str) -> Recovered {
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&tree_sitter_mprx::language()).is_err() {
        return Recovered::default();
    }
    let Some(tree) = parser.parse(source, None) else {
        return Recovered::default();
    };
    let root = tree.root_node();
    // R2: nothing recursive runs on a tree deeper than the limit.
    if nesting::check(root).is_some() {
        return Recovered::default();
    }
    let mut dropped = Vec::new();
    let elements = find_elements(root, source, &mut dropped);
    // R5: the clean expression of each pruned block whose braces are real.
    let mut expressions: Vec<RecoveredExpression> = dropped
        .into_iter()
        .filter_map(|block| clean_expression(block))
        .map(|node| RecoveredExpression {
            span: span_of(node),
            expression: lower_expression(node, source),
        })
        .collect();
    expressions.sort_by_key(|part| part.span.start_byte);
    Recovered {
        elements,
        expressions,
    }
}

/// R3: every element under `node`, in source order, without descending
/// into the elements found. Iterative, so a long chain of other nodes
/// can't exhaust the stack.
fn find_elements<'t>(node: Node<'t>, source: &str, dropped: &mut Vec<Node<'t>>) -> Vec<Element> {
    let mut elements = Vec::new();
    let mut stack = vec![node];
    while let Some(node) = stack.pop() {
        if is_element(node) {
            elements.push(recover_element(node, source, dropped));
            continue;
        }
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();
        stack.extend(children.into_iter().rev());
    }
    elements
}

/// A self-closing or container element whose tag name was written.
fn is_element(node: Node) -> bool {
    matches!(node.kind(), "self_closing_element" | "container_element")
        && node
            .child_by_field_name("name")
            .is_some_and(|name| name.kind() == "tag_name" && !name.is_missing())
}

/// R4: `node`, an element, keeping what has no error. Recursion is
/// bounded by R2's depth check.
fn recover_element<'t>(node: Node<'t>, source: &str, dropped: &mut Vec<Node<'t>>) -> Element {
    let (name, name_span) = field_text(node, "name", source);
    let mut attributes = Vec::new();
    let mut event_bindings = Vec::new();
    let mut children = Vec::new();
    let mut tag_names = 0;
    let mut closing_name = None;
    let mut closed = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "attribute" if !child.has_error() => attributes.push(lower_attribute(child, source)),
            "attribute" => drop_block(child.child_by_field_name("value"), dropped),
            "event_binding" if !child.has_error() => {
                event_bindings.push(lower_event_binding(child, source));
            }
            "event_binding" => drop_block(child.child_by_field_name("handler"), dropped),
            "child" => recover_child(child, source, dropped, &mut children),
            "ERROR" => children.extend(
                find_elements(child, source, dropped)
                    .into_iter()
                    .map(|element| Child::Element(Box::new(element))),
            ),
            "tag_name" => {
                tag_names += 1;
                if tag_names == 2 && !child.is_missing() {
                    closing_name = Some(text_of(child, source));
                }
            }
            ">" if tag_names == 2 && !child.is_missing() => closed = true,
            _ => {}
        }
    }

    Element {
        name,
        name_span,
        closing_name: closing_name.filter(|_| closed),
        attributes,
        event_bindings,
        children,
        span: span_of(node),
    }
}

fn recover_child<'t>(
    node: Node<'t>,
    source: &str,
    dropped: &mut Vec<Node<'t>>,
    children: &mut Vec<Child>,
) {
    let Some(inner) = node.child(0) else {
        return;
    };
    match inner.kind() {
        "text" => children.push(Child::Text(Text {
            value: text_of(inner, source),
            span: span_of(inner),
        })),
        "expression_block" if !inner.has_error() => {
            children.push(Child::Expression(lower_expression_block(inner, source)));
        }
        "expression_block" => dropped.push(inner),
        // The `element` wrapper, or anything else: whatever elements it
        // holds, in order.
        _ => children.extend(
            find_elements(inner, source, dropped)
                .into_iter()
                .map(|element| Child::Element(Box::new(element))),
        ),
    }
}

fn drop_block<'t>(block: Option<Node<'t>>, dropped: &mut Vec<Node<'t>>) {
    if let Some(block) = block.filter(|block| block.kind() == "expression_block") {
        dropped.push(block);
    }
}

/// The `expression` of `block` if the block's `{` and `}` are both there
/// and the expression is clean.
fn clean_expression(block: Node) -> Option<Node> {
    let count = block.child_count();
    let first = block.child(0)?;
    let last = block.child(count.checked_sub(1)?)?;
    let braces = first.kind() == "{"
        && !first.is_missing()
        && last.kind() == "}"
        && !last.is_missing()
        && count >= 2;
    let expression = block.child_by_field_name("expression")?;
    (braces
        && expression.kind() == "expression"
        && !expression.has_error()
        && !expression.is_missing())
    .then_some(expression)
}
