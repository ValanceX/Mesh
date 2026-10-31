//! Semantic model for MPRX.
//!
//! This crate builds the Semantic IR from the `mesh-syntax` AST, applying
//! structural validation (duplicate-attribute/duplicate-event-binding
//! deduplication, tag-name-mismatch checks) along the way. It knows
//! nothing about components: resolving and type-checking the IR against a
//! component manifest is `mesh-analysis`'s job.

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
    /// The opening tag's name, e.g. `user-card` in `<user-card ...>`. For a
    /// [`Construct`], the block's keyword: `{#if`, `{:else if`, `{#each`.
    pub name_span: Span,
    /// What a block lowered to this element, or `None` for an element that was
    /// written as one. Only lowering sets it, so it is how a tool tells a block
    /// from an element named the same.
    pub construct: Option<Construct>,
    pub attributes: Vec<Attribute>,
    pub event_bindings: Vec<EventBinding>,
    pub children: Vec<Child>,
    /// The whole element, from `<` to its closing `>` or `/>`.
    pub span: Span,
}

/// The construct a block lowers to. Blocks are syntax, not elements, and the constructs they lower
/// to are built in: a manifest need not declare them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Construct {
    /// `{#if condition} … {/if}` with no other branch: the conditional `mesh-if`.
    If,
    /// An `{#if}` with `{:else if}` or `{:else}` branches: the choice among them, `mesh-switch`.
    Switch,
    /// One branch of a [`Construct::Switch`]. `first` is the `{#if}` branch; the others are `{:else if}`.
    Case { first: bool },
    /// The `{:else}` branch of a [`Construct::Switch`].
    Default,
    /// `{#each items as item (key)} … {/each}`: the repeat `mesh-each`.
    Each,
    /// The `mesh-fragment` that wraps a branch or body that is not exactly one element.
    Fragment,
    /// `{#snippet name(params)} … {/snippet}`: the declaration `mesh-snippet`. Its `name` is a
    /// string attribute, its `params` an array of string literals, and its children the body, as
    /// written.
    Snippet,
    /// `{@render name(args)}`: `mesh-render`, whose `name` is a string attribute and whose
    /// `arguments` is an array of the argument expressions.
    Render,
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
/// shadowed attributes and event bindings (last occurrence wins), flags
/// mismatched closing tags, and reports number literals too large to be
/// finite (`number-literal-out-of-range`, in the kept attributes, the
/// kept handlers and the content). Resolving names against a component
/// model is `mesh-analysis`'s job, not this one's.
///
/// `ir` is always `Some(..)`: no lowering diagnostic, regardless of
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

    let attributes: Vec<Attribute> = attributes.into_iter().map(lower_attribute).collect();
    let event_bindings: Vec<EventBinding> = event_bindings
        .into_iter()
        .map(lower_event_binding)
        .collect();
    for attribute in &attributes {
        if let AttributeValue::Expression(expression) = &attribute.value {
            out_of_range_literals(expression, &mut diagnostics);
        }
    }
    for binding in &event_bindings {
        out_of_range_literals(&binding.handler, &mut diagnostics);
    }

    let mut children = Vec::new();
    for (at, child) in ast.children.iter().enumerate() {
        let (lowered, child_diagnostics) = if is_space_between_interpolations(&ast.children, at) {
            let mesh_syntax::Child::Text(text) = child else {
                unreachable!("a space between interpolations is a text child")
            };

            (
                Some(Child::Text {
                    value: text.value.clone(),
                    span: text.span,
                }),
                Vec::new(),
            )
        } else {
            lower_child(child)
        };
        if let Some(Child::Expression(expression)) = &lowered {
            out_of_range_literals(expression, &mut diagnostics);
        }
        children.extend(lowered);
        diagnostics.extend(child_diagnostics);
    }

    if let Some(block) = retired_block(&ast.name) {
        diagnostics.push(mesh_syntax::Diagnostic {
            severity: mesh_syntax::Severity::Error,
            code: mesh_syntax::DiagnosticCode::RETIRED_ELEMENT,
            message: format!("`<{}>` is no longer an element: write {block}", ast.name),
            span: ast.name_span,
            suggestions: Vec::new(),
        });
    }

    let element = Element {
        name: ast.name.clone(),
        name_span: ast.name_span,
        construct: None,
        attributes,
        event_bindings,
        children,
        span: ast.span,
    };

    (element, diagnostics)
}

