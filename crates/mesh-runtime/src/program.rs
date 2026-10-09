//! A program, validated (D3's program validation, and the assembly rules
//! of docs/manual/templates.md), and the identities derived from it:
//! program identity, keys and handler identifiers (docs/manual/runtime.md).

use crate::diagnostic::{Location, RuntimeCode, RuntimeDiagnostic};
use crate::number::number_to_text;
use mesh_manifest::{Component, Manifest, Type};
use mesh_template::{Child, Element, Expression, Refusal, Template};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

/// The parts of a program, as a host holds them: the root component's
/// name, and each template's text (`template-v1`).
#[derive(Debug, Clone, Copy)]
pub struct Program<'a> {
    pub root: &'a str,
    pub templates: &'a [&'a str],
}

/// A program that passed validation.
pub(crate) struct Valid {
    pub manifest: Manifest,
    pub root: String,
    /// The templates, by component.
    pub templates: BTreeMap<String, Template>,
    pub identity: [u8; 32],
}

impl Valid {
    pub(crate) fn component(&self, name: &str) -> &Component {
        &self.manifest.components()[name]
    }

    pub(crate) fn is_composite(&self, component: &str) -> bool {
        self.templates.contains_key(component)
    }
}

/// D3's program validation: its steps in order, each reporting every
/// problem it finds, and the first step that reports anything ending it.
pub(crate) fn validate(
    program: &Program<'_>,
    model: &str,
) -> Result<Valid, Vec<RuntimeDiagnostic>> {
    // Step 1: the model.
    let manifest = mesh_manifest::load(model).map_err(|diagnostics| {
        diagnostics
            .iter()
            .map(RuntimeDiagnostic::from_manifest)
            .collect::<Vec<_>>()
    })?;

    // Step 2: every template well-formed, of a supported version, and
    // naming only what the model declares.
    let mut diagnostics = Vec::new();
    let mut read = Vec::new();
    for (index, text) in program.templates.iter().enumerate() {
        let component = readable_component(text);
        match mesh_template::from_json(text) {
            Err(Refusal::UnsupportedVersion(version)) => diagnostics.push(RuntimeDiagnostic::new(
                RuntimeCode::UNSUPPORTED_FORMAT_VERSION,
                format!(
                    "the template is format version {version}; this runtime reads version {}",
                    mesh_template::VERSION
                ),
                Location::Template { index, component },
            )),
            Err(Refusal::Malformed(problems)) => diagnostics.push(RuntimeDiagnostic::new(
                RuntimeCode::MALFORMED_TEMPLATE,
                format!(
                    "the template is malformed: {}",
                    problems
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("; ")
                ),
                Location::Template { index, component },
            )),
            Ok(template) => {
                let problems = undeclared(&manifest, &template);
                if problems.is_empty() {
                    read.push((index, template));
                } else {
                    diagnostics.push(RuntimeDiagnostic::new(
                        RuntimeCode::MALFORMED_TEMPLATE,
                        format!(
                            "the template names what the model doesn't declare: {}",
                            problems.join("; ")
                        ),
                        Location::Template {
                            index,
                            component: Some(template.component.clone()),
                        },
                    ));
                }
            }
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    // Step 3: every template checked against this model.
    let fingerprint = mesh_template::fingerprint(&manifest);
    for (index, template) in &read {
        if template.fingerprint != fingerprint {
            diagnostics.push(RuntimeDiagnostic::new(
                RuntimeCode::FINGERPRINT_MISMATCH,
                format!(
                    "the template of `{}` was checked against another model ({}, not {fingerprint}): recompile it",
                    template.component, template.fingerprint
                ),
                Location::Template { index: *index, component: Some(template.component.clone()) },
            ));
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    // Step 4: the assembly rules.
    let diagnostics = assembly(&manifest, program.root, &read);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let identity = identity(program);
    let templates = read
        .into_iter()
        .map(|(_, template)| (template.component.clone(), template))
        .collect();
    Ok(Valid {
        manifest,
        root: program.root.to_string(),
        templates,
        identity,
    })
}

/// A malformed template's `component`, if it has a string one.
fn readable_component(text: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    value.get("component")?.as_str().map(str::to_string)
}

/// Every name in `template` the model doesn't declare with the kind the
/// template gives it (docs/manual/templates.md, "Names").
fn undeclared(manifest: &Manifest, template: &Template) -> Vec<String> {
    let mut problems = Vec::new();
    let Some(own) = manifest.components().get(&template.component) else {
        return vec![format!("no component `{}`", template.component)];
    };
    for reserved in [
        CONDITIONAL_COMPONENT,
        REPEAT_COMPONENT,
        SLOT_COMPONENT,
        FILL_COMPONENT,
    ] {
        if template.root.component == reserved {
            problems.push(format!(
                "a template's root can't be a `{reserved}`: a render has exactly one root node"
            ));
        }
    }
    walk_names(
        manifest,
        own,
        &template.root,
        &mut Vec::new(),
        &mut problems,
    );
    let mut all = Vec::new();
    elements(&template.root, &mut all);
    let mut named = BTreeSet::new();
    for slot in all
        .iter()
        .filter(|element| element.component == SLOT_COMPONENT)
    {
        if !named.insert(slot_name(slot)) {
            problems.push(if slot_name(slot).is_empty() {
                format!("a template has at most one `{SLOT_COMPONENT}` without a name: a composite has one default place for its children")
            } else {
                format!(
                    "a template has at most one `{SLOT_COMPONENT}` named `{}`",
                    slot_name(slot)
                )
            });
        }
    }
    problems
}

/// `loops` is the names the enclosing `mesh-each` elements bind, innermost
/// last: references to them are not scope names.
fn walk_names(
    manifest: &Manifest,
    own: &Component,
    element: &Element,
    loops: &mut Vec<String>,
    problems: &mut Vec<String>,
) {
    let components = manifest.components();
    let Some(component) = components.get(&element.component) else {
        problems.push(format!("no component `{}`", element.component));
        return;
    };
    if element.component == CONDITIONAL_COMPONENT {
        conditional_problems(element, problems);
    }
    if element.component == SLOT_COMPONENT {
        slot_problems(element, problems);
    }
    if element.component == FILL_COMPONENT {
        fill_problems(element, problems);
    }
    let repeated = element.component == REPEAT_COMPONENT;
    if repeated {
        repeat_problems(element, problems);
    }
    let mut bound = false;
    // `items` is read outside the binding; everything else inside it.
    let mut props: Vec<&_> = element.props.iter().collect();
    if repeated {
        props.sort_by_key(|prop| prop.prop != "items");
    }
    for prop in props {
        if !component.props.contains_key(&prop.prop) {
            problems.push(format!(
                "`{}` has no prop `{}`",
                element.component, prop.prop
            ));
        }
        if repeated && prop.prop != "items" && !bound {
            if let Some(name) = repeat_name(element) {
                loops.push(name.to_string());
                bound = true;
            }
        }
        scope_names(own, loops, &prop.value, problems);
    }
    if repeated && !bound {
        if let Some(name) = repeat_name(element) {
            loops.push(name.to_string());
            bound = true;
        }
    }
    for binding in &element.events {
        match component.events.get(&binding.event) {
            None => problems.push(format!(
                "`{}` has no event `{}`",
                element.component, binding.event
            )),
            Some(event) => {
                if event.payload.is_none() && binding.arguments.iter().any(uses_event) {
                    problems.push(format!(
                        "`{}`'s `{}` has no payload for `$event`",
                        element.component, binding.event
                    ));
                }
            }
        }
        match own.commands.get(&binding.command) {
            // Not a command, but an event the component declares: the handler
            // forwards it, to the occurrence of the component, with its
            // payload (one argument if the event has one, else none).
            None if own.events.contains_key(&binding.command) => {
                let expected = usize::from(own.events[&binding.command].payload.is_some());
                if binding.arguments.len() != expected {
                    problems.push(format!(
                        "`{}` is an event of this component, and forwarding it takes {expected} arguments, not {}",
                        binding.command,
                        binding.arguments.len()
                    ));
                }
            }
            None => problems.push(format!("no command `{}`", binding.command)),
            Some(command) if command.parameters.len() != binding.arguments.len() => {
                problems.push(format!(
                    "`{}` takes {} arguments, not {}",
                    binding.command,
                    command.parameters.len(),
                    binding.arguments.len()
                ))
            }
            Some(_) => {}
        }
        for argument in &binding.arguments {
            scope_names(own, loops, argument, problems);
        }
    }
    for child in &element.children {
        match child {
            Child::Text { .. } => {}
            Child::Expression { expression } => scope_names(own, loops, expression, problems),
            Child::Element { element } => walk_names(manifest, own, element, loops, problems),
        }
    }
    if bound {
        loops.pop();
    }
}

/// The name a `mesh-each` binds: its `as`, a string literal.
pub(crate) fn repeat_name(element: &Element) -> Option<&str> {
    element
        .props
        .iter()
        .find_map(|prop| match (&prop.value, prop.prop.as_str()) {
            (
                Expression::Literal {
                    value: mesh_template::Literal::String(name),
                    ..
                },
                "as",
            ) => Some(name.as_str()),
            _ => None,
        })
}

/// What a slot must be: a place, with no events or children of its own, and at
/// most a `name`, a non-empty string literal, which makes it a named slot.
fn slot_problems(element: &Element, problems: &mut Vec<String>) {
    if element.props.iter().any(|prop| prop.prop != "name") {
        problems.push(format!("`{SLOT_COMPONENT}` has no props but `name`"));
    }
    if element.props.iter().any(|prop| prop.prop == "name") && slot_name(element).is_empty() {
        problems.push(format!(
            "the `name` of a `{SLOT_COMPONENT}` is a non-empty string literal"
        ));
    }
    if !element.events.is_empty() {
        problems.push(format!("`{SLOT_COMPONENT}` can't have events"));
    }
    if !element.children.is_empty() {
        problems.push(format!("`{SLOT_COMPONENT}` can't have children"));
    }
}

/// What a fill must be: for a named slot (`slot`, a non-empty string literal),
/// with no events. Its children are free; where it may be is the assembly's.
fn fill_problems(element: &Element, problems: &mut Vec<String>) {
    if element.props.iter().any(|prop| prop.prop != "slot") || fill_name(element).is_empty() {
        problems.push(format!(
            "`{FILL_COMPONENT}` has exactly one prop, `slot`, a non-empty string literal naming the slot it fills"
        ));
    }
    if !element.events.is_empty() {
        problems.push(format!("`{FILL_COMPONENT}` can't have events"));
    }
}

/// What a repeat must be (provisional): `items`, `key` and a literal `as`, no
/// events, and exactly one element child, which is neither a conditional nor
/// a repeat (nested dynamic structures are not part of the tracer).
fn repeat_problems(element: &Element, problems: &mut Vec<String>) {
    for needed in ["items", "key"] {
        if !element.props.iter().any(|prop| prop.prop == needed) {
            problems.push(format!("`{REPEAT_COMPONENT}` has no `{needed}`"));
        }
    }
    if repeat_name(element).is_none_or(str::is_empty) {
        problems.push(format!(
            "`{REPEAT_COMPONENT}` needs `as`, a non-empty string literal naming the item"
        ));
    }
    if !element.events.is_empty() {
        problems.push(format!("`{REPEAT_COMPONENT}` can't have events"));
    }
    let only_elements = element
        .children
        .iter()
        .all(|child| matches!(child, Child::Element { .. }));
    let items = alternatives(element);
    if !only_elements || items.len() != 1 {
        problems.push(format!(
            "`{REPEAT_COMPONENT}` needs exactly one element child, and nothing else"
        ));
    }
    if items.iter().any(|item| item.component == SLOT_COMPONENT) {
        problems.push(format!(
            "the child of `{REPEAT_COMPONENT}` can't be a `{SLOT_COMPONENT}`: put the slot inside an element"
        ));
    }
    if items
        .iter()
        .any(|item| item.component == CONDITIONAL_COMPONENT || item.component == REPEAT_COMPONENT)
    {
        problems.push(format!(
            "the child of `{REPEAT_COMPONENT}` can't be a `{CONDITIONAL_COMPONENT}` or a `{REPEAT_COMPONENT}`"
        ));
    }
}

/// What a conditional must be (provisional): a written `when`, no events, and
/// one or two element children, none of them a conditional.
fn conditional_problems(element: &Element, problems: &mut Vec<String>) {
    if !element.props.iter().any(|prop| prop.prop == "when") {
        problems.push(format!("`{CONDITIONAL_COMPONENT}` has no `when`"));
    }
    if !element.events.is_empty() {
        problems.push(format!("`{CONDITIONAL_COMPONENT}` can't have events"));
    }
    let only_elements = element
        .children
        .iter()
        .all(|child| matches!(child, Child::Element { .. }));
    let alternatives = alternatives(element);
    if !only_elements || !(1..=2).contains(&alternatives.len()) {
        problems.push(format!(
            "`{CONDITIONAL_COMPONENT}` needs one or two element children, and nothing else"
        ));
    }
    if alternatives
        .iter()
        .any(|alternative| alternative.component == SLOT_COMPONENT)
    {
        problems.push(format!(
            "an alternative of `{CONDITIONAL_COMPONENT}` can't be a `{SLOT_COMPONENT}`: put the slot inside an element"
        ));
    }
    if alternatives
        .iter()
        .any(|alternative| alternative.component == CONDITIONAL_COMPONENT)
    {
        problems.push(format!(
            "an alternative of `{CONDITIONAL_COMPONENT}` can't be a `{CONDITIONAL_COMPONENT}`"
        ));
    }
}

fn uses_event(expression: &Expression) -> bool {
    let mut found = false;
    visit(expression, &mut |e| {
        found |= matches!(e, Expression::Event { .. })
    });
    found
}

fn scope_names(
    own: &Component,
    loops: &[String],
    expression: &Expression,
    problems: &mut Vec<String>,
) {
    visit(expression, &mut |e| {
        if let Expression::Scope { name, .. } = e {
            if !own.scope.contains_key(name) && !loops.contains(name) {
                problems.push(format!("no scope name `{name}`"));
            }
        }
    });
}

/// Calls `f` on `expression` and every expression inside it.
pub(crate) fn visit(expression: &Expression, f: &mut impl FnMut(&Expression)) {
    f(expression);
    match expression {
        Expression::Literal { .. } | Expression::Scope { .. } | Expression::Event { .. } => {}
        Expression::Member { object, .. } => visit(object, f),
        Expression::Unary { operand, .. } => visit(operand, f),
        Expression::Binary { left, right, .. } => {
            visit(left, f);
            visit(right, f);
        }
        Expression::Conditional {
            condition,
            consequent,
            alternate,
            ..
        } => {
            visit(condition, f);
            visit(consequent, f);
            visit(alternate, f);
        }
        Expression::List { elements, .. } => elements.iter().for_each(|e| visit(e, f)),
        Expression::Record { fields, .. } => fields.iter().for_each(|field| visit(&field.value, f)),
    }
}

/// One event binding that a program's templates declare: the occurrence in
/// a template, whether or not any render currently contains it.
///
/// It is a fact about the validated program, not about a render: an event
/// inside a `mesh-if` alternative the snapshot doesn't choose, or inside a
/// `mesh-each` body whatever the items, is declared once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredEvent {
    /// The component whose template declares the binding: the component of
    /// the command's intent (docs/manual/runtime.md, "The command intent").
    pub component: String,
    /// The event of the element the binding is on.
    pub event: String,
    /// The command of `component` the event raises.
    pub command: String,
    /// The binding's span in the source of `component`'s template.
    pub span: mesh_template::Span,
}

/// Every event binding of every template of a validated program: templates
/// by component name, then each template's elements in document order.
/// Nothing is deduplicated.
pub(crate) fn declared_events(valid: &Valid) -> Vec<DeclaredEvent> {
    let mut found = Vec::new();
    for (component, template) in &valid.templates {
        let mut all = Vec::new();
        elements(&template.root, &mut all);
        let own = &valid.component(component);
        for element in all {
            for binding in &element.events {
                // A handler that forwards the component's own event is not a
                // command the host handles: the occurrence's binding is.
                if !own.commands.contains_key(&binding.command) {
                    continue;
                }
                found.push(DeclaredEvent {
                    component: component.clone(),
                    event: binding.event.clone(),
                    command: binding.command.clone(),
                    span: binding.span,
                });
            }
        }
    }
    found
}

/// The declared events as a JSON array, in the form docs/manual/runtime.md
/// describes ("The declared events"): each one's `component`, `event`,
/// `command` and `span`, in the order given.
pub fn declared_events_to_json(events: &[DeclaredEvent]) -> String {
    serde_json::Value::Array(
        events
            .iter()
            .map(|e| {
                json!({
                    "component": e.component,
                    "event": e.event,
                    "command": e.command,
                    "span": e.span,
                })
            })
            .collect(),
    )
    .to_string()
}

/// Every element of `element`'s tree, `element` included, in document
/// order.
fn elements<'t>(element: &'t Element, out: &mut Vec<&'t Element>) {
    out.push(element);
    for child in &element.children {
        if let Child::Element { element } = child {
            elements(element, out);
        }
    }
}

/// Step 4: the assembly rules (docs/manual/templates.md), every violation
/// of every rule, in the manual's order.
fn assembly(manifest: &Manifest, root: &str, read: &[(usize, Template)]) -> Vec<RuntimeDiagnostic> {
    let mut diagnostics = Vec::new();
    let source = |template: &Template, span| Location::Source {
        component: template.component.clone(),
        span,
    };

    // Rule 1: one template per component.
    let mut first: BTreeMap<&str, &Template> = BTreeMap::new();
    for (_, template) in read {
        if first.contains_key(template.component.as_str()) {
            diagnostics.push(RuntimeDiagnostic::new(
                RuntimeCode::DUPLICATE_TEMPLATE,
                format!("a second template for `{}`", template.component),
                source(template, template.root.span),
            ));
        } else {
            first.insert(&template.component, template);
        }
    }
    // Rule 2: the root's template.
    if !first.contains_key(root) {
        diagnostics.push(RuntimeDiagnostic::new(
            RuntimeCode::MISSING_ROOT,
            format!("the program has no template for its root, `{root}`"),
            Location::Program,
        ));
    }

    // Rules 4–7, for every composite occurrence in every template.
    let components = manifest.components();
    // Which composites each composite's template uses, for rule 5.
    let mut uses: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (_, template) in read {
        let mut all = Vec::new();
        elements(&template.root, &mut all);
        for element in all {
            let name = element.component.as_str();
            if !first.contains_key(name) {
                continue;
            }
            uses.entry(template.component.as_str())
                .or_default()
                .insert(name);
        }
    }
    let reaches = |from: &str, to: &str| {
        let mut seen = BTreeSet::new();
        let mut stack = vec![from];
        while let Some(at) = stack.pop() {
            if at == to {
                return true;
            }
            if seen.insert(at) {
                if let Some(next) = uses.get(at) {
                    stack.extend(next.iter().copied());
                }
            }
        }
        false
    };
    for (_, template) in read {
        let mut all = Vec::new();
        elements(&template.root, &mut all);
        for element in all {
            let name = element.component.as_str();
            if !first.contains_key(name) {
                continue;
            }
            let composite = &components[name];
            // Rules 4a, 4b: every scope name bound, soundly.
            for (scope_name, scope_type) in &composite.scope {
                match composite.props.get(scope_name) {
                    None => diagnostics.push(RuntimeDiagnostic::new(
                        RuntimeCode::UNBOUND_SCOPE_NAME,
                        format!("`{name}`'s scope name `{scope_name}` has no prop of that name to bind it"),
                        source(template, element.span),
                    )),
                    Some(prop) => {
                        let bound = if prop.required || matches!(manifest.expand(&prop.ty), Type::Optional(_)) {
                            prop.ty.clone()
                        } else {
                            Type::Optional(Box::new(prop.ty.clone()))
                        };
                        let assignable = mesh_analysis::is_assignable(
                            manifest,
                            &mesh_analysis::Ty::from(&bound),
                            &mesh_analysis::Ty::from(scope_type),
                        );
                        if !assignable {
                            diagnostics.push(RuntimeDiagnostic::new(
                                RuntimeCode::UNSOUND_BINDING,
                                format!(
                                    "`{name}`'s prop `{scope_name}` can't bind its scope name `{scope_name}`: {}",
                                    if prop.required { "its type isn't assignable" } else { "it's optional, and an unwritten prop binds absence" }
                                ),
                                source(template, element.span),
                            ));
                        }
                    }
                }
            }
            // Rule 5: no cycles.
            if reaches(name, &template.component) {
                diagnostics.push(RuntimeDiagnostic::new(
                    RuntimeCode::CYCLE,
                    format!(
                        "`{name}` expands `{}` again, through its template: a cycle",
                        template.component
                    ),
                    source(template, element.span),
                ));
            }
            // Rule 6: a composite's events and commands have different names,
            // so a handler in its template that names one of them is one or
            // the other, never both.
            for event in composite.events.keys() {
                if composite.commands.contains_key(event) {
                    diagnostics.push(RuntimeDiagnostic::new(
                        RuntimeCode::COMPOSITE_EVENT,
                        format!("`{name}` has a template, so it's a composite, and declares `{event}` as both an event and a command: a handler named `{event}` in its template would be either"),
                        source(template, element.span),
                    ));
                }
            }
            // Rule 7: children only where the composite's template has a slot
            // for them: loose ones (and text) go to the default slot, and each
            // `mesh-fill` to the slot of its name, once.
            let has_children = element.children.iter().any(|child| match child {
                Child::Text { value, .. } => !value.trim().is_empty(),
                Child::Element { element } => element.component != FILL_COMPONENT,
                Child::Expression { .. } => true,
            });
            let mut inside = Vec::new();
            elements(&first[name].root, &mut inside);
            let slots: BTreeSet<&str> = inside
                .iter()
                .filter(|inner| inner.component == SLOT_COMPONENT)
                .map(|inner| slot_name(inner))
                .collect();
            if has_children && !slots.contains("") {
                diagnostics.push(RuntimeDiagnostic::new(
                    RuntimeCode::COMPOSITE_CHILDREN,
                    format!(
                        "`{name}` is a composite whose template has no default `{SLOT_COMPONENT}`, so an occurrence of it can't have children outside a `{FILL_COMPONENT}`"
                    ),
                    source(template, element.span),
                ));
            }
            let mut filled = BTreeSet::new();
            for child in &element.children {
                let Child::Element { element: fill } = child else {
                    continue;
                };
                if fill.component != FILL_COMPONENT {
                    continue;
                }
                if !slots.contains(fill_name(fill)) {
                    diagnostics.push(RuntimeDiagnostic::new(
                        RuntimeCode::COMPOSITE_CHILDREN,
                        format!(
                            "`{name}`'s template has no `{SLOT_COMPONENT}` named `{}` for this `{FILL_COMPONENT}`",
                            fill_name(fill)
                        ),
                        source(template, fill.span),
                    ));
                } else if !filled.insert(fill_name(fill)) {
                    diagnostics.push(RuntimeDiagnostic::new(
                        RuntimeCode::COMPOSITE_CHILDREN,
                        format!(
                            "an occurrence of `{name}` fills the slot `{}` more than once",
                            fill_name(fill)
                        ),
                        source(template, fill.span),
                    ));
                }
            }
        }
    }
    // A fill belongs directly inside an occurrence of a composite.
    for (_, template) in read {
        let mut all = Vec::new();
        elements(&template.root, &mut all);
        for parent in all {
            if first.contains_key(parent.component.as_str()) {
                continue;
            }
            for child in &parent.children {
                if let Child::Element { element: fill } = child {
                    if fill.component == FILL_COMPONENT {
                        diagnostics.push(RuntimeDiagnostic::new(
                            RuntimeCode::COMPOSITE_CHILDREN,
                            format!(
                                "`{FILL_COMPONENT}` belongs directly inside an occurrence of a composite, not inside `{}`",
                                parent.component
                            ),
                            source(template, fill.span),
                        ));
                    }
                }
            }
        }
    }
    order(&mut diagnostics);
    diagnostics
}

/// Assembly diagnostics' order: the program's first, then templates' by
/// position, then sources' by component, span start and code.
fn order(diagnostics: &mut [RuntimeDiagnostic]) {
    diagnostics.sort_by(|a, b| {
        let key = |d: &RuntimeDiagnostic| match &d.location {
            Location::Program => (0, 0, String::new(), 0),
            Location::Template { index, .. } => (1, *index, String::new(), 0),
            Location::Source { component, span } => (2, 0, component.clone(), span.start.byte),
            _ => (3, 0, String::new(), 0),
        };
        key(a).cmp(&key(b)).then_with(|| a.code.cmp(b.code))
    });
}

// --- identities -------------------------------------------------------------

/// A string, as the manuals' layouts write one: its UTF-8 length as a
/// 32-bit big-endian count, then its bytes.
fn string(hash: &mut Sha256, text: &str) {
    count(hash, text.len());
    hash.update(text.as_bytes());
}

fn count(hash: &mut Sha256, n: usize) {
    hash.update(u32::try_from(n).expect("fewer than 2^32").to_be_bytes());
}

/// The program's identity: over its root and each template's canonical
/// digest, in code-point order of component.
fn identity(program: &Program<'_>) -> [u8; 32] {
    let mut digests: Vec<(String, [u8; 32])> = program
        .templates
        .iter()
        .map(|text| {
            let value: serde_json::Value = serde_json::from_str(text).expect("validated");
            let component = value["component"].as_str().expect("validated").to_string();
            (component, canonical_digest(&value))
        })
        .collect();
    digests.sort();
    let mut hash = Sha256::new();
    string(&mut hash, "mesh-program-v1");
    string(&mut hash, program.root);
    count(&mut hash, digests.len());
    for (component, digest) in &digests {
        string(&mut hash, component);
        hash.update(digest);
    }
    hash.finalize().into()
}

/// A template's canonical digest (docs/manual/templates.md): SHA-256 of
/// its RFC 8785 canonical JSON, as given, without `compiler`, numbers
/// written by §9.7.7.1.
pub(crate) fn canonical_digest(template: &serde_json::Value) -> [u8; 32] {
    let mut without = template.clone();
    if let Some(object) = without.as_object_mut() {
        object.remove("compiler");
    }
    let mut text = String::new();
    canonical(&without, &mut text);
    Sha256::digest(text.as_bytes()).into()
}

/// RFC 8785's canonical JSON of `value`.
pub(crate) fn canonical(value: &serde_json::Value, out: &mut String) {
    use serde_json::Value;
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
        Value::Number(number) => {
            out.push_str(&number_to_text(
                number.as_f64().expect("a JSON number is finite"),
            ));
        }
        Value::String(text) => canonical_string(text, out),
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                canonical(item, out);
            }
            out.push(']');
        }
        Value::Object(members) => {
            let mut members: Vec<(&String, &Value)> = members.iter().collect();
            members.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
            out.push('{');
            for (index, (name, value)) in members.into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                canonical_string(name, out);
                out.push(':');
                canonical(value, out);
            }
            out.push('}');
        }
    }
}

