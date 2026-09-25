//! Emitting a template (v0.5 D1): the Semantic IR of a source that checked
//! clean, written as `template-v1`.
//!
//! A clean check means every name resolved to the declaration it names,
//! so a name is emitted as written: an element's tag is its component, an
//! attribute its prop, a reference its scope declaration, a command the
//! template's own component's. The template holds no values, and nothing
//! here evaluates anything: number literals are converted once, to the
//! nearest binary64 value (§9.7.3), and spans gain their UTF-16 offsets.

use crate::source_map::SourceMap;
use mesh_semantic::{AttributeValue, Child, Element, Expression, Literal};
use mesh_syntax::{BinaryOperator, Span, UnaryOperator};
use mesh_template as t;

/// The template of `component` whose source, `source`, lowered to `ir`
/// and checked clean against the model whose fingerprint is
/// `fingerprint`.
pub(crate) fn template(
    source: &str,
    ir: &Element,
    component: &str,
    fingerprint: t::Fingerprint,
) -> t::Template {
    let emitter = Emitter {
        source,
        map: SourceMap::new(source),
    };
    t::Template {
        component: component.to_string(),
        fingerprint,
        compiler: env!("CARGO_PKG_VERSION").to_string(),
        root: emitter.element(ir),
    }
}

struct Emitter<'s> {
    source: &'s str,
    map: SourceMap,
}

impl Emitter<'_> {
    fn span(&self, span: Span) -> t::Span {
        let offset = |byte| t::Offset {
            byte,
            utf16: self.map.utf16_offset(self.source, byte),
        };
        t::Span {
            start: offset(span.start_byte),
            end: offset(span.end_byte),
        }
    }

    fn element(&self, element: &Element) -> t::Element {
        t::Element {
            component: element.name.clone(),
            props: element
                .attributes
                .iter()
                .map(|attribute| t::Prop {
                    prop: attribute.name.clone(),
                    value: match &attribute.value {
                        AttributeValue::String { value, span } => t::Expression::Literal {
                            value: t::Literal::String(value.clone()),
                            span: self.span(*span),
                        },
                        AttributeValue::Expression(expression) => self.expression(expression),
                    },
                    span: self.span(attribute.span),
                })
                .collect(),
            events: element
                .event_bindings
                .iter()
                .map(|binding| {
                    let Expression::Command {
                        command, arguments, ..
                    } = &binding.handler
                    else {
                        unreachable!(
                            "a clean check guarantees every handler is a command invocation"
                        )
                    };
                    t::EventBinding {
                        event: binding.name.clone(),
                        command: command.clone(),
                        arguments: arguments.iter().map(|a| self.expression(a)).collect(),
                        span: self.span(binding.span),
                    }
                })
                .collect(),
            children: element
                .children
                .iter()
                .map(|child| match child {
                    Child::Text { value, span } => t::Child::Text {
                        value: value.clone(),
                        span: self.span(*span),
                    },
                    Child::Expression(expression) => t::Child::Expression {
                        expression: self.expression(expression),
                    },
                    Child::Element(element) => t::Child::Element {
                        element: Box::new(self.element(element)),
                    },
                })
                .collect(),
            span: self.span(element.span),
        }
    }

    fn expression(&self, expression: &Expression) -> t::Expression {
        let boxed = |expression: &Expression| Box::new(self.expression(expression));
        match expression {
            Expression::Literal { value, span } => t::Expression::Literal {
                value: match value {
                    Literal::String(text) => t::Literal::String(text.clone()),
                    // `digit+ ('.' digit+)?`: Rust's parse is correctly
                    // rounded, ties to even, and a clean check means the
                    // value is finite.
                    Literal::Number(text) => t::Literal::Number(
                        text.parse()
                            .expect("a number literal is decimal digits, with at most one point"),
                    ),
                    Literal::Boolean(value) => t::Literal::Boolean(*value),
                    Literal::Null => t::Literal::Null,
                },
                span: self.span(*span),
            },
            Expression::Reference { name, span } => t::Expression::Scope {
                name: name.clone(),
                span: self.span(*span),
            },
            Expression::MemberAccess {
                object,
                property,
                span,
                ..
            } => t::Expression::Member {
                object: boxed(object),
                field: property.clone(),
                span: self.span(*span),
            },
            Expression::Unary {
                operator,
                operand,
                span,
            } => t::Expression::Unary {
                operator: match operator {
                    UnaryOperator::Not => t::UnaryOperator::Not,
                    UnaryOperator::Negate => t::UnaryOperator::Negate,
                },
                operand: boxed(operand),
                span: self.span(*span),
            },
            Expression::Binary {
                operator,
                left,
                right,
                span,
            } => t::Expression::Binary {
                operator: binary(*operator),
                left: boxed(left),
                right: boxed(right),
                span: self.span(*span),
            },
            Expression::Conditional {
                condition,
                consequent,
                alternate,
                span,
            } => t::Expression::Conditional {
                condition: boxed(condition),
                consequent: boxed(consequent),
                alternate: boxed(alternate),
                span: self.span(*span),
            },
            Expression::Array { elements, span } => t::Expression::List {
                elements: elements.iter().map(|e| self.expression(e)).collect(),
                span: self.span(*span),
            },
            Expression::Object { members, span } => t::Expression::Record {
                // A repeated key is an error (`duplicate-object-key`), so a
                // clean object's keys are already unique.
                fields: members
                    .iter()
                    .map(|member| t::Field {
                        name: member.key.clone(),
                        value: self.expression(&member.value),
                        span: self.span(member.span),
                    })
                    .collect(),
                span: self.span(*span),
            },
            Expression::EventValue { span, .. } => t::Expression::Event {
                span: self.span(*span),
            },
            Expression::Command { .. } => {
                unreachable!("a clean check guarantees a command appears only as a handler")
            }
        }
    }
}

fn binary(operator: BinaryOperator) -> t::BinaryOperator {
    match operator {
        BinaryOperator::Add => t::BinaryOperator::Add,
        BinaryOperator::Sub => t::BinaryOperator::Subtract,
        BinaryOperator::Mul => t::BinaryOperator::Multiply,
        BinaryOperator::Div => t::BinaryOperator::Divide,
        BinaryOperator::Mod => t::BinaryOperator::Remainder,
        BinaryOperator::Eq => t::BinaryOperator::Equal,
        BinaryOperator::Ne => t::BinaryOperator::NotEqual,
        BinaryOperator::Lt => t::BinaryOperator::Less,
        BinaryOperator::Le => t::BinaryOperator::LessEqual,
        BinaryOperator::Gt => t::BinaryOperator::Greater,
        BinaryOperator::Ge => t::BinaryOperator::GreaterEqual,
        BinaryOperator::And => t::BinaryOperator::And,
        BinaryOperator::Or => t::BinaryOperator::Or,
    }
}
