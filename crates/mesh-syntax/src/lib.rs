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
/// For v0.1, `Error` and `Warning` both describe how serious an issue is,
/// not whether it blocks lowering — check `ir.is_some()` on the
/// `LowerResult`/`ParseResult` to see whether IR was produced, regardless
/// of severity (e.g. a mismatched closing tag is an `Error` but still
/// produces IR).
/// A compile can produce multiple diagnostics with either severity
/// (see `ParseResult`/`LowerResult`). Marked `#[non_exhaustive]` because
/// future passes are expected to add more variants, and that should not
/// be a breaking change for consumers.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// The lowercase label renderers print before a diagnostic's message
/// (`error`, `warning`). Lives here, next to the enum, so the match stays
/// exhaustive — downstream crates can't match `#[non_exhaustive]`
/// `Severity` without a wildcard arm.
impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        })
    }
}

/// A diagnostic's stable, machine-readable code, such as
/// `mismatched-closing-tag`.
///
/// Codes are API, under a fixed stability policy: they are kebab-case,
/// a code is never renamed, and a retired code is never reused for a
/// different meaning. Messages may change freely; codes may not.
/// `syntax-error` is the permanent fallback for a syntax problem MESH
/// can't classify more precisely. The field is private, so every code is
/// one of the associated constants below, and [`DiagnosticCode::ALL`]
/// lists them all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DiagnosticCode(&'static str);

impl DiagnosticCode {
    /// A syntax problem with no more specific code. Permanent fallback.
    pub const SYNTAX_ERROR: DiagnosticCode = DiagnosticCode("syntax-error");
    /// A closing tag names a different element than its opening tag.
    pub const MISMATCHED_CLOSING_TAG: DiagnosticCode = DiagnosticCode("mismatched-closing-tag");
    /// An element repeats an attribute; the last occurrence wins.
    pub const DUPLICATE_ATTRIBUTE: DiagnosticCode = DiagnosticCode("duplicate-attribute");
    /// An element repeats an event binding; the last occurrence wins.
    pub const DUPLICATE_EVENT_BINDING: DiagnosticCode = DiagnosticCode("duplicate-event-binding");
    /// A tag was opened (`<page`) but never finished with `>` or `/>`.
    pub const UNTERMINATED_TAG: DiagnosticCode = DiagnosticCode("unterminated-tag");
    /// A container element's opening tag has no matching closing tag.
    pub const MISSING_CLOSING_TAG: DiagnosticCode = DiagnosticCode("missing-closing-tag");
    /// A literal `<` in text, which MPRX reads as the start of a tag.
    pub const LESS_THAN_IN_TEXT: DiagnosticCode = DiagnosticCode("less-than-in-text");
    /// An attribute name containing `-`, such as `data-id`.
    pub const HYPHENATED_ATTRIBUTE_NAME: DiagnosticCode =
        DiagnosticCode("hyphenated-attribute-name");
    /// An object literal written with one pair of braces: `data={ k: 1 }`.
    pub const SINGLE_BRACE_OBJECT: DiagnosticCode = DiagnosticCode("single-brace-object");
    /// A trailing comma in command arguments: `save(a, b,)`.
    pub const COMMAND_TRAILING_COMMA: DiagnosticCode = DiagnosticCode("command-trailing-comma");
    /// An `on.` event binding with a missing, dotted, or hyphenated event
    /// name, or a value that isn't `{...}`.
    pub const MALFORMED_EVENT_BINDING: DiagnosticCode = DiagnosticCode("malformed-event-binding");
    /// A component manifest that isn't valid JSON.
    pub const MANIFEST_SYNTAX_ERROR: DiagnosticCode = DiagnosticCode("manifest-syntax-error");
    /// A manifest whose `version` is missing or isn't one MESH reads.
    pub const MANIFEST_UNSUPPORTED_VERSION: DiagnosticCode =
        DiagnosticCode("manifest-unsupported-version");
    /// A manifest value of the wrong JSON type, such as `"required": "yes"`.
    pub const MANIFEST_INVALID_VALUE: DiagnosticCode = DiagnosticCode("manifest-invalid-value");
    /// A manifest object without a property it must have.
    pub const MANIFEST_MISSING_PROPERTY: DiagnosticCode =
        DiagnosticCode("manifest-missing-property");
    /// A manifest object with a property its schema doesn't allow.
    pub const MANIFEST_UNKNOWN_PROPERTY: DiagnosticCode =
        DiagnosticCode("manifest-unknown-property");
    /// A key repeated in one manifest object.
    pub const MANIFEST_DUPLICATE_KEY: DiagnosticCode = DiagnosticCode("manifest-duplicate-key");
    /// A type whose `kind` isn't one a manifest can write.
    pub const MANIFEST_UNKNOWN_KIND: DiagnosticCode = DiagnosticCode("manifest-unknown-kind");
    /// A declared name that MPRX can't write in its position.
    pub const MANIFEST_INVALID_NAME: DiagnosticCode = DiagnosticCode("manifest-invalid-name");
    /// A command that declares two parameters with the same name.
    pub const MANIFEST_DUPLICATE_PARAMETER: DiagnosticCode =
        DiagnosticCode("manifest-duplicate-parameter");
    /// A `named` type that refers to a type the manifest doesn't declare.
    pub const MANIFEST_UNKNOWN_TYPE: DiagnosticCode = DiagnosticCode("manifest-unknown-type");
    /// Named types that refer to themselves, directly or indirectly.
    pub const MANIFEST_RECURSIVE_TYPE: DiagnosticCode = DiagnosticCode("manifest-recursive-type");
    /// An `optional` type wrapping a type that is already optional.
    pub const MANIFEST_NESTED_OPTIONAL: DiagnosticCode = DiagnosticCode("manifest-nested-optional");
    /// The manifest has no component for the file being checked.
    pub const MANIFEST_MISSING_COMPONENT: DiagnosticCode =
        DiagnosticCode("manifest-missing-component");