fn canonical_string(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// One step of a structural path: a position among the parent's
/// children in the render tree, a kind, and a component.
#[derive(Debug, Clone)]
pub(crate) struct Step {
    pub position: usize,
    pub kind: u8,
    pub component: String,
}

/// A node, a composite expansion, a text run, or a conditional alternative.
pub(crate) const NODE: u8 = 0x01;
pub(crate) const TEXT: u8 = 0x02;
pub(crate) const COMPOSITE: u8 = 0x03;
/// A conditional alternative (spec §9.10.2): one step for the alternative a
/// node is in, then the node's own step at position 0, as a composite
/// expansion adds two. The alternatives of one conditional are distinct
/// sites, so this step names which one, not where the node ended up.
pub(crate) const CONDITIONAL: u8 = 0x04;

/// PROVISIONAL, for the §9.10 conditional tracer: the one component the
/// runtime gives conditional meaning. It is declared in the model like any
/// other (its `when` prop, a boolean, is checked by the compiler as any
/// prop is), so no syntax, grammar or checker changed. It is never a node:
/// the render tree has what it chose, or nothing. Its spelling is not the
/// language's decision.
pub(crate) const CONDITIONAL_COMPONENT: &str = "mesh-if";

/// The names of a conditional's alternatives, in the order of its element
/// children: the first is chosen when `when` is true, the second, if there
/// is one, when it is false.
pub(crate) const ALTERNATIVES: [&str; 2] = ["consequent", "alternate"];

/// A conditional's alternatives: its element children, in order.
pub(crate) fn alternatives(conditional: &Element) -> Vec<&Element> {
    conditional
        .children
        .iter()
        .filter_map(|child| match child {
            Child::Element { element } => Some(element.as_ref()),
            _ => None,
        })
        .collect()
}

/// Whether any template of the program uses a conditional.
pub(crate) fn uses_conditional(templates: &BTreeMap<String, Template>) -> bool {
    templates.values().any(|template| {
        let mut all = Vec::new();
        elements(&template.root, &mut all);
        all.iter()
            .any(|element| element.component == CONDITIONAL_COMPONENT)
    })
}

/// A repeat's instance (spec §9.10.2): one step for the item's declared key,
/// then the node's own step at position 0. Unlike every other step it is
/// not a function of the template alone: `component` holds the key,
/// canonically, and the position is the repeat's slot, not the item's index.
pub(crate) const REPEAT: u8 = 0x05;

/// PROVISIONAL, for the §9.10 repeated-identity tracer: the component the
/// runtime gives repetition meaning, as `mesh-if` has conditionals. It is
/// declared in the model with props `items`, `as` and `key`; the compiler
/// binds `as` for `key` and the children, and the runtime evaluates `key`
/// per item. Never a node. Its spelling is not the language's decision.
pub(crate) const REPEAT_COMPONENT: &str = "mesh-each";

/// A slot inside a repeat's item, or a conditional's alternative, or in the
/// caller's content that a composite's template places: one step for the
/// slot, then the content's own steps, so that the same content placed by
/// two occurrences, or by one occurrence's two slots, is named by different paths.
pub(crate) const SLOT: u8 = 0x06;

/// The component the runtime gives a composite's **slot** meaning: where, in
/// a composite's template, the children of an occurrence of the composite are
/// placed. It is declared in the model like `mesh-if`, with no props. Never a
/// node: the render tree has the children, evaluated in the **caller's**
/// scope, where the slot was. A template has at most one (the default slot).
pub(crate) const SLOT_COMPONENT: &str = "mesh-slot";

/// The component that names the slot some of an occurrence's children go to:
/// `<card><mesh-fill slot="header">…</mesh-fill>…</card>`. Declared in the model
/// like `mesh-slot`, with a required string prop `slot`. Never a node, and only
/// ever a direct child of a composite occurrence; the slot it names is
/// `<mesh-slot name="header" />` in the composite's template.
pub(crate) const FILL_COMPONENT: &str = "mesh-fill";

/// The string literal written for `prop` on `element`, if there is one.
fn literal<'e>(element: &'e Element, prop: &str) -> Option<&'e str> {
    element
        .props
        .iter()
        .find_map(|written| match &written.value {
            Expression::Literal {
                value: mesh_template::Literal::String(text),
                ..
            } if written.prop == prop => Some(text.as_str()),
            _ => None,
        })
}

