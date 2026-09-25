//! Reading a template, and refusing one that isn't.

use crate::types::{Child, Element, Expression, Literal, Template};
use crate::{Fingerprint, FORMAT, VERSION};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fmt;

/// Why [`from_json`] refused a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// Not a `template-v1` document: every problem found. (A document
    /// whose shape is wrong reports the first shape error only, since
    /// nothing after it can be read.)
    Malformed(Vec<Problem>),
    /// A template of a format version this crate doesn't read.
    UnsupportedVersion(u64),
}

/// One problem with a document, at a JSON Pointer (RFC 6901) into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// Where the problem is: `""` for the whole document.
    pub path: String,
    pub message: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.path.is_empty() {
            write!(f, "{}", self.message)
        } else {
            write!(f, "{}: {}", self.path, self.message)
        }
    }
}

/// The document as written: the template with its format and version.
#[derive(Deserialize)]
struct Document {
    component: String,
    fingerprint: String,
    compiler: String,
    root: Element,
}

/// Reads a `template-v1` document.
///
/// It is refused as [`Refusal::Malformed`] if it isn't JSON, isn't an
/// object whose `format` is `mesh-template`, has no integer `version`, or
/// doesn't have the structure the schema describes. It is refused as
/// [`Refusal::UnsupportedVersion`] if its `version` is an integer other
/// than [`VERSION`]. Beyond the schema, a template must place `$event`
/// only inside a handler's arguments, must not repeat a prop, event or
/// record field name, and must not have a `-0` literal.
///
/// Properties the format doesn't define are ignored, as the format's
/// promise requires of readers.
pub fn from_json(text: &str) -> Result<Template, Refusal> {
    let value: Value = serde_json::from_str(text)
        .map_err(|error| malformed("", format!("the template isn't JSON: {error}")))?;
    let Value::Object(object) = &value else {
        return Err(malformed("", "a template is a JSON object"));
    };
    if object.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return Err(malformed(
            "/format",
            format!("a template's `format` is \"{FORMAT}\""),
        ));
    }
    match object.get("version").and_then(Value::as_u64) {
        Some(VERSION) => {}
        Some(other) => return Err(Refusal::UnsupportedVersion(other)),
        None => {
            return Err(malformed(
                "/version",
                "a template's `version` is an integer",
            ))
        }
    }
    let document: Document = serde_json::from_value(value)
        .map_err(|error| malformed("", format!("not a template-v1 document: {error}")))?;

    let mut problems = Vec::new();
    let fingerprint = match document.fingerprint.parse::<Fingerprint>() {
        Ok(fingerprint) => Some(fingerprint),
        Err(error) => {
            problems.push(problem("/fingerprint", error.to_string()));
            None
        }
    };
    check_component(&document.component, "/component", &mut problems);
    if document.compiler.is_empty() {
        problems.push(problem("/compiler", "`compiler` is not empty"));
    }
    check_element(&document.root, "/root", &mut problems);

    match (problems.is_empty(), fingerprint) {
        (true, Some(fingerprint)) => Ok(Template {
            component: document.component,
            fingerprint,
            compiler: document.compiler,
            root: document.root,
        }),
        _ => Err(Refusal::Malformed(problems)),
    }
}

fn problem(path: &str, message: impl Into<String>) -> Problem {
    Problem {
        path: path.to_string(),
        message: message.into(),
    }
}

fn malformed(path: &str, message: impl Into<String>) -> Refusal {
    Refusal::Malformed(vec![problem(path, message)])
}

/// `[A-Za-z_][A-Za-z0-9_]*`: a prop, event, command, scope or field name.
fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `[A-Za-z_][A-Za-z0-9_-]*`: a component name.
fn is_tag_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn check_component(name: &str, path: &str, problems: &mut Vec<Problem>) {
    if !is_tag_name(name) {
        problems.push(problem(path, format!("{name:?} isn't a component name")));
    }
}

fn check_identifier(name: &str, path: &str, problems: &mut Vec<Problem>) {
    if !is_identifier(name) {
        problems.push(problem(path, format!("{name:?} isn't a name")));
    }
}

/// Reports every name after the first occurrence of one already seen.
fn check_unique<'a>(
    names: impl Iterator<Item = &'a str>,
    path: &str,
    what: &str,
    problems: &mut Vec<Problem>,
) {
    let mut seen = BTreeSet::new();
    for (index, name) in names.enumerate() {
        if !seen.insert(name) {
            problems.push(problem(
                &format!("{path}/{index}"),
                format!("a second {what} named {name:?}"),
            ));
        }
    }
}