/// Reports each number literal in `expression` whose nearest binary64
/// value would be infinite (§9.7.3), in source order. Literals are
/// unsigned and `digit+ ('.' digit+)?`, so Rust's correctly rounded parse
/// always succeeds, and gives an infinity exactly when the literal is out
/// of range.
fn out_of_range_literals(expression: &Expression, diagnostics: &mut Vec<mesh_syntax::Diagnostic>) {
    match expression {
        Expression::Literal {
            value: Literal::Number(text),
            span,
        } => {
            if text.parse::<f64>().is_ok_and(f64::is_infinite) {
                diagnostics.push(mesh_syntax::Diagnostic {
                    severity: mesh_syntax::Severity::Error,
                    code: mesh_syntax::DiagnosticCode::NUMBER_LITERAL_OUT_OF_RANGE,
                    message: "this number is too large: the largest number MPRX can represent is about 1.8e308"
                        .to_string(),
                    span: *span,
                    suggestions: Vec::new(),
                });
            }
        }
        Expression::Literal { .. }
        | Expression::Reference { .. }
        | Expression::EventValue { .. } => {}
        Expression::MemberAccess { object, .. } => out_of_range_literals(object, diagnostics),
        Expression::Unary { operand, .. } => out_of_range_literals(operand, diagnostics),
        Expression::Binary { left, right, .. } => {
            out_of_range_literals(left, diagnostics);
            out_of_range_literals(right, diagnostics);
        }
        Expression::Conditional {
            condition,
            consequent,
            alternate,
            ..
        } => {
            out_of_range_literals(condition, diagnostics);
            out_of_range_literals(consequent, diagnostics);
            out_of_range_literals(alternate, diagnostics);
        }
        Expression::Array { elements, .. } => {
            for element in elements {
                out_of_range_literals(element, diagnostics);
            }
        }
        Expression::Object { members, .. } => {
            for member in members {
                out_of_range_literals(&member.value, diagnostics);
            }
        }
        Expression::Command { arguments, .. } => {
            for argument in arguments {
                out_of_range_literals(argument, diagnostics);
            }
        }
    }
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
        mesh_syntax::Child::Block(block) => {
            let (lowered, diagnostics) = lower_block(block);
            (Some(Child::Element(Box::new(lowered))), diagnostics)
        }
    }
}

/// A whitespace-only text child with no line break and an interpolation on each side: a space someone wrote between two parts of a text run, so it is content, not formatting. A line break
/// makes it layout (indentation), and next to anything but an interpolation it is dropped as before (docs/explanation/specs/2026-10-16-mprx-whitespace-between-interpolations.md, W-1 to W-3).
fn is_space_between_interpolations(children: &[mesh_syntax::Child], at: usize) -> bool {
    let mesh_syntax::Child::Text(text) = &children[at] else {
        return false;
    };

    at > 0
        && text.value.trim().is_empty()
        && !text.value.contains(['\n', '\r'])
        && matches!(children[at - 1], mesh_syntax::Child::Expression(_))
        && matches!(
            children.get(at + 1),
            Some(mesh_syntax::Child::Expression(_))
        )
}

/// What to write instead of a reserved element that blocks replaced, or `None` for any other tag.
pub fn retired_block(tag: &str) -> Option<&'static str> {
    match tag {
        "mesh-if" => Some("a block: `{#if condition} … {/if}`"),
        "mesh-each" => Some("a block: `{#each items as item (key)} … {/each}`"),
        "mesh-switch" => Some("a chain of branches: `{#if a} … {:else if b} … {:else} … {/if}`"),
        "mesh-case" => Some("a branch: `{#if a} … {:else if b} … {/if}`"),
        "mesh-default" => Some("the last branch: `{:else} …` inside `{#if} … {/if}`"),
        "mesh-snippet" => Some("a snippet: `{#snippet name(parameters)} … {/snippet}`"),
        "mesh-render" => Some("a render tag: `{@render name(arguments)}`"),
        _ => None,
    }
}

const CONDITIONAL: &str = "mesh-if";
const SWITCH: &str = "mesh-switch";
const CASE: &str = "mesh-case";
const DEFAULT: &str = "mesh-default";
const REPEAT: &str = "mesh-each";
const FRAGMENT: &str = "mesh-fragment";
const SNIPPET: &str = "mesh-snippet";
const RENDER: &str = "mesh-render";