/// The name of a slot: its `name`, or `""` for the default slot.
pub(crate) fn slot_name(slot: &Element) -> &str {
    literal(slot, "name").unwrap_or("")
}

/// The name of the slot a fill is for: its `slot`.
pub(crate) fn fill_name(fill: &Element) -> &str {
    literal(fill, "slot").unwrap_or("")
}

/// The fill an occurrence has for the slot `name`, if any.
pub(crate) fn fill_for<'e>(occurrence: &'e Element, name: &str) -> Option<&'e Element> {
    occurrence.children.iter().find_map(|child| match child {
        Child::Element { element }
            if element.component == FILL_COMPONENT && fill_name(element) == name =>
        {
            Some(element.as_ref())
        }
        _ => None,
    })
}

/// Whether any template of the program uses a repeat.
pub(crate) fn uses_repeat(templates: &BTreeMap<String, Template>) -> bool {
    templates.values().any(|template| {
        let mut all = Vec::new();
        elements(&template.root, &mut all);
        all.iter()
            .any(|element| element.component == REPEAT_COMPONENT)
    })
}

/// The key at `path` in the program whose identity is `identity`.
pub(crate) fn key(identity: &[u8; 32], path: &[Step]) -> String {
    KeyPrefix::new(identity, path.len(), path).key(&[])
}

