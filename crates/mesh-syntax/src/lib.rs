//! Syntax nodes, AST types, and source locations for MPRX.
//!
//! This crate owns the shape of the MPRX AST. It does not parse source text
//! (see `mesh-parser`) and does not perform semantic analysis (see
//! `mesh-semantic`).

/// A byte-offset range into the original source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start_byte: usize,
    pub end_byte: usize,
}

/// An MPRX element: `<name attr={...}>children</name>` or
/// `<name attr={...} />`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    pub name: String,
    pub attributes: Vec<Attribute>,
    pub children: Vec<Child>,
    pub span: Span,
}

/// A single `name=value` or `name={expression}` attribute on an [`Element`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    pub value: AttributeValue,
    pub span: Span,
}

/// A quoted string literal, with escape sequences already decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringLiteral {
    pub value: String,
    pub span: Span,
}

/// A run of literal text inside an element's children (not inside an
/// `{expression}` block).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text {
    pub value: String,
    pub span: Span,
}

/// A number literal. Stored as the raw source text (not parsed to `f64`)
/// so the AST stays a lossless representation of what was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberLiteral {
    pub value: String,
    pub span: Span,
}

/// A `true` or `false` literal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BooleanLiteral {
    pub value: bool,
    pub span: Span,
}

/// A `null` literal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NullLiteral {
    pub span: Span,
}

/// A literal value inside an `{expression}` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Literal {
    String(StringLiteral),
    Number(NumberLiteral),
    Boolean(BooleanLiteral),
    Null(NullLiteral),
}

/// A bare identifier reference, e.g. `{user}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub name: String,
    pub span: Span,
}

/// A property access on another expression, e.g. `{user.name}` or the
/// chained `{a.b.c}` (whose `object` is itself a `MemberAccess`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberAccess {
    pub object: Box<Expression>,
    pub property: String,
    pub span: Span,
}

/// An expression inside an `{...}` block: a literal, a reference, or a
/// member access. Pass 3 will extend this with unary/binary/conditional/
/// array/object/command/event-value variants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    Literal(Literal),
    Reference(Reference),
    MemberAccess(MemberAccess),
}

/// The value side of an [`Attribute`]: either a plain quoted string or an
/// `{expression}` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeValue {
    String(StringLiteral),
    Expression(Expression),
}

/// One child of an [`Element`]: either literal [`Text`] or an
/// `{expression}` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Child {
    Text(Text),
    Expression(Expression),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_a_self_closing_element() {
        let element = Element {
            name: "page".to_string(),
            attributes: vec![Attribute {
                name: "title".to_string(),
                value: AttributeValue::String(StringLiteral {
                    value: "Users".to_string(),
                    span: Span {
                        start_byte: 12,
                        end_byte: 19,
                    },
                }),
                span: Span {
                    start_byte: 6,
                    end_byte: 19,
                },
            }],
            children: vec![],
            span: Span {
                start_byte: 0,
                end_byte: 22,
            },
        };

        assert_eq!(element.name, "page");
        assert_eq!(element.attributes[0].name, "title");
        match &element.attributes[0].value {
            AttributeValue::String(literal) => assert_eq!(literal.value, "Users"),
            other => panic!("expected a string attribute value, got {other:?}"),
        }
    }

    #[test]
    fn constructs_an_element_with_an_expression_attribute() {
        let element = Element {
            name: "page".to_string(),
            attributes: vec![Attribute {
                name: "title".to_string(),
                value: AttributeValue::Expression(Expression::MemberAccess(MemberAccess {
                    object: Box::new(Expression::Reference(Reference {
                        name: "user".to_string(),
                        span: Span {
                            start_byte: 0,
                            end_byte: 4,
                        },
                    })),
                    property: "name".to_string(),
                    span: Span {
                        start_byte: 0,
                        end_byte: 9,
                    },
                })),
                span: Span {
                    start_byte: 0,
                    end_byte: 9,
                },
            }],
            children: vec![],
            span: Span {
                start_byte: 0,
                end_byte: 10,
            },
        };

        match &element.attributes[0].value {
            AttributeValue::Expression(Expression::MemberAccess(member)) => {
                assert_eq!(member.property, "name");
                match member.object.as_ref() {
                    Expression::Reference(reference) => assert_eq!(reference.name, "user"),
                    other => panic!("expected a reference, got {other:?}"),
                }
            }
            other => panic!("expected a member access expression, got {other:?}"),
        }
    }

    #[test]
    fn constructs_an_element_with_an_expression_child() {
        let element = Element {
            name: "title".to_string(),
            attributes: vec![],
            children: vec![Child::Expression(Expression::Reference(Reference {
                name: "user".to_string(),
                span: Span {
                    start_byte: 7,
                    end_byte: 11,
                },
            }))],
            span: Span {
                start_byte: 0,
                end_byte: 20,
            },
        };

        match &element.children[0] {
            Child::Expression(Expression::Reference(reference)) => {
                assert_eq!(reference.name, "user");
            }
            other => panic!("expected an expression child, got {other:?}"),
        }
    }
}