fn construct_element(
    name: &str,
    construct: Construct,
    keyword: Span,
    attributes: Vec<Attribute>,
    children: Vec<Child>,
    span: Span,
) -> Element {
    Element {
        name: name.to_string(),
        name_span: keyword,
        construct: Some(construct),
        attributes,
        event_bindings: Vec::new(),
        children,
        span,
    }
}

/// The children of a branch or body: exactly one element is that element; anything else (text, several
/// nodes, none, a lone block or slot) is wrapped in a fragment, so that a branch is always one element.
fn body(
    children: Vec<(Option<Child>, Vec<mesh_syntax::Diagnostic>)>,
    keyword: Span,
    span: Span,
    diagnostics: &mut Vec<mesh_syntax::Diagnostic>,
) -> Vec<Child> {
    let mut lowered = Vec::new();

    for (child, found) in children {
        diagnostics.extend(found);
        if let Some(child) = child {
            if let Child::Expression(expression) = &child {
                out_of_range_literals(expression, diagnostics);
            }
            lowered.push(child);
        }
    }

    let alone = matches!(
        lowered.as_slice(),
        [Child::Element(element)] if element.construct.is_none() && element.name != "mesh-slot"
    );

    if alone {
        lowered
    } else {
        vec![Child::Element(Box::new(construct_element(
            FRAGMENT,
            Construct::Fragment,
            keyword,
            Vec::new(),
            lowered,
            span,
        )))]
    }
}

fn condition_attribute(keyword: Span, condition: &mesh_syntax::Expression) -> Attribute {
    let value = lower_expression(condition);

    Attribute {
        name: "when".to_string(),
        name_span: keyword,
        span: value.span(),
        value: AttributeValue::Expression(value),
    }
}