/// The hash of a key's path up to some step, so that the keys of siblings,
/// which share the path above them, are each finished from it and not hashed
/// from the root. The bytes are exactly those [`key`] hashes: `total` is the
/// length of the whole path, which the hash states before any step.
#[derive(Clone)]
pub(crate) struct KeyPrefix {
    hash: Sha256,
}

impl KeyPrefix {
    pub(crate) fn new(identity: &[u8; 32], total: usize, steps: &[Step]) -> KeyPrefix {
        let mut hash = Sha256::new();
        string(&mut hash, "mesh-key-v1");
        hash.update(identity);
        count(&mut hash, total);
        for step in steps {
            absorb(&mut hash, step);
        }
        KeyPrefix { hash }
    }

    /// The key of the path whose remaining steps are `rest`.
    pub(crate) fn key(&self, rest: &[Step]) -> String {
        let mut hash = self.hash.clone();
        for step in rest {
            absorb(&mut hash, step);
        }
        let digest: [u8; 32] = hash.finalize().into();
        format!("k{}", base64url(&digest[..16]))
    }
}

fn absorb(hash: &mut Sha256, step: &Step) {
    count(hash, step.position);
    hash.update([step.kind]);
    string(hash, &step.component);
}

/// The first part of every handler identifier of a program.
pub(crate) fn handler_prefix(identity: &[u8; 32]) -> String {
    format!("h{}", base64url(&identity[..8]))
}

