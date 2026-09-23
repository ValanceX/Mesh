//! Syntax nodes, AST types, and source locations for MPRX.
//!
//! This crate owns the shape of the MPRX AST. It does not parse source text
//! (see `mesh-parser`) and does not perform semantic analysis (see
//! `mesh-semantic`).

use std::fmt;

/// A byte-offset range into the original source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start_byte: usize,
    pub end_byte: usize,
}

/// How serious a [`Diagnostic`] is.
///
/// Only `Error` exists for v0.1 — a compile either fully succeeds or
/// produces exactly one fatal error. Marked `#[non_exhaustive]` because
/// future passes are expected to add more variants (e.g. `Warning`), and
/// that should not be a breaking change for consumers.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
}

/// A single compile-time diagnostic: a message, its severity, and the
/// source [`Span`] it applies to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Span,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}..{})",
            self.message, self.span.start_byte, self.span.end_byte
        )
    }
}

/// An MPRX element: `<name attr={...}>children</name>` or
/// `<name attr={...} />`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    pub name: String,
    /// The close tag's text, verbatim as written in source. `None` for a
    /// self-closing element (no close tag exists); `Some(..)` for a
    /// container element. AST-only — the Semantic IR does not retain
    /// this field; it exists solely so `mesh-semantic`'s tag-mismatch
    /// check has the close tag's text to compare against `name`.
    pub closing_name: Option<String>,
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
/// so the AST stays a lossless representation of what was written. Always
/// unsigned as of Pass 3a — a negative numeric literal like `-3.5` lowers
/// to a [`UnaryExpression`] (`Negate`) wrapping this, not a signed token;
/// see `docs/MPRX-SPEC.md` §2.
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

/// A prefix operator applicable to a [`UnaryExpression`]: `!` (logical
/// negation) or `-` (numeric negation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    Not,
    Negate,
}

/// A unary expression: a prefix operator applied to its operand, e.g.
/// `{!disabled}` or `{-count}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnaryExpression {
    pub operator: UnaryOperator,
    pub operand: Box<Expression>,
    pub span: Span,
}

/// An infix operator applicable to a [`BinaryExpression`], covering
/// `docs/MPRX-SPEC.md` §5's multiplicative, additive, relational,
/// equality, and logical operator tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    Mul,
    Div,
    Mod,
    Add,
    Sub,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    And,
    Or,
}

/// A binary expression: an infix operator applied to a left and right
/// operand, e.g. `{a + b}` or `{count > 0}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryExpression {
    pub operator: BinaryOperator,
    pub left: Box<Expression>,
    pub right: Box<Expression>,
    pub span: Span,
}

/// A ternary conditional expression, e.g. `{compact ? "sm" : "md"}`.
/// Right-associative: `a ? b : c ? d : e` parses as `a ? b : (c ? d : e)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionalExpression {
    pub condition: Box<Expression>,
    pub consequent: Box<Expression>,
    pub alternate: Box<Expression>,
    pub span: Span,
}

/// An array literal expression, e.g. `{[1, 2, 3]}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayExpression {
    pub elements: Vec<Expression>,
    pub span: Span,
}

/// The key side of an [`ObjectMember`]: either a bare identifier or a
/// quoted string, e.g. `name: 1` vs `"a-b": 1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectKey {
    Identifier(String),
    String(StringLiteral),
}

/// One `key: value` entry inside an [`ObjectExpression`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectMember {
    pub key: ObjectKey,
    pub value: Expression,
    pub span: Span,
}

/// An object literal expression, e.g. `{{ name: user.name, active: true }}`
/// — doubled braces, since the outer pair is the `{...}` expression-block
/// wrapper and the inner pair is the object literal itself; see
/// `docs/MPRX-SPEC.md` §5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectExpression {
    pub members: Vec<ObjectMember>,
    pub span: Span,
}

/// A command invocation, representing intent rather than execution — see
/// `docs/MPRX-SPEC.md` §6. e.g. `selectUser($event)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandInvocation {
    pub command: String,
    pub arguments: Vec<Expression>,
    pub span: Span,
}

/// A `$`-prefixed special value, e.g. `$event`. The lexical rule is
/// general (`'$' identifier`), but v0.1 semantics only meaningfully
/// understand `$event` — structural parsing only, no validation of the
/// name; see `docs/MPRX-SPEC.md` §2 and §5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventValue {
    pub name: String,
    pub span: Span,
}

