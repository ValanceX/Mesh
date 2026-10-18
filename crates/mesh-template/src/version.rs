//! Which format version a template needs.
//!
//! `template-v2` is a superset of `template-v1`: it adds constructs, and a template that uses none
//! of them is written as version 1, byte for byte as before (so its fingerprint, tree and keys do
//! not change). A version 1 document that holds a version 2 construct is malformed.

use crate::types::{BinaryOperator, Child, Element, Expression};

/// A construct only `template-v2` has, and where (a JSON Pointer) it is.
pub(crate) struct Construct {
    pub path: String,
    pub name: &'static str,
}

/// Every version 2 construct in `element`, in document order.
pub(crate) fn constructs(element: &Element, path: &str) -> Vec<Construct> {
    let mut found = Vec::new();
    in_element(element, path, &mut found);
    found
}

/// The least version that can hold `element`: 2 if it has a version 2 construct, else 1.
pub(crate) fn needed(element: &Element) -> u64 {
    if constructs(element, "").is_empty() {
        1
    } else {
        2
    }
}

fn in_element(element: &Element, path: &str, found: &mut Vec<Construct>) {
    for (index, prop) in element.props.iter().enumerate() {
        in_expression(&prop.value, &format!("{path}/props/{index}/value"), found);
    }
    for (index, binding) in element.events.iter().enumerate() {
        for (position, argument) in binding.arguments.iter().enumerate() {
            in_expression(
                argument,
                &format!("{path}/events/{index}/arguments/{position}"),
                found,
            );
        }
    }
    for (index, child) in element.children.iter().enumerate() {
        let at = format!("{path}/children/{index}");
        match child {
            Child::Text { .. } => {}
            Child::Expression { expression } => {
                in_expression(expression, &format!("{at}/expression"), found);
            }
            Child::Element { element } => in_element(element, &format!("{at}/element"), found),
        }
    }
}

fn in_expression(expression: &Expression, path: &str, found: &mut Vec<Construct>) {
    match expression {
        Expression::Literal { .. } | Expression::Scope { .. } | Expression::Event { .. } => {}
        Expression::Member { object, .. } => {
            in_expression(object, &format!("{path}/object"), found)
        }
        Expression::Unary { operand, .. } => {
            in_expression(operand, &format!("{path}/operand"), found);
        }
        Expression::Has { operand, .. } => {
            found.push(Construct {
                path: path.to_string(),
                name: "has",
            });
            in_expression(operand, &format!("{path}/operand"), found);
        }
        Expression::Binary {
            operator,
            left,
            right,
            ..
        } => {
            if *operator == BinaryOperator::Coalesce {
                found.push(Construct {
                    path: path.to_string(),
                    name: "coalesce",
                });
            }
            in_expression(left, &format!("{path}/left"), found);
            in_expression(right, &format!("{path}/right"), found);
        }
        Expression::Conditional {
            condition,
            consequent,
            alternate,
            ..
        } => {
            in_expression(condition, &format!("{path}/condition"), found);
            in_expression(consequent, &format!("{path}/consequent"), found);
            in_expression(alternate, &format!("{path}/alternate"), found);
        }
        Expression::List { elements, .. } => {
            for (index, element) in elements.iter().enumerate() {
                in_expression(element, &format!("{path}/elements/{index}"), found);
            }
        }
        Expression::Record { fields, .. } => {
            for (index, field) in fields.iter().enumerate() {
                in_expression(&field.value, &format!("{path}/fields/{index}/value"), found);
            }
        }
    }
}