/// The handler identifier of the handler of `event` at the node `key`.
pub(crate) fn handler(identity: &[u8; 32], key: &str, event: &str) -> String {
    let mut hash = Sha256::new();
    string(&mut hash, "mesh-handler-v1");
    hash.update(identity);
    string(&mut hash, key);
    string(&mut hash, event);
    let digest: [u8; 32] = hash.finalize().into();
    format!("{}.{}", handler_prefix(identity), base64url(&digest[..16]))
}

/// Unpadded base64url (RFC 4648 §5).
fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, byte)| n | u32::from(*byte) << (16 - 8 * i));
        let chars = chunk.len() + 1;
        for i in 0..chars {
            out.push(char::from(ALPHABET[(n >> (18 - 6 * i) & 63) as usize]));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64url_is_rfc_4648s() {
        assert_eq!(base64url(b""), "");
        assert_eq!(base64url(b"f"), "Zg");
        assert_eq!(base64url(b"fo"), "Zm8");
        assert_eq!(base64url(b"foo"), "Zm9v");
        assert_eq!(base64url(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64url(&[0xfb, 0xff]), "-_8");
        assert_eq!(base64url(&[0; 16]).len(), 22);
        assert_eq!(base64url(&[0; 8]).len(), 11);
    }

    #[test]
    fn canonical_json_is_rfc_8785s() {
        let value: serde_json::Value = serde_json::from_str(
            r#"{"b": [1, 2.50, 1e21, 0.000001, "é\n\u0001"], "a": null, "€": true, "😀": false}"#,
        )
        .unwrap();
        let mut text = String::new();
        canonical(&value, &mut text);
        // Keys by UTF-16 code units: "a", "b", "€" (U+20AC), then "😀"
        // (a surrogate pair, D83D, which sorts after 20AC).
        assert_eq!(
            text,
            "{\"a\":null,\"b\":[1,2.5,1e+21,0.000001,\"é\\n\\u0001\"],\"€\":true,\"😀\":false}"
        );
    }
}