/// An expression inside an `{...}` block. This is §5's complete v0.1
/// expression grammar — every node kind the spec defines has a variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    Literal(Literal),
    Reference(Reference),
    MemberAccess(MemberAccess),
    Unary(UnaryExpression),
    Binary(BinaryExpression),
    Conditional(ConditionalExpression),
    Array(ArrayExpression),
    Object(ObjectExpression),
    Command(CommandInvocation),
    EventValue(EventValue),
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
            closing_name: None,
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
            closing_name: None,
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
            closing_name: None,
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

    #[test]
    fn constructs_a_unary_expression() {
        let expression = Expression::Unary(UnaryExpression {
            operator: UnaryOperator::Negate,
            operand: Box::new(Expression::Reference(Reference {
                name: "count".to_string(),
                span: Span {
                    start_byte: 1,
                    end_byte: 6,
                },
            })),
            span: Span {
                start_byte: 0,
                end_byte: 6,
            },
        });

        match expression {
            Expression::Unary(unary) => {
                assert_eq!(unary.operator, UnaryOperator::Negate);
                match unary.operand.as_ref() {
                    Expression::Reference(reference) => assert_eq!(reference.name, "count"),
                    other => panic!("expected operand to be a reference, got {other:?}"),
                }
            }
            other => panic!("expected a unary expression, got {other:?}"),
        }
    }

    #[test]
    fn constructs_a_binary_expression() {
        let expression = Expression::Binary(BinaryExpression {
            operator: BinaryOperator::Add,
            left: Box::new(Expression::Reference(Reference {
                name: "a".to_string(),
                span: Span {
                    start_byte: 0,
                    end_byte: 1,
                },
            })),
            right: Box::new(Expression::Reference(Reference {
                name: "b".to_string(),
                span: Span {
                    start_byte: 4,
                    end_byte: 5,
                },
            })),
            span: Span {
                start_byte: 0,
                end_byte: 5,
            },
        });

        match expression {
            Expression::Binary(binary) => {
                assert_eq!(binary.operator, BinaryOperator::Add);
                match (binary.left.as_ref(), binary.right.as_ref()) {
                    (Expression::Reference(left), Expression::Reference(right)) => {
                        assert_eq!(left.name, "a");
                        assert_eq!(right.name, "b");
                    }
                    other => panic!("expected both operands to be references, got {other:?}"),
                }
            }
            other => panic!("expected a binary expression, got {other:?}"),
        }
    }

    #[test]
    fn constructs_a_conditional_expression() {
        let expression = Expression::Conditional(ConditionalExpression {
            condition: Box::new(Expression::Reference(Reference {
                name: "compact".to_string(),
                span: Span {
                    start_byte: 0,
                    end_byte: 7,
                },
            })),
            consequent: Box::new(Expression::Literal(Literal::String(StringLiteral {
                value: "sm".to_string(),
                span: Span {
                    start_byte: 10,
                    end_byte: 14,
                },
            }))),
            alternate: Box::new(Expression::Literal(Literal::String(StringLiteral {
                value: "md".to_string(),
                span: Span {
                    start_byte: 17,
                    end_byte: 21,
                },
            }))),
            span: Span {
                start_byte: 0,
                end_byte: 21,
            },
        });

        match expression {
            Expression::Conditional(conditional) => match conditional.consequent.as_ref() {
                Expression::Literal(Literal::String(s)) => assert_eq!(s.value, "sm"),
                other => panic!("expected consequent to be a string literal, got {other:?}"),
            },
            other => panic!("expected a conditional expression, got {other:?}"),
        }
    }

    #[test]
    fn constructs_an_array_expression() {
        let expression = Expression::Array(ArrayExpression {
            elements: vec![
                Expression::Reference(Reference {
                    name: "a".to_string(),
                    span: Span {
                        start_byte: 1,
                        end_byte: 2,
                    },
                }),
                Expression::Reference(Reference {
                    name: "b".to_string(),
                    span: Span {
                        start_byte: 4,
                        end_byte: 5,
                    },
                }),
            ],
            span: Span {
                start_byte: 0,
                end_byte: 6,
            },
        });

        match expression {
            Expression::Array(array) => {
                assert_eq!(array.elements.len(), 2);
                match &array.elements[0] {
                    Expression::Reference(reference) => assert_eq!(reference.name, "a"),
                    other => panic!("expected first element to be a reference, got {other:?}"),
                }
            }
            other => panic!("expected an array expression, got {other:?}"),
        }
    }

    #[test]
    fn constructs_an_object_expression_with_an_identifier_key() {
        let expression = Expression::Object(ObjectExpression {
            members: vec![ObjectMember {
                key: ObjectKey::Identifier("name".to_string()),
                value: Expression::Literal(Literal::String(StringLiteral {
                    value: "Users".to_string(),
                    span: Span {
                        start_byte: 6,
                        end_byte: 13,
                    },
                })),
                span: Span {
                    start_byte: 0,
                    end_byte: 13,
                },
            }],
            span: Span {
                start_byte: 0,
                end_byte: 13,
            },
        });

        match expression {
            Expression::Object(object) => {
                assert_eq!(object.members.len(), 1);
                match &object.members[0].key {
                    ObjectKey::Identifier(name) => assert_eq!(name, "name"),
                    other => panic!("expected an identifier key, got {other:?}"),
                }
            }
            other => panic!("expected an object expression, got {other:?}"),
        }
    }

    #[test]
    fn constructs_an_object_expression_with_a_string_key() {
        let member = ObjectMember {
            key: ObjectKey::String(StringLiteral {
                value: "a-b".to_string(),
                span: Span {
                    start_byte: 0,
                    end_byte: 5,
                },
            }),
            value: Expression::Literal(Literal::Number(NumberLiteral {
                value: "1".to_string(),
                span: Span {
                    start_byte: 7,
                    end_byte: 8,
                },
            })),
            span: Span {
                start_byte: 0,
                end_byte: 8,
            },
        };

        match member.key {
            ObjectKey::String(s) => assert_eq!(s.value, "a-b"),
            other => panic!("expected a string key, got {other:?}"),
        }
    }

    #[test]
    fn constructs_a_command_invocation() {
        let expression = Expression::Command(CommandInvocation {
            command: "selectUser".to_string(),
            arguments: vec![Expression::EventValue(EventValue {
                name: "event".to_string(),
                span: Span {
                    start_byte: 11,
                    end_byte: 17,
                },
            })],
            span: Span {
                start_byte: 0,
                end_byte: 18,
            },
        });

        match expression {
            Expression::Command(command) => {
                assert_eq!(command.command, "selectUser");
                assert_eq!(command.arguments.len(), 1);
            }
            other => panic!("expected a command invocation, got {other:?}"),
        }
    }

    #[test]
    fn constructs_an_event_value() {
        let expression = Expression::EventValue(EventValue {
            name: "event".to_string(),
            span: Span {
                start_byte: 0,
                end_byte: 6,
            },
        });

        match expression {
            Expression::EventValue(event) => assert_eq!(event.name, "event"),
            other => panic!("expected an event value, got {other:?}"),
        }
    }
}