fn check_element(element: &Element, path: &str, problems: &mut Vec<Problem>) {
    check_component(&element.component, &format!("{path}/component"), problems);
    for (index, prop) in element.props.iter().enumerate() {
        let at = format!("{path}/props/{index}");
        check_identifier(&prop.prop, &format!("{at}/prop"), problems);
        check_expression(&prop.value, &format!("{at}/value"), false, problems);
    }
    check_unique(
        element.props.iter().map(|prop| prop.prop.as_str()),
        &format!("{path}/props"),
        "prop",
        problems,
    );
    for (index, binding) in element.events.iter().enumerate() {
        let at = format!("{path}/events/{index}");
        check_identifier(&binding.event, &format!("{at}/event"), problems);
        check_identifier(&binding.command, &format!("{at}/command"), problems);
        for (position, argument) in binding.arguments.iter().enumerate() {
            check_expression(
                argument,
                &format!("{at}/arguments/{position}"),
                true,
                problems,
            );
        }
    }
    check_unique(
        element.events.iter().map(|binding| binding.event.as_str()),
        &format!("{path}/events"),
        "event binding",
        problems,
    );
    for (index, child) in element.children.iter().enumerate() {
        let at = format!("{path}/children/{index}");
        match child {
            Child::Text { value, .. } if value.is_empty() => {
                problems.push(problem(&format!("{at}/value"), "text is never empty"));
            }
            Child::Text { .. } => {}
            Child::Expression { expression } => {
                check_expression(expression, &format!("{at}/expression"), false, problems);
            }
            Child::Element { element } => {
                check_element(element, &format!("{at}/element"), problems);
            }
        }
    }
}

/// `in_handler`: whether the expression is inside a handler's arguments,
/// the only place `$event` may appear.
fn check_expression(
    expression: &Expression,
    path: &str,
    in_handler: bool,
    problems: &mut Vec<Problem>,
) {
    match expression {
        Expression::Literal {
            value: Literal::Number(number),
            ..
        } if *number == 0.0 && number.is_sign_negative() => {
            problems.push(problem(&format!("{path}/value"), "a literal is never -0"));
        }
        Expression::Literal { .. } => {}
        Expression::Scope { name, .. } => check_identifier(name, &format!("{path}/name"), problems),
        Expression::Member { object, field, .. } => {
            check_expression(object, &format!("{path}/object"), in_handler, problems);
            check_identifier(field, &format!("{path}/field"), problems);
        }
        Expression::Unary { operand, .. } => {
            check_expression(operand, &format!("{path}/operand"), in_handler, problems);
        }
        Expression::Binary { left, right, .. } => {
            check_expression(left, &format!("{path}/left"), in_handler, problems);
            check_expression(right, &format!("{path}/right"), in_handler, problems);
        }
        Expression::Conditional {
            condition,
            consequent,
            alternate,
            ..
        } => {
            check_expression(
                condition,
                &format!("{path}/condition"),
                in_handler,
                problems,
            );
            check_expression(
                consequent,
                &format!("{path}/consequent"),
                in_handler,
                problems,
            );
            check_expression(
                alternate,
                &format!("{path}/alternate"),
                in_handler,
                problems,
            );
        }
        Expression::List { elements, .. } => {
            for (index, element) in elements.iter().enumerate() {
                check_expression(
                    element,
                    &format!("{path}/elements/{index}"),
                    in_handler,
                    problems,
                );
            }
        }
        Expression::Record { fields, .. } => {
            for (index, field) in fields.iter().enumerate() {
                // A key may be any string (`{ "display-name": x }`, §5), so
                // only its uniqueness is checked.
                let at = format!("{path}/fields/{index}");
                check_expression(&field.value, &format!("{at}/value"), in_handler, problems);
            }
            check_unique(
                fields.iter().map(|field| field.name.as_str()),
                &format!("{path}/fields"),
                "record field",
                problems,
            );
        }
        Expression::Event { .. } if !in_handler => {
            problems.push(problem(
                path,
                "`$event` may appear only inside a handler's arguments",
            ));
        }
        Expression::Event { .. } => {}
    }
}
