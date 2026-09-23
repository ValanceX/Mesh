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
    pub event_bindings: Vec<EventBinding>,
    pub children: Vec<Child>,
}

/// The Semantic IR form of an [`mesh_syntax::Attribute`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    pub value: AttributeValue,
}

/// The Semantic IR form of an [`mesh_syntax::EventBinding`]. `name` holds
/// only the identifier that follows `on.` in source (e.g. `"click"`) —
/// the `on.` prefix itself is never part of this field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventBinding {
    pub name: String,
    pub handler: Expression,
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
    Element(Box<Element>),
}

/// The Semantic IR form of an [`mesh_syntax::Literal`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Literal {
    String(String),
    Number(String),
    Boolean(bool),
    Null,
}

/// The Semantic IR form of an [`mesh_syntax::ObjectMember`]. `key`
/// flattens [`mesh_syntax::ObjectKey`] to a plain `String` — its two
/// variants (bare identifier vs. quoted string) are semantically
/// equivalent once lowered, both just naming a field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectMember {
    pub key: String,
    pub value: Expression,
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
    Array(Vec<Expression>),
    Object(Vec<ObjectMember>),
    Command {
        command: String,
        arguments: Vec<Expression>,
    },
    EventValue(String),
}

/// The result of lowering one AST [`mesh_syntax::Element`] into Semantic
/// IR: the IR, if lowering produced one, and every diagnostic reported
/// along the way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LowerResult {
    pub ir: Option<Element>,
    pub diagnostics: Vec<mesh_syntax::Diagnostic>,
}

fn dedupe_last_wins<'a, T>(
    items: &'a [T],
    name: fn(&T) -> &str,
    span: fn(&T) -> mesh_syntax::Span,
    kind: &str,
) -> (Vec<&'a T>, Vec<mesh_syntax::Diagnostic>) {
    let mut last_index_for_name: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (index, item) in items.iter().enumerate() {
        last_index_for_name.insert(name(item), index);
    }

    let mut survivors = Vec::new();
    let mut diagnostics = Vec::new();

    for (index, item) in items.iter().enumerate() {
        if last_index_for_name[name(item)] == index {
            survivors.push(item);
        } else {
            diagnostics.push(mesh_syntax::Diagnostic {
                severity: mesh_syntax::Severity::Warning,
                message: format!(
                    "duplicate {kind} {:?}: this occurrence is shadowed by a later one",
                    name(item)
                ),
                span: span(item),
            });
        }
    }

    (survivors, diagnostics)
}

/// Lowers an AST [`mesh_syntax::Element`] into its Semantic IR form.
///
/// For v0.1 this is a structural pass-through (drops source spans, copies
/// everything else) — it does not yet resolve references against a
/// component model. `ir` is always `Some(..)`: v0.1 introduces no fatal
/// semantic validation rule that prevents producing IR for an AST that
/// exists.
pub fn lower(ast: &mesh_syntax::Element) -> LowerResult {
    let (element, diagnostics) = lower_element(ast);
    LowerResult {
        ir: Some(element),
        diagnostics,
    }
}

fn lower_element(ast: &mesh_syntax::Element) -> (Element, Vec<mesh_syntax::Diagnostic>) {
    let mut diagnostics = Vec::new();

    let (attributes, attribute_diagnostics) = dedupe_last_wins(
        &ast.attributes,
        |a| a.name.as_str(),
        |a| a.span,
        "attribute",
    );
    diagnostics.extend(attribute_diagnostics);

    let (event_bindings, event_binding_diagnostics) = dedupe_last_wins(
        &ast.event_bindings,
        |e| e.name.as_str(),
        |e| e.span,
        "event binding",
    );
    diagnostics.extend(event_binding_diagnostics);

    if let Some(closing_name) = &ast.closing_name {
        if closing_name != &ast.name {
            diagnostics.push(mesh_syntax::Diagnostic {
                severity: mesh_syntax::Severity::Error,
                message: format!(
                    "mismatched closing tag: opened with {:?}, closed with {:?}",
                    ast.name, closing_name
                ),
                span: ast.span,
            });
        }
    }

    let mut children = Vec::new();
    for child in &ast.children {
        let (lowered, child_diagnostics) = lower_child(child);
        children.extend(lowered);
        diagnostics.extend(child_diagnostics);
    }

    let element = Element {
        name: ast.name.clone(),
        attributes: attributes.into_iter().map(lower_attribute).collect(),
        event_bindings: event_bindings.into_iter().map(lower_event_binding).collect(),
        children,
    };

    (element, diagnostics)
}

fn lower_attribute(attribute: &mesh_syntax::Attribute) -> Attribute {
    Attribute {
        name: attribute.name.clone(),
        value: lower_attribute_value(&attribute.value),
    }
}

fn lower_event_binding(binding: &mesh_syntax::EventBinding) -> EventBinding {
    EventBinding {
        name: binding.name.clone(),
        handler: lower_expression(&binding.handler),
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

/// Lowers one AST child into its IR form, or `None` if the child is
/// formatting-only. "Whitespace-only" is exactly Rust's
/// `str::trim().is_empty()` (Unicode-aware) — not a custom character
/// class. This is IR-only: `mesh-parser`'s CST -> AST lowering performs
/// no filtering, so the AST retains every text node exactly as parsed.
fn lower_child(child: &mesh_syntax::Child) -> (Option<Child>, Vec<mesh_syntax::Diagnostic>) {
    match child {
        mesh_syntax::Child::Text(text) if text.value.trim().is_empty() => (None, Vec::new()),
        mesh_syntax::Child::Text(text) => (Some(Child::Text(text.value.clone())), Vec::new()),
        mesh_syntax::Child::Expression(expression) => {
            (Some(Child::Expression(lower_expression(expression))), Vec::new())
        }
        // Second mutually-recursive lowering path (AST -> IR), same
        // deferred no-guard status as mesh-parser's lower_child (see its
        // comment on the CST -> AST path for the full explanation).
        mesh_syntax::Child::Element(element) => {
            let (lowered, diagnostics) = lower_element(element);
            (Some(Child::Element(Box::new(lowered))), diagnostics)
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
        mesh_syntax::Expression::Array(array) => {
            Expression::Array(array.elements.iter().map(lower_expression).collect())
        }
        mesh_syntax::Expression::Object(object) => {
            Expression::Object(object.members.iter().map(lower_object_member).collect())
        }
        mesh_syntax::Expression::Command(command) => Expression::Command {
            command: command.command.clone(),
            arguments: command.arguments.iter().map(lower_expression).collect(),
        },
        mesh_syntax::Expression::EventValue(event) => Expression::EventValue(event.name.clone()),
    }
}

fn lower_object_member(member: &mesh_syntax::ObjectMember) -> ObjectMember {
    ObjectMember {
        key: lower_object_key(&member.key),
        value: lower_expression(&member.value),
    }
}

fn lower_object_key(key: &mesh_syntax::ObjectKey) -> String {
    match key {
        mesh_syntax::ObjectKey::Identifier(name) => name.clone(),
        mesh_syntax::ObjectKey::String(s) => s.value.clone(),
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