    /// Every code MESH can emit, in catalogue order. The diagnostics
    /// reference (`docs/manual/diagnostics.md`) documents each one, and a
    /// test keeps the two in step.
    pub const ALL: &'static [DiagnosticCode] = &[
        DiagnosticCode::SYNTAX_ERROR,
        DiagnosticCode::MISMATCHED_CLOSING_TAG,
        DiagnosticCode::DUPLICATE_ATTRIBUTE,
        DiagnosticCode::DUPLICATE_EVENT_BINDING,
        DiagnosticCode::UNTERMINATED_TAG,
        DiagnosticCode::MISSING_CLOSING_TAG,
        DiagnosticCode::LESS_THAN_IN_TEXT,
        DiagnosticCode::HYPHENATED_ATTRIBUTE_NAME,
        DiagnosticCode::SINGLE_BRACE_OBJECT,
        DiagnosticCode::COMMAND_TRAILING_COMMA,
        DiagnosticCode::MALFORMED_EVENT_BINDING,
        DiagnosticCode::MANIFEST_SYNTAX_ERROR,
        DiagnosticCode::MANIFEST_UNSUPPORTED_VERSION,
        DiagnosticCode::MANIFEST_INVALID_VALUE,
        DiagnosticCode::MANIFEST_MISSING_PROPERTY,
        DiagnosticCode::MANIFEST_UNKNOWN_PROPERTY,
        DiagnosticCode::MANIFEST_DUPLICATE_KEY,
        DiagnosticCode::MANIFEST_UNKNOWN_KIND,
        DiagnosticCode::MANIFEST_INVALID_NAME,
        DiagnosticCode::MANIFEST_DUPLICATE_PARAMETER,
        DiagnosticCode::MANIFEST_UNKNOWN_TYPE,
        DiagnosticCode::MANIFEST_RECURSIVE_TYPE,
        DiagnosticCode::MANIFEST_NESTED_OPTIONAL,
        DiagnosticCode::MANIFEST_MISSING_COMPONENT,
    ];

    /// The code as a string, e.g. `"mismatched-closing-tag"`.
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// A single compile-time diagnostic: its severity, its stable
/// [`DiagnosticCode`], a human-readable message, and the source [`Span`]
/// it applies to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: DiagnosticCode,
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
    /// The opening tag's name, e.g. `user-card` in `<user-card ...>`.
    pub name_span: Span,
    /// The close tag's text, verbatim as written in source. `None` for a
    /// self-closing element (no close tag exists); `Some(..)` for a
    /// container element. AST-only — the Semantic IR does not retain
    /// this field; it exists solely so `mesh-semantic`'s tag-mismatch
    /// check has the close tag's text to compare against `name`.
    pub closing_name: Option<String>,
    pub attributes: Vec<Attribute>,
    pub event_bindings: Vec<EventBinding>,
    pub children: Vec<Child>,
    pub span: Span,
}

/// A single `name=value` or `name={expression}` attribute on an [`Element`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    /// The attribute's name, without `=` or the value.
    pub name_span: Span,
    pub value: AttributeValue,
    pub span: Span,
}

