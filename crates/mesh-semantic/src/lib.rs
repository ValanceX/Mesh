//! Semantic model for MPRX.
//!
//! For v0.1, this crate builds the Semantic IR from the `mesh-syntax` AST,
//! applying structural validation (duplicate-attribute/duplicate-event-binding
//! deduplication, tag-name-mismatch checks) along the way. It does not yet
//! resolve component/prop/binding references against an external component
//! model — that requires a typed component model that doesn't exist yet.

use mesh_syntax::Span;

/// The Semantic IR form of an [`mesh_syntax::Element`].
///
/// Every IR node carries the source [`Span`]s a diagnostic may need to
/// point at: `span` covers the whole construct, and a separate `*_span`
/// field covers each name that can be wrong on its own. Spans are byte
/// offsets into the source exactly as read, a leading BOM included.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    pub name: String,
    /// The opening tag's name, e.g. `user-card` in `<user-card ...>`.
    pub name_span: Span,
    pub attributes: Vec<Attribute>,
    pub event_bindings: Vec<EventBinding>,
    pub children: Vec<Child>,
    /// The whole element, from `<` to its closing `>` or `/>`.
    pub span: Span,
}

/// The Semantic IR form of an [`mesh_syntax::Attribute`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    /// The attribute's name, without `=` or the value.
    pub name_span: Span,
    pub value: AttributeValue,
    /// The whole `name=value`.
    pub span: Span,
}

/// The Semantic IR form of an [`mesh_syntax::EventBinding`]. `name` holds
/// only the identifier that follows `on.` in source (e.g. `"click"`) —
/// the `on.` prefix itself is never part of this field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventBinding {
    pub name: String,
    /// The event name after `on.`, e.g. `click` in `on.click={...}`.
    pub name_span: Span,
    pub handler: Expression,
    /// The whole `on.name={handler}`.
    pub span: Span,
}

/// The Semantic IR form of an [`mesh_syntax::AttributeValue`]. A string
/// value is decoded; its `span` covers the quoted source text, quotes
/// included.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeValue {
    String { value: String, span: Span },
    Expression(Expression),
}

impl AttributeValue {
    /// The value's source span: the quoted string, or the expression
    /// inside `{...}`.
    pub fn span(&self) -> Span {
        match self {
            AttributeValue::String { span, .. } => *span,
            AttributeValue::Expression(expression) => expression.span(),
        }
    }
}

/// The Semantic IR form of an [`mesh_syntax::Child`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Child {
    Text { value: String, span: Span },
    Expression(Expression),
    Element(Box<Element>),
}

/// The Semantic IR form of an [`mesh_syntax::Literal`]: the value only.
/// Its span is on the enclosing [`Expression::Literal`].
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
    /// The key as written: the bare identifier, or the quoted string
    /// including its quotes.
    pub key_span: Span,
    pub value: Expression,
    /// The whole `key: value`.
    pub span: Span,
}

/// The Semantic IR form of an [`mesh_syntax::Expression`]. `Unary` and
/// `Binary` reuse [`mesh_syntax::UnaryOperator`]/[`mesh_syntax::BinaryOperator`]
/// directly rather than redeclaring an IR-local copy.
///
/// Every variant has a `span` field covering the whole expression, so
/// `Expression::Reference { name, .. }` matches without naming it, and
/// [`Expression::span`] reads it from any variant. A parenthesized
/// expression has no node of its own, and its span excludes the
/// parentheses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    Literal {
        value: Literal,
        span: Span,
    },
    Reference {
        name: String,
        span: Span,
    },
    MemberAccess {
        object: Box<Expression>,
        property: String,
        /// The property name after the `.`.
        property_span: Span,
        span: Span,
    },
    Unary {
        operator: mesh_syntax::UnaryOperator,
        operand: Box<Expression>,
        span: Span,
    },
    Binary {
        operator: mesh_syntax::BinaryOperator,
        left: Box<Expression>,
        right: Box<Expression>,
        span: Span,
    },
    Conditional {
        condition: Box<Expression>,
        consequent: Box<Expression>,
        alternate: Box<Expression>,
        span: Span,
    },
    Array {
        elements: Vec<Expression>,
        span: Span,
    },
    Object {
        members: Vec<ObjectMember>,
        span: Span,
    },
    Command {
        command: String,
        /// The command's name, without the parentheses or arguments.
        command_span: Span,
        arguments: Vec<Expression>,
        span: Span,
    },
    /// A `$` special value. `name` has no `$`: `$event` is `"event"`.
    EventValue {
        name: String,
        span: Span,
    },
}

impl Expression {
    /// The span of the whole expression.
    pub fn span(&self) -> Span {
        match self {
            Expression::Literal { span, .. }
            | Expression::Reference { span, .. }
            | Expression::MemberAccess { span, .. }
            | Expression::Unary { span, .. }
            | Expression::Binary { span, .. }
            | Expression::Conditional { span, .. }
            | Expression::Array { span, .. }
            | Expression::Object { span, .. }
            | Expression::Command { span, .. }
            | Expression::EventValue { span, .. } => *span,
        }
    }
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
    code: mesh_syntax::DiagnosticCode,
) -> (Vec<&'a T>, Vec<mesh_syntax::Diagnostic>) {
    let mut last_index_for_name: std::collections::HashMap<&str, usize> =
        std::collections::HashMap::new();
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
                code,
                message: format!(
                    "duplicate {kind} {:?}: this occurrence is shadowed by a later one",
                    name(item)
                ),
                span: span(item),
                suggestions: Vec::new(),
            });
        }
    }

    (survivors, diagnostics)
}

