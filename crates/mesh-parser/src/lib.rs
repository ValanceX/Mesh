//! Parses MPRX source text via the `tree-sitter-mprx` grammar and lowers the
//! resulting concrete syntax tree into the `mesh-syntax` AST.
//!
//! The Tree-sitter CST is an implementation detail of this crate — it is
//! never exposed as the application's final UI model.

use mesh_syntax::{
    Attribute, AttributeValue, BinaryExpression, BinaryOperator, BooleanLiteral, Child,
    ConditionalExpression, Element, Expression, Literal, MemberAccess, NullLiteral, NumberLiteral,
    Reference, Span, StringLiteral, Text, UnaryExpression, UnaryOperator,
};
use tree_sitter::Node;

/// An error produced while parsing MPRX source text.
///
/// Currently carries a single message and the source [`Span`] it applies
/// to. `mesh_parser::parse`/`mesh_semantic::lower`'s signatures will need
/// to change to carry multiple diagnostics once Pass 4 needs more than one
/// per compile — see the v0.1 roadmap.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

/// Parses `source` as MPRX and lowers the resulting concrete syntax tree
/// into the [`mesh_syntax`] AST.
///
/// # Errors
///
/// Returns [`ParseError`] if the source fails to parse outright, if the
/// resulting tree contains a syntax error, or if the tree does not contain
/// exactly one root element.
pub fn parse(source: &str) -> Result<Element, ParseError> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_mprx::language())
        .expect("loading the MPRX grammar should never fail");

    let tree = parser.parse(source, None).ok_or_else(|| ParseError {
        message: "failed to parse source".to_string(),
        span: Span {
            start_byte: 0,
            end_byte: source.len(),
        },
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
            span: Span {
                start_byte: 0,
                end_byte: source.len(),
            },
        })?;

    Ok(lower_element(element_node, source))
}

fn lower_element(node: Node, source: &str) -> Element {
    let name = node
        .child_by_field_name("name")
        .map(|n| text_of(n, source))
        .unwrap_or_default();

    let mut attr_cursor = node.walk();
    let attributes = node
        .children(&mut attr_cursor)
        .filter(|n| n.kind() == "attribute")
        .map(|n| lower_attribute(n, source))
        .collect();

    let mut child_cursor = node.walk();
    let children = node
        .children(&mut child_cursor)
        .filter(|n| n.kind() == "child")
        .map(|n| lower_child(n, source))
        .collect();

    Element {
        name,
        attributes,
        children,
        span: span_of(node),
    }
}

fn lower_child(node: Node, source: &str) -> Child {
    let inner = node.child(0).unwrap_or(node);
    match inner.kind() {
        "expression_block" => Child::Expression(lower_expression_block(inner, source)),
        "text" => Child::Text(Text {
            value: text_of(inner, source),
            span: span_of(inner),
        }),
        other => {
            unreachable!("unexpected child node kind {other:?} inside a successfully-parsed tree")
        }
    }
}

fn lower_attribute(node: Node, source: &str) -> Attribute {
    let name = node
        .child_by_field_name("name")
        .map(|n| text_of(n, source))
        .unwrap_or_default();

    let value_node = node.child_by_field_name("value");
    let value = match value_node {
        Some(v) if v.kind() == "expression_block" => {
            AttributeValue::Expression(lower_expression_block(v, source))
        }
        Some(v) => AttributeValue::String(lower_string(v, source)),
        None => AttributeValue::String(StringLiteral {
            value: String::new(),
            span: span_of(node),
        }),
    };

    Attribute {
        name,
        value,
        span: span_of(node),
    }
}

fn lower_expression_block(node: Node, source: &str) -> Expression {
    match node.child_by_field_name("expression") {
        Some(e) => lower_expression(e, source),
        None => missing_expression(node),
    }
}

/// A placeholder used where the grammar guarantees a field is present for
/// any successfully-parsed tree — the branch it backs is never actually
/// reached in practice. Returning a harmlessly-empty reference here (as
/// opposed to `unwrap`-ing) means a future grammar bug would surface as a
/// wrong-but-visible AST shape rather than a panic.
fn missing_expression(node: Node) -> Expression {
    Expression::Reference(Reference {
        name: String::new(),
        span: span_of(node),
    })
}

fn lower_string(node: Node, source: &str) -> StringLiteral {
    let raw = node
        .child_by_field_name("value")
        .map(|n| text_of(n, source))
        .unwrap_or_default();

    StringLiteral {
        value: decode_string_escapes(&raw),
        span: span_of(node),
    }
}

fn decode_string_escapes(raw: &str) -> String {
    let mut result = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            result.push(c);
            continue;
        }
        match chars.next() {
            Some('"') => result.push('"'),
            Some('\\') => result.push('\\'),
            Some('n') => result.push('\n'),
            Some('t') => result.push('\t'),
            Some(other) => {
                result.push('\\');
                result.push(other);
            }
            None => result.push('\\'),
        }
    }
    result
}

