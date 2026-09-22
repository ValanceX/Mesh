//! Semantic model for MPRX.
//!
//! For v0.1, this crate builds the Semantic IR from the `mesh-syntax` AST
//! and performs structural pass-through only. It does not yet resolve
//! component/prop/binding references against an external component model —
//! that requires a typed component model that doesn't exist yet.

/// The Semantic IR form of an [`mesh_syntax::Element`] — structurally
/// identical to the AST for v0.1, since no resolution happens yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    pub name: String,
    pub attributes: Vec<Attribute>,
    pub children: Vec<Child>,
}

/// The Semantic IR form of an [`mesh_syntax::Attribute`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    pub value: AttributeValue,
}

/// The Semantic IR form of an [`mesh_syntax::AttributeValue`]. Unlike the
/// AST's `String(StringLiteral)`, the string case here is a plain `String`
/// — source spans are AST-only and don't carry into the IR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeValue {
    String(String),
    Expression(Expression),
}

/// The Semantic IR form of an [`mesh_syntax::Child`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Child {
    Text(String),
    Expression(Expression),
}

/// The Semantic IR form of an [`mesh_syntax::Literal`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Literal {
    String(String),
    Number(String),
    Boolean(bool),
    Null,
}

/// The Semantic IR form of an [`mesh_syntax::Expression`]. `Unary` and
/// `Binary` reuse [`mesh_syntax::UnaryOperator`]/[`mesh_syntax::BinaryOperator`]
/// directly rather than redeclaring an IR-local copy — those enums carry
/// no [`mesh_syntax::Span`] to strip, unlike every other AST type mirrored
/// here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    Literal(Literal),
    Reference(String),
    MemberAccess {
        object: Box<Expression>,
        property: String,
    },
    Unary {
        operator: mesh_syntax::UnaryOperator,
        operand: Box<Expression>,
    },
    Binary {
        operator: mesh_syntax::BinaryOperator,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Conditional {
        condition: Box<Expression>,
        consequent: Box<Expression>,
        alternate: Box<Expression>,
    },
}

/// Lowers an AST [`mesh_syntax::Element`] into its Semantic IR form.
///
/// For v0.1 this is a structural pass-through (drops source spans, copies
/// everything else) — it does not yet resolve references against a
/// component model.
pub fn lower(ast: &mesh_syntax::Element) -> Element {
    Element {
        name: ast.name.clone(),
        attributes: ast.attributes.iter().map(lower_attribute).collect(),
        children: ast.children.iter().map(lower_child).collect(),
    }
}

fn lower_attribute(attribute: &mesh_syntax::Attribute) -> Attribute {
    Attribute {
        name: attribute.name.clone(),
        value: lower_attribute_value(&attribute.value),
    }
}

fn lower_attribute_value(value: &mesh_syntax::AttributeValue) -> AttributeValue {
    match value {
        mesh_syntax::AttributeValue::String(literal) => {
            AttributeValue::String(literal.value.clone())
        }
        mesh_syntax::AttributeValue::Expression(expression) => {
            AttributeValue::Expression(lower_expression(expression))
        }
    }
}

fn lower_child(child: &mesh_syntax::Child) -> Child {
    match child {
        mesh_syntax::Child::Text(text) => Child::Text(text.value.clone()),
        mesh_syntax::Child::Expression(expression) => {
            Child::Expression(lower_expression(expression))
        }
    }
}

fn lower_expression(expression: &mesh_syntax::Expression) -> Expression {
    match expression {
        mesh_syntax::Expression::Literal(literal) => Expression::Literal(lower_literal(literal)),
        mesh_syntax::Expression::Reference(reference) => {
            Expression::Reference(reference.name.clone())
        }
        mesh_syntax::Expression::MemberAccess(member) => Expression::MemberAccess {
            object: Box::new(lower_expression(&member.object)),
            property: member.property.clone(),
        },
        mesh_syntax::Expression::Unary(unary) => Expression::Unary {
            operator: unary.operator,
            operand: Box::new(lower_expression(&unary.operand)),
        },
        mesh_syntax::Expression::Binary(binary) => Expression::Binary {
            operator: binary.operator,
            left: Box::new(lower_expression(&binary.left)),
            right: Box::new(lower_expression(&binary.right)),
        },
        mesh_syntax::Expression::Conditional(conditional) => Expression::Conditional {
            condition: Box::new(lower_expression(&conditional.condition)),
            consequent: Box::new(lower_expression(&conditional.consequent)),
            alternate: Box::new(lower_expression(&conditional.alternate)),
        },
    }
}

fn lower_literal(literal: &mesh_syntax::Literal) -> Literal {
    match literal {
        mesh_syntax::Literal::String(s) => Literal::String(s.value.clone()),
        mesh_syntax::Literal::Number(n) => Literal::Number(n.value.clone()),
        mesh_syntax::Literal::Boolean(b) => Literal::Boolean(b.value),
        mesh_syntax::Literal::Null(_) => Literal::Null,
    }
}
