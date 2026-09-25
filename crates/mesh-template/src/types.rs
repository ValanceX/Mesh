//! The template's types, one for each object in `template-v1`.
//!
//! They derive `serde` for the crate's own reader and writer; the JSON
//! they correspond to is the schema's, and `serde_json` never appears in
//! the public API.

use crate::Fingerprint;
use serde::{Deserialize, Serialize};

/// One compiled component: `template-v1` without its `format` and
/// `version`, which [`crate::from_json`] checks and [`crate::to_json`]
/// writes.
#[derive(Debug, Clone, PartialEq)]
pub struct Template {
    /// The component whose template this is.
    pub component: String,
    /// The fingerprint of the model the template was checked against.
    pub fingerprint: Fingerprint,
    /// The compiler version that produced it: provenance only.
    pub compiler: String,
    /// The root element.
    pub root: Element,
}

/// An occurrence: an instance of `component`. Whether it is a composite
/// or a primitive is decided by the program, never here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Element {
    pub component: String,
    /// The kept attributes, in source order.
    pub props: Vec<Prop>,
    /// The kept event bindings, in source order.
    pub events: Vec<EventBinding>,
    /// The children, in order; no whitespace-only text.
    pub children: Vec<Child>,
    pub span: Span,
}

/// A prop of the element's component, and its value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Prop {
    pub prop: String,
    pub value: Expression,
    pub span: Span,
}

/// An event of the element's component, and its handler: a command of
/// the template's own component, with its arguments. `$event` may appear
/// only in `arguments`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventBinding {
    pub event: String,
    pub command: String,
    pub arguments: Vec<Expression>,
    pub span: Span,
}

/// A child of an element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Child {
    /// Literal text, as the semantic model keeps it.
    Text { value: String, span: Span },
    /// An interpolation.
    Expression { expression: Expression },
    /// A child element.
    Element { element: Box<Element> },
}

/// An expression. Every one has a span.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Expression {
    /// A literal, or a quoted attribute value.
    Literal { value: Literal, span: Span },
    /// A reference to a scope declaration of the template's component.
    Scope { name: String, span: Span },
    /// `object.field`.
    Member {
        object: Box<Expression>,
        field: String,
        span: Span,
    },
    Unary {
        operator: UnaryOperator,
        operand: Box<Expression>,
        span: Span,
    },
    Binary {
        operator: BinaryOperator,
        left: Box<Expression>,
        right: Box<Expression>,
        span: Span,
    },
    /// `condition ? consequent : alternate`.
    Conditional {
        condition: Box<Expression>,
        consequent: Box<Expression>,
        alternate: Box<Expression>,
        span: Span,
    },
    /// A list literal.
    List {
        elements: Vec<Expression>,
        span: Span,
    },
    /// A record literal; its field names are unique.
    Record { fields: Vec<Field>, span: Span },
    /// `$event`.
    Event { span: Span },
}

impl Expression {
    /// The span of the whole expression.
    pub fn span(&self) -> Span {
        match self {
            Expression::Literal { span, .. }
            | Expression::Scope { span, .. }
            | Expression::Member { span, .. }
            | Expression::Unary { span, .. }
            | Expression::Binary { span, .. }
            | Expression::Conditional { span, .. }
            | Expression::List { span, .. }
            | Expression::Record { span, .. }
            | Expression::Event { span } => *span,
        }
    }
}

/// One field of a record literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub value: Expression,
    pub span: Span,
}

/// A literal's value. A number is the binary64 value the compiler
/// converted the literal to: finite, and never `-0`.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    String(String),
    Number(f64),
    Boolean(bool),
    Null,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UnaryOperator {
    /// `!`
    Not,
    /// `-`
    Negate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
}

/// Where a construct was in the template's source. Nothing depends on it
/// for meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start: Offset,
    pub end: Offset,
}

/// A place in the source: `byte` counts UTF-8 bytes, a byte-order mark
/// included, and `utf16` counts UTF-16 code units of the same text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Offset {
    pub byte: usize,
    pub utf16: usize,
}

impl Serialize for Literal {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Literal::String(value) => serializer.serialize_str(value),
            Literal::Boolean(value) => serializer.serialize_bool(*value),
            Literal::Null => serializer.serialize_unit(),
            // An integral value whose integer is exact in binary64 is
            // written as an integer (`2`, not `2.0`). It reads back as the
            // same value either way.
            Literal::Number(value)
                if value.fract() == 0.0 && value.abs() < 9_007_199_254_740_992.0 =>
            {
                #[allow(clippy::cast_possible_truncation)]
                serializer.serialize_i64(*value as i64)
            }
            Literal::Number(value) => serializer.serialize_f64(*value),
        }
    }
}

impl<'de> Deserialize<'de> for Literal {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = Literal;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a string, a number, a boolean or null")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Literal, E> {
                Ok(Literal::String(value.to_string()))
            }
            fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Literal, E> {
                Ok(Literal::String(value))
            }
            fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Literal, E> {
                Ok(Literal::Boolean(value))
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Literal, E> {
                Ok(Literal::Null)
            }
            fn visit_none<E: serde::de::Error>(self) -> Result<Literal, E> {
                Ok(Literal::Null)
            }
            fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Literal, E> {
                Ok(Literal::Number(value))
            }
            // `as` rounds to the nearest binary64 value, ties to even.
            #[allow(clippy::cast_precision_loss)]
            fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Literal, E> {
                Ok(Literal::Number(value as f64))
            }
            #[allow(clippy::cast_precision_loss)]
            fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Literal, E> {
                Ok(Literal::Number(value as f64))
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}