/// Lowers an AST [`mesh_syntax::Element`] into its Semantic IR form.
///
/// This copies each node's source spans into the IR, deduplicates
/// shadowed attributes and event bindings (last occurrence wins), and
/// flags mismatched closing tags — it does not yet resolve references
/// against a component model.
/// `ir` is always `Some(..)`: no diagnostic in v0.1, regardless of
/// severity, prevents producing IR for an AST that exists.
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
        mesh_syntax::DiagnosticCode::DUPLICATE_ATTRIBUTE,
    );
    diagnostics.extend(attribute_diagnostics);

    let (event_bindings, event_binding_diagnostics) = dedupe_last_wins(
        &ast.event_bindings,
        |e| e.name.as_str(),
        |e| e.span,
        "event binding",
        mesh_syntax::DiagnosticCode::DUPLICATE_EVENT_BINDING,
    );
    diagnostics.extend(event_binding_diagnostics);

    if let Some(closing_name) = &ast.closing_name {
        if closing_name != &ast.name {
            diagnostics.push(mesh_syntax::Diagnostic {
                severity: mesh_syntax::Severity::Error,
                code: mesh_syntax::DiagnosticCode::MISMATCHED_CLOSING_TAG,
                message: format!(
                    "mismatched closing tag: opened with {:?}, closed with {:?}",
                    ast.name, closing_name
                ),
                span: ast.span,
                suggestions: Vec::new(),
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
        name_span: ast.name_span,
        attributes: attributes.into_iter().map(lower_attribute).collect(),
        event_bindings: event_bindings
            .into_iter()
            .map(lower_event_binding)
            .collect(),
        children,
        span: ast.span,
    };

    (element, diagnostics)
}

fn lower_attribute(attribute: &mesh_syntax::Attribute) -> Attribute {
    Attribute {
        name: attribute.name.clone(),
        name_span: attribute.name_span,
        value: lower_attribute_value(&attribute.value),
        span: attribute.span,
    }
}

fn lower_event_binding(binding: &mesh_syntax::EventBinding) -> EventBinding {
    EventBinding {
        name: binding.name.clone(),
        name_span: binding.name_span,
        handler: lower_expression(&binding.handler),
        span: binding.span,
    }
}

fn lower_attribute_value(value: &mesh_syntax::AttributeValue) -> AttributeValue {
    match value {
        mesh_syntax::AttributeValue::String(literal) => AttributeValue::String {
            value: literal.value.clone(),
            span: literal.span,
        },
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
        mesh_syntax::Child::Text(text) => (
            Some(Child::Text {
                value: text.value.clone(),
                span: text.span,
            }),
            Vec::new(),
        ),
        mesh_syntax::Child::Expression(expression) => (
            Some(Child::Expression(lower_expression(expression))),
            Vec::new(),
        ),
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
        mesh_syntax::Expression::Literal(literal) => Expression::Literal {
            value: lower_literal(literal),
            span: literal_span(literal),
        },
        mesh_syntax::Expression::Reference(reference) => Expression::Reference {
            name: reference.name.clone(),
            span: reference.span,
        },
        mesh_syntax::Expression::MemberAccess(member) => Expression::MemberAccess {
            object: Box::new(lower_expression(&member.object)),
            property: member.property.clone(),
            property_span: member.property_span,
            span: member.span,
        },
        mesh_syntax::Expression::Unary(unary) => Expression::Unary {
            operator: unary.operator,
            operand: Box::new(lower_expression(&unary.operand)),
            span: unary.span,
        },
        mesh_syntax::Expression::Binary(binary) => Expression::Binary {
            operator: binary.operator,
            left: Box::new(lower_expression(&binary.left)),
            right: Box::new(lower_expression(&binary.right)),
            span: binary.span,
        },
        mesh_syntax::Expression::Conditional(conditional) => Expression::Conditional {
            condition: Box::new(lower_expression(&conditional.condition)),
            consequent: Box::new(lower_expression(&conditional.consequent)),
            alternate: Box::new(lower_expression(&conditional.alternate)),
            span: conditional.span,
        },
        mesh_syntax::Expression::Array(array) => Expression::Array {
            elements: array.elements.iter().map(lower_expression).collect(),
            span: array.span,
        },
        mesh_syntax::Expression::Object(object) => Expression::Object {
            members: object.members.iter().map(lower_object_member).collect(),
            span: object.span,
        },
        mesh_syntax::Expression::Command(command) => Expression::Command {
            command: command.command.clone(),
            command_span: command.command_span,
            arguments: command.arguments.iter().map(lower_expression).collect(),
            span: command.span,
        },
        mesh_syntax::Expression::EventValue(event) => Expression::EventValue {
            name: event.name.clone(),
            span: event.span,
        },
    }
}

fn lower_object_member(member: &mesh_syntax::ObjectMember) -> ObjectMember {
    ObjectMember {
        key: lower_object_key(&member.key),
        key_span: member.key_span,
        value: lower_expression(&member.value),
        span: member.span,
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

fn literal_span(literal: &mesh_syntax::Literal) -> Span {
    match literal {
        mesh_syntax::Literal::String(s) => s.span,
        mesh_syntax::Literal::Number(n) => n.span,
        mesh_syntax::Literal::Boolean(b) => b.span,
        mesh_syntax::Literal::Null(n) => n.span,
    }
}