fn lower_expression(node: Node, source: &str) -> Expression {
    // `node` is an `expression` node; its single named child is the real
    // variant — except the parenthesized-grouping case, where that child
    // is itself another `expression` node to recurse into (no AST node
    // for the parens themselves). Uses `named_child` rather than `child`
    // specifically because of that case: the parenthesized alternative's
    // literal `(`/`)` tokens are anonymous children that would otherwise
    // land at index 0.
    let Some(inner) = node.named_child(0) else {
        return missing_expression(node);
    };
    match inner.kind() {
        "expression" => lower_expression(inner, source),
        "literal" => Expression::Literal(lower_literal(inner, source)),
        "member_access" => Expression::MemberAccess(lower_member_access(inner, source)),
        "reference" => Expression::Reference(lower_reference(inner, source)),
        "unary_expression" => Expression::Unary(lower_unary_expression(inner, source)),
        "binary_expression" => Expression::Binary(lower_binary_expression(inner, source)),
        "conditional_expression" => {
            Expression::Conditional(lower_conditional_expression(inner, source))
        }
        other => unreachable!(
            "unexpected expression node kind {other:?} inside a successfully-parsed tree"
        ),
    }
}

fn lower_unary_expression(node: Node, source: &str) -> UnaryExpression {
    let operator = match node.child_by_field_name("operator") {
        Some(n) => lower_unary_operator(&text_of(n, source)),
        None => unreachable!(
            "unary_expression node missing its operator field inside a successfully-parsed tree"
        ),
    };

    let operand = node
        .child_by_field_name("operand")
        .map(|n| lower_expression(n, source))
        .unwrap_or_else(|| missing_expression(node));

    UnaryExpression {
        operator,
        operand: Box::new(operand),
        span: span_of(node),
    }
}

fn lower_unary_operator(text: &str) -> UnaryOperator {
    match text {
        "!" => UnaryOperator::Not,
        "-" => UnaryOperator::Negate,
        other => unreachable!(
            "unexpected unary operator token {other:?} inside a successfully-parsed tree"
        ),
    }
}

fn lower_binary_expression(node: Node, source: &str) -> BinaryExpression {
    let operator = match node.child_by_field_name("operator") {
        Some(n) => lower_binary_operator(&text_of(n, source)),
        None => unreachable!(
            "binary_expression node missing its operator field inside a successfully-parsed tree"
        ),
    };

    let left = node
        .child_by_field_name("left")
        .map(|n| lower_expression(n, source))
        .unwrap_or_else(|| missing_expression(node));

    let right = node
        .child_by_field_name("right")
        .map(|n| lower_expression(n, source))
        .unwrap_or_else(|| missing_expression(node));

    BinaryExpression {
        operator,
        left: Box::new(left),
        right: Box::new(right),
        span: span_of(node),
    }
}

fn lower_binary_operator(text: &str) -> BinaryOperator {
    match text {
        "*" => BinaryOperator::Mul,
        "/" => BinaryOperator::Div,
        "%" => BinaryOperator::Mod,
        "+" => BinaryOperator::Add,
        "-" => BinaryOperator::Sub,
        "<" => BinaryOperator::Lt,
        "<=" => BinaryOperator::Le,
        ">" => BinaryOperator::Gt,
        ">=" => BinaryOperator::Ge,
        "==" => BinaryOperator::Eq,
        "!=" => BinaryOperator::Ne,
        "&&" => BinaryOperator::And,
        "||" => BinaryOperator::Or,
        other => unreachable!(
            "unexpected binary operator token {other:?} inside a successfully-parsed tree"
        ),
    }
}

fn lower_conditional_expression(node: Node, source: &str) -> ConditionalExpression {
    let condition = node
        .child_by_field_name("condition")
        .map(|n| lower_expression(n, source))
        .unwrap_or_else(|| missing_expression(node));

    let consequent = node
        .child_by_field_name("consequent")
        .map(|n| lower_expression(n, source))
        .unwrap_or_else(|| missing_expression(node));

    let alternate = node
        .child_by_field_name("alternate")
        .map(|n| lower_expression(n, source))
        .unwrap_or_else(|| missing_expression(node));

    ConditionalExpression {
        condition: Box::new(condition),
        consequent: Box::new(consequent),
        alternate: Box::new(alternate),
        span: span_of(node),
    }
}

fn lower_literal(node: Node, source: &str) -> Literal {
    let inner = node.child(0).unwrap_or(node);
    match inner.kind() {
        "string" => Literal::String(lower_string(inner, source)),
        "boolean_literal" => Literal::Boolean(BooleanLiteral {
            value: text_of(inner, source) == "true",
            span: span_of(inner),
        }),
        "null_literal" => Literal::Null(NullLiteral {
            span: span_of(inner),
        }),
        "number_literal" => Literal::Number(NumberLiteral {
            value: text_of(inner, source),
            span: span_of(inner),
        }),
        other => {
            unreachable!("unexpected literal node kind {other:?} inside a successfully-parsed tree")
        }
    }
}

fn lower_reference(node: Node, source: &str) -> Reference {
    Reference {
        name: text_of(node, source),
        span: span_of(node),
    }
}

fn lower_member_access(node: Node, source: &str) -> MemberAccess {
    let object = node
        .child_by_field_name("object")
        .map(|o| match o.kind() {
            "member_access" => Expression::MemberAccess(lower_member_access(o, source)),
            "reference" => Expression::Reference(lower_reference(o, source)),
            other => unreachable!(
                "unexpected member-access object kind {other:?} inside a successfully-parsed tree"
            ),
        })
        .unwrap_or_else(|| Expression::Reference(lower_reference(node, source)));

    let property = node
        .child_by_field_name("property")
        .map(|p| text_of(p, source))
        .unwrap_or_default();

    MemberAccess {
        object: Box::new(object),
        property,
        span: span_of(node),
    }
}

fn text_of(node: Node, source: &str) -> String {
    node.utf8_text(source.as_bytes())
        .unwrap_or_default()
        .to_string()
}

fn span_of(node: Node) -> Span {
    Span {
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
    }
}