/// Lowers a block to the construct it stands for. The runtime, the template format and node identities
/// are unchanged: `{#if}` is the conditional, a chain of branches the choice among them, `{#each}` the
/// repeat, and each of them was an element before blocks existed.
fn lower_block(block: &mesh_syntax::Block) -> (Element, Vec<mesh_syntax::Diagnostic>) {
    let mut diagnostics = Vec::new();

    match block {
        mesh_syntax::Block::If(block) => {
            let first = &block.branches[0];
            let mut condition_diagnostics = Vec::new();
            for branch in &block.branches {
                let condition = lower_expression(&branch.condition);
                out_of_range_literals(&condition, &mut condition_diagnostics);
            }
            diagnostics.extend(condition_diagnostics);

            if block.branches.len() == 1 && block.otherwise.is_none() {
                let children = body(
                    first.children.iter().map(lower_child).collect(),
                    first.keyword_span,
                    first.span,
                    &mut diagnostics,
                );

                return (
                    construct_element(
                        CONDITIONAL,
                        Construct::If,
                        first.keyword_span,
                        vec![condition_attribute(first.keyword_span, &first.condition)],
                        children,
                        block.span,
                    ),
                    diagnostics,
                );
            }

            let mut cases = Vec::new();
            for (index, branch) in block.branches.iter().enumerate() {
                let children = body(
                    branch.children.iter().map(lower_child).collect(),
                    branch.keyword_span,
                    branch.span,
                    &mut diagnostics,
                );
                cases.push(Child::Element(Box::new(construct_element(
                    CASE,
                    Construct::Case { first: index == 0 },
                    branch.keyword_span,
                    vec![condition_attribute(branch.keyword_span, &branch.condition)],
                    children,
                    branch.span,
                ))));
            }
            if let Some(otherwise) = &block.otherwise {
                let children = body(
                    otherwise.children.iter().map(lower_child).collect(),
                    otherwise.keyword_span,
                    otherwise.span,
                    &mut diagnostics,
                );
                cases.push(Child::Element(Box::new(construct_element(
                    DEFAULT,
                    Construct::Default,
                    otherwise.keyword_span,
                    Vec::new(),
                    children,
                    otherwise.span,
                ))));
            }

            (
                construct_element(
                    SWITCH,
                    Construct::Switch,
                    first.keyword_span,
                    Vec::new(),
                    cases,
                    block.span,
                ),
                diagnostics,
            )
        }
        mesh_syntax::Block::Snippet(block) => {
            let attributes = vec![
                Attribute {
                    name: "name".to_string(),
                    name_span: block.keyword_span,
                    span: block.name_span,
                    value: AttributeValue::String {
                        value: block.name.clone(),
                        span: block.name_span,
                    },
                },
                Attribute {
                    name: "params".to_string(),
                    name_span: block.keyword_span,
                    span: block.name_span,
                    value: AttributeValue::Expression(Expression::Array {
                        elements: block
                            .parameters
                            .iter()
                            .map(|parameter| Expression::Literal {
                                value: Literal::String(parameter.name.clone()),
                                span: parameter.span,
                            })
                            .collect(),
                        span: block.name_span,
                    }),
                },
            ];
            let mut children = Vec::new();
            for (child, found) in block.children.iter().map(lower_child) {
                diagnostics.extend(found);
                if let Some(child) = child {
                    if let Child::Expression(expression) = &child {
                        out_of_range_literals(expression, &mut diagnostics);
                    }
                    children.push(child);
                }
            }

            (
                construct_element(
                    SNIPPET,
                    Construct::Snippet,
                    block.keyword_span,
                    attributes,
                    children,
                    block.span,
                ),
                diagnostics,
            )
        }
        mesh_syntax::Block::Render(tag) => {
            let arguments: Vec<Expression> = tag.arguments.iter().map(lower_expression).collect();
            for argument in &arguments {
                out_of_range_literals(argument, &mut diagnostics);
            }
            let attributes = vec![
                Attribute {
                    name: "name".to_string(),
                    name_span: tag.keyword_span,
                    span: tag.name_span,
                    value: AttributeValue::String {
                        value: tag.name.clone(),
                        span: tag.name_span,
                    },
                },
                Attribute {
                    name: "arguments".to_string(),
                    name_span: tag.keyword_span,
                    span: tag.span,
                    value: AttributeValue::Expression(Expression::Array {
                        elements: arguments,
                        span: tag.span,
                    }),
                },
            ];

            (
                construct_element(
                    RENDER,
                    Construct::Render,
                    tag.keyword_span,
                    attributes,
                    Vec::new(),
                    tag.span,
                ),
                diagnostics,
            )
        }
        mesh_syntax::Block::Each(block) => {
            let items = lower_expression(&block.items);
            let mut attributes = vec![
                Attribute {
                    name: "items".to_string(),
                    name_span: block.keyword_span,
                    span: items.span(),
                    value: AttributeValue::Expression(items),
                },
                Attribute {
                    name: "as".to_string(),
                    name_span: block.keyword_span,
                    span: block.binding_span,
                    value: AttributeValue::String {
                        value: block.binding.clone(),
                        span: block.binding_span,
                    },
                },
            ];
            match &block.key {
                Some(key) => {
                    let key = lower_expression(key);
                    attributes.push(Attribute {
                        name: "key".to_string(),
                        name_span: block.keyword_span,
                        span: key.span(),
                        value: AttributeValue::Expression(key),
                    });
                }
                None => diagnostics.push(mesh_syntax::Diagnostic {
                    severity: mesh_syntax::Severity::Error,
                    code: mesh_syntax::DiagnosticCode::MISSING_REPEAT_KEY,
                    message: "`{#each}` needs a key, which identifies each item across updates: write `{#each items as item (key)}`, with a string or a number that is different for each item".to_string(),
                    span: block.keyword_span,
                    suggestions: Vec::new(),
                }),
            }
            for attribute in &attributes {
                if let AttributeValue::Expression(expression) = &attribute.value {
                    out_of_range_literals(expression, &mut diagnostics);
                }
            }
            let children = body(
                block.children.iter().map(lower_child).collect(),
                block.keyword_span,
                block.span,
                &mut diagnostics,
            );

            (
                construct_element(
                    REPEAT,
                    Construct::Each,
                    block.keyword_span,
                    attributes,
                    children,
                    block.span,
                ),
                diagnostics,
            )
        }
    }
}

/// Lowers one AST expression into its IR form. [`lower`] uses it for
/// every expression in an element; it is public for tools that have an
/// expression without an element around it, such as an editor's recovery
/// of a file with syntax errors. Lowering an expression reports nothing.
pub fn lower_expression(expression: &mesh_syntax::Expression) -> Expression {
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