/// A single `on.name={handler}` event binding on an [`Element`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventBinding {
    pub name: String,
    /// The event name after `on.`, e.g. `click` in `on.click={...}`.
    pub name_span: Span,
    pub handler: Expression,
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
    /// The property name after the `.`.
    pub property_span: Span,
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
    /// The key as written: the bare identifier, or the quoted string
    /// including its quotes.
    pub key_span: Span,
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
    /// The command's name, without the parentheses or arguments.
    pub command_span: Span,
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

/// One child of an [`Element`]: literal [`Text`], an `{expression}`
/// block, or a nested [`Element`]. `Box` is required for `Element` —
/// `Element` contains `Vec<Child>`, so an unboxed variant would make
/// `Child` infinitely-sized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Child {
    Text(Text),
    Expression(Expression),
    Element(Box<Element>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_a_self_closing_element() {
        let element = Element {
            name: "page".to_string(),
            closing_name: None,
            name_span: Span {
                start_byte: 1,
                end_byte: 5,
            },
            attributes: vec![Attribute {
                name: "title".to_string(),
                name_span: Span {
                    start_byte: 6,
                    end_byte: 11,
                },
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
            event_bindings: vec![],
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
            name_span: Span {
                start_byte: 1,
                end_byte: 5,
            },
            attributes: vec![Attribute {
                name: "title".to_string(),
                name_span: Span {
                    start_byte: 6,
                    end_byte: 11,
                },
                value: AttributeValue::Expression(Expression::MemberAccess(MemberAccess {
                    object: Box::new(Expression::Reference(Reference {
                        name: "user".to_string(),
                        span: Span {
                            start_byte: 0,
                            end_byte: 4,
                        },
                    })),
                    property: "name".to_string(),
                    property_span: Span {
                        start_byte: 5,
                        end_byte: 9,
                    },
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
            event_bindings: vec![],
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
            name_span: Span {
                start_byte: 1,
                end_byte: 5,
            },
            attributes: vec![],
            event_bindings: vec![],
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
                key_span: Span {
                    start_byte: 0,
                    end_byte: 4,
                },
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
            key_span: Span {
                start_byte: 0,
                end_byte: 5,
            },
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
            command_span: Span {
                start_byte: 0,
                end_byte: 10,
            },
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

    #[test]
    fn severity_displays_as_its_lowercase_label() {
        assert_eq!(Severity::Error.to_string(), "error");
        assert_eq!(Severity::Warning.to_string(), "warning");
    }

    #[test]
    fn diagnostic_code_displays_as_its_string() {
        let code = DiagnosticCode::MISMATCHED_CLOSING_TAG;
        assert_eq!(code.to_string(), "mismatched-closing-tag");
        assert_eq!(code.as_str(), "mismatched-closing-tag");
    }

    #[test]
    fn diagnostic_codes_are_unique_kebab_case() {
        let mut seen = std::collections::HashSet::new();
        for code in DiagnosticCode::ALL {
            let code = code.as_str();
            let is_kebab_case = !code.is_empty()
                && !code.starts_with('-')
                && !code.ends_with('-')
                && !code.contains("--")
                && code
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
            assert!(is_kebab_case, "{code:?} is not kebab-case");
            assert!(seen.insert(code), "{code:?} is listed twice");
        }
    }

    /// Codes are never renamed or removed. If this fails because a code
    /// was added, append it here; if it fails for any other reason, the
    /// change breaks the stability policy.
    #[test]
    fn diagnostic_codes_never_change() {
        let codes: Vec<&str> = DiagnosticCode::ALL.iter().map(|c| c.as_str()).collect();
        assert_eq!(
            codes,
            [
                "syntax-error",
                "mismatched-closing-tag",
                "duplicate-attribute",
                "duplicate-event-binding",
                "unterminated-tag",
                "missing-closing-tag",
                "less-than-in-text",
                "hyphenated-attribute-name",
                "single-brace-object",
                "command-trailing-comma",
                "malformed-event-binding",
                "manifest-syntax-error",
                "manifest-unsupported-version",
                "manifest-invalid-value",
                "manifest-missing-property",
                "manifest-unknown-property",
                "manifest-duplicate-key",
                "manifest-unknown-kind",
                "manifest-invalid-name",
                "manifest-duplicate-parameter",
                "manifest-unknown-type",
                "manifest-recursive-type",
                "manifest-nested-optional",
                "manifest-missing-component",
            ]
        );
    }
}
