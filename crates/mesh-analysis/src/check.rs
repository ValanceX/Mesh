//! The analysis walk: one pass over the IR, from the root element down,
//! in source order within each element. It resolves names, types every
//! expression, and records facts as it goes.

use crate::relation::{self, is_assignable, join};
use crate::types::{FieldTy, Ty};
use crate::{
    Analysis, BlockPart, Combination, Expectation, Fact, Operator, Resolution, Target, Typed,
};
use mesh_manifest::{Component, Manifest, Template, Type};
use mesh_semantic::{AttributeValue, Child, Construct, Element, Expression, Literal, ObjectMember};
use mesh_syntax::{BinaryOperator, Span, UnaryOperator};
use std::collections::BTreeMap;

pub(crate) fn analyze(ir: &Element, template: Template<'_>) -> Analysis {
    let mut walker = Walker::new(template);
    walker.element(ir);
    walker.finish()
}

pub(crate) fn analyze_expression(expression: &Expression, template: Template<'_>) -> Analysis {
    let mut walker = Walker::new(template);
    walker.expression(expression, Place::Value);
    walker.finish()
}

impl<'m> Walker<'m> {
    fn new(template: Template<'m>) -> Self {
        Walker {
            template,
            facts: Vec::new(),
            resolutions: Vec::new(),
            types: Vec::new(),
            loops: Vec::new(),
            parents: Vec::new(),
            snippets: Vec::new(),
            snippets_changed: false,
        }
    }

    fn finish(self) -> Analysis {
        let Walker {
            mut facts,
            resolutions,
            types,
            ..
        } = self;
        // Stable, so facts that start at the same byte keep the order the
        // walk found them in.
        facts.sort_by_key(|fact| fact.span().start_byte);
        Analysis {
            facts,
            resolutions,
            types,
        }
    }
}

/// Where an expression sits, which decides whether `$event` may appear in
/// it. A command may only be a handler's whole expression, which
/// [`Walker::handler`] checks before any expression is walked, so every
/// command the walk meets as an expression is misplaced.
#[derive(Clone, Copy)]
enum Place<'m> {
    /// Anywhere but inside a handler command's arguments.
    Value,
    /// Inside the arguments of an `on.` handler's command.
    HandlerArgument(Payload<'m>),
}

/// What `$event` is in a handler.
#[derive(Clone, Copy)]
enum Payload<'m> {
    /// The event declares a payload of this type.
    Value(&'m Type),
    /// The event is declared without a payload.
    None { component: &'m str, event: &'m str },
    /// The event, or the element's component, isn't declared. That is
    /// already reported, so `$event` is accepted silently.
    Unknown,
}

struct Walker<'m> {
    template: Template<'m>,
    facts: Vec<Fact>,
    resolutions: Vec<Resolution>,
    types: Vec<Typed>,
    /// PROVISIONAL (§9.10 repeated-identity tracer): the names bound by the
    /// enclosing `mesh-each` elements, innermost last, each with its item
    /// type. A name here shadows a scope declaration and resolves to
    /// nothing a tool could navigate to.
    loops: Vec<(String, Ty, Span)>,
    /// The tags of the elements being walked, innermost last, the current
    /// element not among them.
    parents: Vec<String>,
    /// The snippets declared among the children being walked, and among the children around them,
    /// innermost last: each a name for a piece of template, whose parameters' types are the ones
    /// its render sites give.
    snippets: Vec<Local>,
    /// Whether a render site has widened a local snippet's parameter type since the last time its
    /// body was checked (the checks are repeated until none does).
    snippets_changed: bool,
}

/// A snippet declared among some children, and not passed to a component.
struct Local {
    name: String,
    name_span: Span,
    params: Vec<(String, Span)>,
    /// What the render sites give each parameter, joined; `None` while no render has.
    inferred: Vec<Option<Ty>>,
}

/// What `declare_snippets` found among an element's children.
struct Declared<'e> {
    /// How long `snippets` was before.
    base: usize,
    /// The local snippets, as their place in `snippets` and their declaration.
    local: Vec<(usize, &'e Element)>,
    /// The snippets passed as props: the declaration and the prop.
    passed: Vec<(&'e Element, String)>,
}

/// The reserved elements that blocks replaced. Written as elements they are reported while lowering, so a missing
/// declaration is not reported again.
const RETIRED: [&str; 5] = [
    "mesh-if",
    "mesh-each",
    "mesh-switch",
    "mesh-case",
    "mesh-default",
];

impl<'m> Walker<'m> {
    fn manifest(&self) -> &'m Manifest {
        self.template.manifest()
    }

    fn resolve(&mut self, span: Span, target: Target) {
        self.resolutions.push(Resolution { span, target });
    }

    /// Checks one element as an instance of the component its tag names,
    /// then its attribute values, its handlers and its children.
    fn element(&mut self, element: &Element) {
        if element.construct == Some(Construct::Render) {
            self.render_site(element);
            return;
        }
        let manifest = self.manifest();
        // A block lowered to this element: it is the language's, not a component of the manifest's, so no tool is
        // offered it and nothing about it resolves.
        let tracked = element.construct.is_none();
        let component = match element.construct {
            // Declared, a manifest's own is used; otherwise the language's.
            Some(_) => manifest
                .component(&element.name)
                .or_else(|| mesh_manifest::construct_component(&element.name)),
            None => match manifest.component(&element.name) {
                Some((name, component)) => {
                    self.resolve(element.name_span, Target::Component(name.to_string()));
                    Some((name, component))
                }
                None => {
                    if !RETIRED.contains(&element.name.as_str()) {
                        self.facts.push(Fact::UnknownComponent {
                            name: element.name.clone(),
                            span: element.name_span,
                            candidates: manifest.components().keys().cloned().collect(),
                        });
                    }
                    None
                }
            },
        };

        let declared = self.declare_snippets(element, component, tracked);
        if let Some((name, component)) = component {
            self.missing_props(element, name, component);
            self.children_conflict(element, name, component);
        }

        let repeating = element.construct == Some(Construct::Each);
        let mut pushed = false;
        let mut bound: Option<(String, Span)> = None;
        let mut attributes: Vec<&_> = element.attributes.iter().collect();
        if repeating {
            // `items` first, so `key` sees the binding whatever the order written.
            attributes.sort_by_key(|attribute| attribute.name != "items");
            bound = element.attributes.iter().find_map(|attribute| {
                match (&attribute.value, attribute.name.as_str()) {
                    (AttributeValue::String { value, span }, "as") => Some((value.clone(), *span)),
                    _ => None,
                }
            });
        }
        for attribute in attributes {
            // The prop's declared type, if the prop resolves.
            let mut expected = None;
            if let Some((name, component)) = component {
                match component.props.get_key_value(&attribute.name) {
                    Some((prop, field)) => {
                        if tracked {
                            self.resolve(
                                attribute.name_span,
                                Target::Prop {
                                    component: name.to_string(),
                                    prop: prop.clone(),
                                },
                            );
                        }
                        let expectation = match (element.construct, prop.as_str()) {
                            (Some(Construct::If | Construct::Case { first: true }), _) => {
                                Expectation::Block(BlockPart::IfCondition)
                            }
                            (Some(Construct::Case { first: false }), _) => {
                                Expectation::Block(BlockPart::ElseIfCondition)
                            }
                            (Some(Construct::Each), _) => Expectation::Block(BlockPart::EachItems),
                            _ => Expectation::Prop {
                                component: name.to_string(),
                                prop: prop.clone(),
                            },
                        };
                        expected = Some((Ty::from(&field.ty), expectation));
                    }
                    None => self.facts.push(Fact::UnknownProp {
                        component: name.to_string(),
                        prop: attribute.name.clone(),
                        span: attribute.name_span,
                        candidates: component.props.keys().cloned().collect(),
                    }),
                }
            }
            if repeating && attribute.name == "key" {
                if let AttributeValue::Expression(expression) = &attribute.value {
                    // Typed alone: the prop's declared type can't say
                    // "string or number", so the repeat's rule is the check.
                    if let Some(ty) = self.expression(expression, Place::Value) {
                        self.repeat_key(ty, expression.span());
                    }
                    continue;
                }
            }
            match (&attribute.value, expected) {
                (AttributeValue::String { value, span }, Some((expected, expectation))) => {
                    // Where an enum is expected, a quoted value is a literal site (T3).
                    if self.variant(value, *span, &expected).is_none() {
                        self.expect(Ty::String, *span, expectation, &expected);
                    }
                }
                (AttributeValue::String { .. }, None) => {}
                (AttributeValue::Expression(expression), Some((expected, expectation))) => {
                    let ty = self.check(expression, Place::Value, &expected, expectation);
                    if repeating && attribute.name == "items" {
                        if let (Some((name, declaration)), Some(ty)) = (&bound, ty) {
                            let ty = self.item_of(&ty);
                            // The binding is a declaration, not a read: it resolves to itself and has the item's type.
                            self.resolve(
                                *declaration,
                                Target::Binding {
                                    name: name.clone(),
                                    declaration: *declaration,
                                },
                            );
                            self.typed(*declaration, ty.clone());
                            self.loops.push((name.clone(), ty, *declaration));
                            pushed = true;
                        }
                    }
                }
                (AttributeValue::Expression(expression), None) => {
                    self.expression(expression, Place::Value);
                }
            }
        }

        for binding in &element.event_bindings {
            let payload = match component {
                Some((name, component)) => match component.events.get_key_value(&binding.name) {
                    Some((event, declared)) => {
                        self.resolve(
                            binding.name_span,
                            Target::Event {
                                component: name.to_string(),
                                event: event.clone(),
                            },
                        );
                        match &declared.payload {
                            Some(ty) => Payload::Value(ty),
                            None => Payload::None {
                                component: name,
                                event,
                            },
                        }
                    }
                    None => {
                        self.facts.push(Fact::UnknownEvent {
                            component: name.to_string(),
                            event: binding.name.clone(),
                            span: binding.name_span,
                            candidates: component.events.keys().cloned().collect(),
                        });
                        Payload::Unknown
                    }
                },
                None => Payload::Unknown,
            };
            self.handler(&binding.name, &binding.handler, payload);
        }

        self.parents.push(element.name.clone());
        self.walk_children(element);
        self.parents.pop();
        self.finish_snippets(declared, component);
        if pushed {
            self.loops.pop();
        }
    }

    /// The content of `element`: text, interpolations and elements. A snippet declaration is not
    /// content; `finish_snippets` checks it.
    fn walk_children(&mut self, element: &Element) {
        for child in &element.children {
            match child {
                Child::Text { .. } => {}
                Child::Expression(expression) => {
                    if let Some(ty) = self.expression(expression, Place::Value) {
                        self.content(&ty, expression.span());
                    }
                }
                Child::Element(child) if child.construct == Some(Construct::Snippet) => {}
                Child::Element(child) => self.element(child),
            }
        }
    }

    /// A snippet declaration's name and its parameters, as lowering wrote them.
    fn snippet_parts(declaration: &Element) -> (String, Span, Vec<(String, Span)>) {
        let mut name = (String::new(), declaration.name_span);
        let mut params = Vec::new();
        for attribute in &declaration.attributes {
            match (&attribute.value, attribute.name.as_str()) {
                (AttributeValue::String { value, span }, "name") => name = (value.clone(), *span),
                (AttributeValue::Expression(Expression::Array { elements, .. }), "params") => {
                    for element in elements {
                        if let Expression::Literal {
                            value: Literal::String(text),
                            span,
                        } = element
                        {
                            params.push((text.clone(), *span));
                        }
                    }
                }
                _ => {}
            }
        }
        (name.0, name.1, params)
    }

    /// Registers the snippets declared among `element`'s children: a declaration whose name is a prop
    /// of the component the element is an occurrence of is passed as that prop; any other is a local
    /// name, readable among all of `element`'s children (and in its own attributes, to pass it along).
    fn declare_snippets<'e>(
        &mut self,
        element: &'e Element,
        component: Option<(&str, &Component)>,
        tracked: bool,
    ) -> Declared<'e> {
        let mut declared = Declared {
            base: self.snippets.len(),
            local: Vec::new(),
            passed: Vec::new(),
        };
        let mut seen: Vec<String> = Vec::new();
        for child in &element.children {
            let Child::Element(child) = child else {
                continue;
            };
            if child.construct != Some(Construct::Snippet) {
                continue;
            }
            let (name, name_span, params) = Self::snippet_parts(child);
            if seen.contains(&name) {
                self.facts.push(Fact::DuplicateSnippet {
                    name: name.clone(),
                    span: name_span,
                });
            }
            seen.push(name.clone());
            match component {
                Some((_, component)) if tracked && component.props.contains_key(&name) => {
                    declared.passed.push((child, name));
                }
                _ => {
                    let inferred = vec![None; params.len()];
                    declared.local.push((self.snippets.len(), child));
                    self.snippets.push(Local {
                        name,
                        name_span,
                        params,
                        inferred,
                    });
                }
            }
        }
        declared
    }

    /// Checks the bodies of the snippets `declared` found, and forgets the local names.
    fn finish_snippets(&mut self, declared: Declared<'_>, component: Option<(&str, &Component)>) {
        let name = component.map(|(name, _)| name.to_string());
        for (declaration, prop) in &declared.passed {
            let (_, name_span, params) = Self::snippet_parts(declaration);
            let component_name = name.clone().unwrap_or_default();
            let field = component
                .and_then(|(_, component)| component.props.get(prop))
                .expect("a passed snippet names a prop");
            self.resolve(
                name_span,
                Target::Prop {
                    component: component_name.clone(),
                    prop: prop.clone(),
                },
            );
            let expected = Ty::from(&field.ty);
            let wanted = match relation::expand(self.manifest(), &expected) {
                Ty::Optional(inner) => relation::expand(self.manifest(), &inner),
                other => other,
            };
            let types = match &wanted {
                Ty::Snippet(parameters) => {
                    if parameters.len() != params.len() {
                        self.facts.push(Fact::SnippetArity {
                            name: prop.clone(),
                            expected: parameters.len(),
                            found: params.len(),
                            span: name_span,
                        });
                    }
                    parameters.clone()
                }
                _ => {
                    let given = Ty::Snippet(vec![Ty::Any; params.len()]);
                    self.expect(
                        given,
                        name_span,
                        Expectation::Prop {
                            component: component_name,
                            prop: prop.clone(),
                        },
                        &expected,
                    );
                    Vec::new()
                }
            };
            let types: Vec<Ty> = (0..params.len())
                .map(|index| types.get(index).cloned().unwrap_or(Ty::Any))
                .collect();
            self.snippet_body(declaration, &params, &types);
        }
        // The local snippets' parameters are what their render sites give them, and a body can
        // render itself and the others, so the bodies are checked again until no render widens a
        // parameter (what an earlier round reported is dropped, and found again if it still holds).
        for round in 0..4 {
            let marks = (self.facts.len(), self.resolutions.len(), self.types.len());
            self.snippets_changed = false;
            for (index, declaration) in &declared.local {
                let params = self.snippets[*index].params.clone();
                let types: Vec<Ty> = self.snippets[*index]
                    .inferred
                    .iter()
                    .map(|ty| ty.clone().unwrap_or(Ty::Any))
                    .collect();
                self.snippet_body(declaration, &params, &types);
            }
            if !self.snippets_changed || round == 3 {
                break;
            }
            self.facts.truncate(marks.0);
            self.resolutions.truncate(marks.1);
            self.types.truncate(marks.2);
        }
        self.snippets.truncate(declared.base);
    }

    /// Checks the body of the snippet `declaration` with its parameters bound to `types`.
    fn snippet_body(&mut self, declaration: &Element, params: &[(String, Span)], types: &[Ty]) {
        for ((name, span), ty) in params.iter().zip(types) {
            // A parameter is a declaration: it resolves to itself and has the type it is given.
            self.resolve(
                *span,
                Target::Binding {
                    name: name.clone(),
                    declaration: *span,
                },
            );
            self.typed(*span, ty.clone());
            self.loops.push((name.clone(), ty.clone(), *span));
        }
        let inner = self.declare_snippets(declaration, None, false);
        self.parents.push(declaration.name.clone());
        self.walk_children(declaration);
        self.parents.pop();
        self.finish_snippets(inner, None);
        for _ in params {
            self.loops.pop();
        }
    }

    /// `{@render name(arguments)}`: the snippet it names must be one declared among the children it
    /// is in or around (its arguments then give its parameters their types), or a snippet-typed name
    /// of the template's scope (its arguments are then checked against its parameters).
    fn render_site(&mut self, site: &Element) {
        let mut name = (String::new(), site.name_span);
        let mut arguments: &[Expression] = &[];
        for attribute in &site.attributes {
            match (&attribute.value, attribute.name.as_str()) {
                (AttributeValue::String { value, span }, "name") => name = (value.clone(), *span),
                (AttributeValue::Expression(Expression::Array { elements, .. }), "arguments") => {
                    arguments = elements;
                }
                _ => {}
            }
        }
        let (name, name_span) = name;

        if let Some(index) = self.snippets.iter().rposition(|local| local.name == name) {
            let (expected, declaration) = {
                let local = &self.snippets[index];
                (local.params.len(), local.name_span)
            };
            self.resolve(
                name_span,
                Target::Binding {
                    name: name.clone(),
                    declaration,
                },
            );
            if arguments.len() != expected {
                self.facts.push(Fact::SnippetArity {
                    name,
                    expected,
                    found: arguments.len(),
                    span: site.span,
                });
                for argument in arguments {
                    self.expression(argument, Place::Value);
                }
                return;
            }
            for (position, argument) in arguments.iter().enumerate() {
                if let Some(ty) = self.expression(argument, Place::Value) {
                    self.give(index, position, ty, argument.span());
                }
            }
            return;
        }

        let scope = &self.template.component().scope;
        let snippet_named = scope.get_key_value(&name).and_then(|(key, declared)| {
            let ty = Ty::from(declared);
            let present = match relation::expand(self.template.manifest(), &ty) {
                Ty::Optional(inner) => relation::expand(self.template.manifest(), &inner),
                other => other,
            };
            match present {
                Ty::Snippet(parameters) => Some((key.clone(), parameters)),
                _ => None,
            }
        });
        match snippet_named {
            Some((key, parameters)) => {
                self.resolve(name_span, Target::Scope(key));
                if arguments.len() != parameters.len() {
                    self.facts.push(Fact::SnippetArity {
                        name,
                        expected: parameters.len(),
                        found: arguments.len(),
                        span: site.span,
                    });
                    for argument in arguments {
                        self.expression(argument, Place::Value);
                    }
                    return;
                }
                for (position, (argument, parameter)) in
                    arguments.iter().zip(&parameters).enumerate()
                {
                    let expectation = Expectation::SnippetArgument {
                        snippet: name.clone(),
                        index: position,
                    };
                    self.check(argument, Place::Value, parameter, expectation);
                }
            }
            None => {
                let mut candidates: Vec<String> = self
                    .snippets
                    .iter()
                    .map(|local| local.name.clone())
                    .collect();
                candidates.extend(scope.iter().filter_map(|(key, declared)| {
                    let present = match declared {
                        Type::Optional(inner) => inner.as_ref(),
                        other => other,
                    };
                    matches!(present, Type::Snippet(_)).then(|| key.clone())
                }));
                self.facts.push(Fact::UnknownSnippet {
                    name,
                    span: name_span,
                    candidates,
                });
                for argument in arguments {
                    self.expression(argument, Place::Value);
                }
            }
        }
    }

    /// A render gives the local snippet `index` the argument of type `ty` for parameter `position`:
    /// the parameter's type becomes the common type of everything it was given. An argument that has
    /// none with an earlier one is reported at the argument.
    fn give(&mut self, index: usize, position: usize, ty: Ty, span: Span) {
        if matches!(ty, Ty::Snippet(_)) {
            let snippet = self.snippets[index].name.clone();
            self.expect(
                ty,
                span,
                Expectation::SnippetArgument {
                    snippet,
                    index: position,
                },
                &Ty::Any,
            );
            return;
        }
        let previous = self.snippets[index].inferred[position].clone();
        match previous {
            None => {
                self.snippets[index].inferred[position] = Some(ty);
                self.snippets_changed = true;
            }
            Some(previous) => match join(self.manifest(), &previous, &ty) {
                Some(joined) => {
                    if joined != previous {
                        self.snippets[index].inferred[position] = Some(joined);
                        self.snippets_changed = true;
                    }
                }
                None => self.facts.push(Fact::NoCommonType {
                    combination: Combination::SnippetArgument,
                    left: previous,
                    right: ty,
                    span,
                }),
            },
        }
    }

    /// `children={…}` together with content between the tags of a component that takes `children`:
    /// two answers to one prop.
    fn children_conflict(&mut self, element: &Element, name: &str, component: &Component) {
        if !component.props.contains_key("children") {
            return;
        }
        let loose = element.children.iter().any(|child| match child {
            Child::Element(child) => child.construct != Some(Construct::Snippet),
            _ => true,
        });
        if let Some(attribute) = element
            .attributes
            .iter()
            .find(|attribute| attribute.name == "children")
        {
            if loose {
                self.facts.push(Fact::ChildrenConflict {
                    component: name.to_string(),
                    span: attribute.name_span,
                });
            }
        }
    }

    /// A repeat's key must be a string or a finite number (§9.10.4). The
    /// finiteness is a value's, so statically the type must be `string` or
    /// `number`, or `any`, which every check leaves to the runtime. A
    /// possibly absent key, and every other type, is refused here; the
    /// runtime refuses what a type can't show (`runtime-invalid-key`).
    fn repeat_key(&mut self, ty: Ty, span: Span) {
        let manifest = self.manifest();
        let present = match relation::expand(manifest, &ty) {
            Ty::Optional(inner) => relation::expand(manifest, &inner),
            other => other,
        };
        let expected = if present == Ty::Number {
            Ty::Number
        } else {
            Ty::String
        };
        self.expect(ty, span, Expectation::RepeatKey, &expected);
    }

    /// The type of one item of a list type (an optional list's too); `any`
    /// when the type isn't a list, which the prop check has already reported.
    fn item_of(&self, ty: &Ty) -> Ty {
        let expanded = relation::expand(self.manifest(), ty);
        let list = match expanded {
            Ty::Optional(inner) => relation::expand(self.manifest(), &inner),
            other => other,
        };
        match list {
            Ty::List(item) => *item,
            _ => Ty::Any,
        }
    }

    /// Content needs a text form (§9.7.8): a value whose type, with named
    /// references expanded and one optional stripped, is a list or a
    /// record has none. `any` does, statically; the runtime checks its
    /// value.
    fn content(&mut self, ty: &Ty, span: Span) {
        let present = match relation::expand(self.manifest(), ty) {
            Ty::Optional(inner) => relation::expand(self.manifest(), &inner),
            other => other,
        };
        if matches!(present, Ty::List(_) | Ty::Record(_) | Ty::Snippet(_)) {
            self.facts.push(Fact::ContentNotText {
                ty: ty.clone(),
                span,
            });
        }
    }

    /// Every prop `component` declares required that `element` doesn't
    /// supply, in the manifest's order, each at the tag name.
    fn missing_props(&mut self, element: &Element, name: &str, component: &Component) {
        for (prop, field) in &component.props {
            let supplied = element
                .attributes
                .iter()
                .any(|attribute| &attribute.name == prop)
                // A snippet declared among the children is passed as the prop of its name, and content
                // between the tags is the prop `children`.
                || element.children.iter().any(|child| match child {
                    Child::Element(child) if child.construct == Some(Construct::Snippet) => {
                        Self::snippet_parts(child).0 == *prop
                    }
                    Child::Element(_) | Child::Expression(_) => prop == "children",
                    Child::Text { .. } => prop == "children",
                });
            // A repeat without a key is reported while lowering, with a message of its own.
            let reported = element.construct == Some(Construct::Each) && prop == "key";
            if field.required && !supplied && !reported {
                self.facts.push(Fact::MissingRequiredProp {
                    component: name.to_string(),
                    prop: prop.clone(),
                    span: element.name_span,
                });
            }
        }
    }

    /// An `on.` handler must be exactly one command invocation. Anything
    /// else is reported once and not looked inside.
    fn handler(&mut self, event: &str, handler: &Expression, payload: Payload<'m>) {
        match handler {
            Expression::Command {
                command,
                command_span,
                arguments,
                span,
            } => {
                self.command(
                    command,
                    *command_span,
                    arguments,
                    *span,
                    Place::HandlerArgument(payload),
                );
                self.typed(*span, Ty::Void);
            }
            other => self.facts.push(Fact::HandlerNotCommand {
                event: event.to_string(),
                span: other.span(),
            }),
        }
    }

    /// A command in handler position: resolves its name and checks its
    /// arity, then checks each argument against its parameter's type.
    /// With the wrong number of arguments, or an unknown command, the
    /// arguments are only typed.
    fn command(
        &mut self,
        command: &str,
        command_span: Span,
        arguments: &[Expression],
        span: Span,
        place: Place<'m>,
    ) {
        let component = self.template.component();
        let commands = &component.commands;
        let parameters = match commands.get_key_value(command) {
            Some((name, declared)) => {
                self.resolve(command_span, Target::Command(name.clone()));
                if declared.parameters.len() == arguments.len() {
                    Some(
                        declared
                            .parameters
                            .iter()
                            .map(|parameter| (parameter.name.clone(), parameter.ty.clone()))
                            .collect::<Vec<_>>(),
                    )
                } else {
                    self.facts.push(Fact::CommandArityMismatch {
                        command: command.to_string(),
                        expected: declared.parameters.len(),
                        found: arguments.len(),
                        span,
                    });
                    None
                }
            }
            // Not a command, but an event the template's own component
            // declares: the handler forwards that event to the occurrence of
            // the component, with its argument (if the event has a payload)
            // as the payload. Handled where the occurrence binds it.
            None if component.events.contains_key(command) => {
                let (name, declared) = component
                    .events
                    .get_key_value(command)
                    .expect("checked just above");
                self.resolve(
                    command_span,
                    Target::Event {
                        component: self.template.name().to_string(),
                        event: name.clone(),
                    },
                );
                let expected = usize::from(declared.payload.is_some());
                if expected == arguments.len() {
                    Some(
                        declared
                            .payload
                            .iter()
                            .map(|payload| ("payload".to_string(), payload.clone()))
                            .collect::<Vec<_>>(),
                    )
                } else {
                    self.facts.push(Fact::CommandArityMismatch {
                        command: command.to_string(),
                        expected,
                        found: arguments.len(),
                        span,
                    });
                    None
                }
            }
            None => {
                self.facts.push(Fact::UnknownCommand {
                    command: command.to_string(),
                    span: command_span,
                    candidates: commands
                        .keys()
                        .chain(component.events.keys())
                        .cloned()
                        .collect(),
                });
                None
            }
        };
        match &parameters {
            Some(parameters) => {
                for (argument, (name, ty)) in arguments.iter().zip(parameters) {
                    let expectation = Expectation::Argument {
                        command: command.to_string(),
                        parameter: name.clone(),
                    };
                    self.check(argument, place, &Ty::from(ty), expectation);
                }
            }
            None => {
                for argument in arguments {
                    self.expression(argument, place);
                }
            }
        }
    }

    /// Checks `expression` where a value of type `expected` is expected
    /// (outline D9): types it, and reports a [`Fact::TypeMismatch`] unless
    /// its type is assignable to `expected`. The expected type is pushed
    /// into literals instead, with the same relation: an object literal
    /// where a record is expected is checked field by field, an array
    /// literal where a list is expected element by element, and a
    /// conditional branch by branch. Returns the expression's type, if it
    /// has one.
    fn check(
        &mut self,
        expression: &Expression,
        place: Place<'m>,
        expected: &Ty,
        expectation: Expectation,
    ) -> Option<Ty> {
        // A literal is present, so where a record or list that may be
        // absent is expected, the record or list itself is (rule 2).
        let present = || match relation::expand(self.manifest(), expected) {
            Ty::Optional(inner) => *inner,
            _ => expected.clone(),
        };
        match expression {
            // A string literal where an enum is expected is checked for membership (T3).
            Expression::Literal {
                value: Literal::String(text),
                span,
            } if self.enum_values(&present()).is_some() => {
                return self.variant(text, *span, &present()).flatten();
            }
            // A local snippet passed along where a snippet is expected is given the parameters the
            // expected one is called with.
            Expression::Reference { name, span }
                if !self.loops.iter().any(|(bound, _, _)| bound == name)
                    && self.snippets.iter().any(|local| &local.name == name) =>
            {
                let index = self
                    .snippets
                    .iter()
                    .rposition(|local| &local.name == name)
                    .expect("just found");
                if let Ty::Snippet(parameters) = relation::expand(self.manifest(), &present()) {
                    if parameters.len() == self.snippets[index].params.len() {
                        for (position, parameter) in parameters.iter().enumerate() {
                            self.give(index, position, parameter.clone(), *span);
                        }
                    }
                }
            }
            Expression::Object { members, span } => {
                let present = present();
                if let Ty::Record(fields) = relation::expand(self.manifest(), &present) {
                    return self.object_against(members, *span, &present, &fields, place);
                }
            }
            Expression::Array { elements, span } => {
                if let Ty::List(item) = relation::expand(self.manifest(), &present()) {
                    return self.array_against(elements, *span, &item, place);
                }
            }
            Expression::Conditional {
                condition,
                consequent,
                alternate,
                span,
            } => {
                if let Some(ty) = self.expression(condition, place) {
                    self.expect(ty, condition.span(), Expectation::Condition, &Ty::Boolean);
                }
                let consequent = self.check(consequent, place, expected, expectation.clone());
                let alternate = self.check(alternate, place, expected, expectation);
                // Each branch fits on its own, so they need no common
                // type; the conditional has one where they do.
                let ty = join(self.manifest(), &consequent?, &alternate?)
                    .unwrap_or_else(|| expected.clone());
                return self.typed(*span, ty);
            }
            _ => {}
        }
        let ty = self.expression(expression, place)?;
        self.expect(ty.clone(), expression.span(), expectation, expected);
        Some(ty)
    }

    /// An array literal where a list of `item` is expected: each element
    /// must fit `item`, so the elements need no common type. The array is
    /// a list of their common type where they have one (`[]` stays
    /// `list<nothing>`), and a list of `item` where they don't.
    fn array_against(
        &mut self,
        elements: &[Expression],
        span: Span,
        item: &Ty,
        place: Place<'m>,
    ) -> Option<Ty> {
        let types: Vec<Option<Ty>> = elements
            .iter()
            .map(|element| self.check(element, place, item, Expectation::Element))
            .collect();
        let mut common = Some(Ty::Nothing);
        for ty in types {
            let ty = ty?;
            common = common.and_then(|common| join(self.manifest(), &common, &ty));
        }
        let common = common.unwrap_or_else(|| item.clone());
        self.typed(span, Ty::List(Box::new(common)))
    }

    /// An object literal where the record type `record` (whose fields are
    /// `fields`) is expected: each key must be one of the fields, each
    /// value must fit its field, and every required field must be
    /// present. The same as `is_assignable` for the literal's own type,
    /// reported field by field.
    fn object_against(
        &mut self,
        members: &[ObjectMember],
        span: Span,
        record: &Ty,
        fields: &BTreeMap<String, FieldTy>,
        place: Place<'m>,
    ) -> Option<Ty> {
        let last = last_occurrences(members);
        let mut own = BTreeMap::new();
        let mut complete = true;
        for (index, member) in members.iter().enumerate() {
            // A shadowed occurrence isn't part of the literal: its value is
            // typed, but not checked against the field.
            if self.shadowed(members, index, &last) {
                self.expression(&member.value, place);
                continue;
            }
            let ty = match fields.get(&member.key) {
                Some(field) => {
                    let expectation = Expectation::Field {
                        field: member.key.clone(),
                    };
                    self.check(&member.value, place, &field.ty, expectation)
                }
                None => {
                    self.facts.push(Fact::UnknownField {
                        record: record.clone(),
                        field: member.key.clone(),
                        span: member.key_span,
                        candidates: fields.keys().cloned().collect(),
                    });
                    self.expression(&member.value, place)
                }
            };
            match ty {
                Some(ty) => {
                    own.insert(member.key.clone(), FieldTy { ty, required: true });
                }
                None => complete = false,
            }
        }
        for (name, field) in fields {
            if field.required && !last.contains_key(name.as_str()) {
                self.facts.push(Fact::MissingRequiredField {
                    record: record.clone(),
                    field: name.clone(),
                    span,
                });
            }
        }
        if complete {
            self.typed(span, Ty::Record(own))
        } else {
            None
        }
    }

    /// The values of `ty` if it is an enum once named references and one
    /// `optional` are stripped (`size?` and a named alias of an enum
    /// included); `None` for every other type.
    fn enum_values(&self, ty: &Ty) -> Option<Vec<String>> {
        let manifest = self.manifest();
        let present = match relation::expand(manifest, ty) {
            Ty::Optional(inner) => relation::expand(manifest, &inner),
            other => other,
        };
        match present {
            Ty::Enum(values) => Some(values),
            _ => None,
        }
    }

    /// A string literal `text` (at `span`, quotes included) where `expected`
    /// is expected: a literal site (T3). `None` if `expected` is not an
    /// enum, so this is no literal site. Otherwise `Some` of the literal's
    /// type, which is the enum's (the type as written, `Size`, not its
    /// values) when `text` is one of its values, and `Some(None)`, the
    /// error marker, after reporting `unknown-variant` when it is not.
    fn variant(&mut self, text: &str, span: Span, expected: &Ty) -> Option<Option<Ty>> {
        let values = self.enum_values(expected)?;
        if values.iter().any(|value| value == text) {
            let manifest = self.manifest();
            let present = match relation::expand(manifest, expected) {
                Ty::Optional(inner) => *inner,
                _ => expected.clone(),
            };
            return Some(self.typed(span, present));
        }
        self.facts.push(Fact::UnknownVariant {
            value: text.to_string(),
            values,
            span,
        });
        Some(None)
    }

    /// Records that the expression at `span` has type `ty`, and returns
    /// it.
    fn typed(&mut self, span: Span, ty: Ty) -> Option<Ty> {
        self.types.push(Typed {
            span,
            ty: ty.clone(),
        });
        Some(ty)
    }

    /// Resolves and types `expression` (outline D15). `None` is the error
    /// marker: a fact about the expression, or a part of it, has been
    /// reported, and nothing that depends on its type is checked, so one
    /// mistake gets one diagnostic.
    fn expression(&mut self, expression: &Expression, place: Place<'m>) -> Option<Ty> {
        let ty = match expression {
            Expression::Literal { value, .. } => Some(match value {
                Literal::String(_) => Ty::String,
                Literal::Number(_) => Ty::Number,
                Literal::Boolean(_) => Ty::Boolean,
                Literal::Null => Ty::Null,
            }),
            Expression::Reference { name, span }
                if self.loops.iter().any(|(bound, _, _)| bound == name) =>
            {
                let (_, ty, declaration) = self
                    .loops
                    .iter()
                    .rev()
                    .find(|(bound, _, _)| bound == name)
                    .expect("just found");
                let (ty, declaration) = (ty.clone(), *declaration);
                self.resolve(
                    *span,
                    Target::Binding {
                        name: name.clone(),
                        declaration,
                    },
                );
                Some(ty)
            }
            Expression::Reference { name, span }
                if self.snippets.iter().any(|local| &local.name == name) =>
            {
                let local = self
                    .snippets
                    .iter()
                    .rev()
                    .find(|local| &local.name == name)
                    .expect("just found");
                let declaration = local.name_span;
                let ty = Ty::Snippet(
                    local
                        .inferred
                        .iter()
                        .map(|ty| ty.clone().unwrap_or(Ty::Any))
                        .collect(),
                );
                self.resolve(
                    *span,
                    Target::Binding {
                        name: name.clone(),
                        declaration,
                    },
                );
                Some(ty)
            }
            Expression::Reference { name, span } => {
                let scope = &self.template.component().scope;
                match scope.get_key_value(name) {
                    Some((name, ty)) => {
                        self.resolve(*span, Target::Scope(name.clone()));
                        Some(Ty::from(ty))
                    }
                    None => {
                        self.facts.push(Fact::UnknownReference {
                            name: name.clone(),
                            span: *span,
                            candidates: scope.keys().cloned().collect(),
                        });
                        None
                    }
                }
            }
            Expression::MemberAccess {
                object,
                property,
                property_span,
                ..
            } => {
                let object_ty = self.expression(object, place)?;
                self.member(object_ty, object.span(), property, *property_span)
            }
            Expression::Unary {
                operator, operand, ..
            } => {
                let needs = match operator {
                    UnaryOperator::Not => Ty::Boolean,
                    UnaryOperator::Negate => Ty::Number,
                };
                self.operand(operand, place, Operator::Unary(*operator), &needs);
                Some(needs)
            }
            Expression::Binary {
                operator,
                left,
                right,
                span,
            } => self.binary(*operator, left, right, *span, place),
            Expression::Conditional {
                condition,
                consequent,
                alternate,
                span,
            } => {
                if let Some(ty) = self.expression(condition, place) {
                    self.expect(ty, condition.span(), Expectation::Condition, &Ty::Boolean);
                }
                let consequent = self.expression(consequent, place);
                let alternate = self.expression(alternate, place);
                self.common(consequent?, alternate?, *span, Combination::Branches)
            }
            Expression::Array { elements, .. } => self.array(elements, place),
            Expression::Object { members, .. } => self.object(members, place),
            // A handler's own command never gets here (see `handler`), so
            // this one is misplaced. It is reported once, and neither its
            // name nor its arguments are checked.
            // `has(x)`: whether `x` is present. Not a command, and no name is
            // reserved: it is this only where a command could not be.
            Expression::Command {
                command,
                arguments,
                span,
                ..
            } if command == "has" => self.presence(arguments, *span, place),
            Expression::Command { command, span, .. } => {
                self.facts.push(Fact::CommandOutsideHandler {
                    command: command.clone(),
                    span: *span,
                });
                None
            }
            Expression::EventValue { name, span } => self.event_value(name, *span, place),
        }?;
        self.typed(expression.span(), ty)
    }

    /// `has(operand)`: a boolean. The operand must be able to be absent;
    /// otherwise the test is always true, which is a warning.
    fn presence(&mut self, arguments: &[Expression], span: Span, place: Place<'m>) -> Option<Ty> {
        if arguments.len() != 1 {
            self.facts.push(Fact::PresenceArity {
                found: arguments.len(),
                span,
            });
            for argument in arguments {
                self.expression(argument, place);
            }
            return None;
        }
        let ty = self.expression(&arguments[0], place)?;
        if !matches!(relation::expand(self.manifest(), &ty), Ty::Optional(_)) {
            self.facts.push(Fact::RedundantPresenceTest { ty, span });
        }
        Some(Ty::Boolean)
    }

    /// `left ?? right`: `left` if it is present, otherwise `right`. The
    /// result is never optional: the common type of what `left` is when
    /// present and what `right` is. `right` must not be optional itself,
    /// and a `left` that cannot be absent makes `right` dead code, which is
    /// a warning. Where `left` is an enum, a string literal `right` is a
    /// literal site, so `size ?? "md"` is still the enum.
    fn coalesce(
        &mut self,
        left: &Expression,
        right: &Expression,
        span: Span,
        place: Place<'m>,
    ) -> Option<Ty> {
        let manifest = self.manifest();
        let left_ty = self.expression(left, place);
        let left_present = left_ty
            .as_ref()
            .map(|ty| match relation::expand(manifest, ty) {
                Ty::Optional(inner) => *inner,
                _ => ty.clone(),
            });
        if let (Some(ty), Some(_)) = (&left_ty, &left_present) {
            if !matches!(relation::expand(manifest, ty), Ty::Optional(_)) {
                self.facts.push(Fact::RedundantDefault {
                    ty: ty.clone(),
                    span,
                });
            }
        }
        let right_ty = match (&left_present, right) {
            (
                Some(expected),
                Expression::Literal {
                    value: Literal::String(text),
                    span: literal,
                },
            ) if self.enum_values(expected).is_some() => {
                self.variant(text, *literal, expected).flatten()
            }
            _ => self.expression(right, place),
        };
        let (left_present, right_ty) = (left_present?, right_ty?);
        // The default is a value, not a maybe-value.
        if let Ty::Optional(inner) = relation::expand(manifest, &right_ty) {
            self.facts.push(Fact::TypeMismatch {
                expectation: Expectation::Operand(Operator::Binary(BinaryOperator::Coalesce)),
                expected: *inner,
                actual: right_ty,
                possibly_absent: true,
                span: right.span(),
            });
            return None;
        }
        self.common(left_present, right_ty, span, Combination::Default)
    }

    /// The type of `object.property`, where `object` has type `object_ty`.
    fn member(
        &mut self,
        object_ty: Ty,
        object_span: Span,
        property: &str,
        property_span: Span,
    ) -> Option<Ty> {
        let manifest = self.manifest();
        match relation::expand(manifest, &object_ty) {
            Ty::Optional(_) => {
                self.facts.push(Fact::PossiblyAbsentAccess {
                    object: object_ty,
                    property: property.to_string(),
                    span: object_span,
                });
                return None;
            }
            Ty::Any => return Some(Ty::Any),
            _ => {}
        }
        // `members` is the one rule for which members a type has; an
        // editor's completion offers exactly its keys.
        let fields = relation::members(manifest, &object_ty);
        match fields.as_ref().and_then(|fields| fields.get(property)) {
            Some(field) => Some(relation::read(manifest, field)),
            None => {
                self.facts.push(Fact::UnknownMember {
                    object: object_ty,
                    property: property.to_string(),
                    span: property_span,
                    candidates: fields
                        .map(|fields| fields.keys().cloned().collect())
                        .unwrap_or_default(),
                });
                None
            }
        }
    }

    /// Types an operand that must be assignable to `needs`.
    fn operand(&mut self, operand: &Expression, place: Place<'m>, operator: Operator, needs: &Ty) {
        if let Some(ty) = self.expression(operand, place) {
            self.expect(ty, operand.span(), Expectation::Operand(operator), needs);
        }
    }

    /// Reports a [`Fact::TypeMismatch`] unless `actual` is assignable to
    /// `expected`.
    fn expect(&mut self, actual: Ty, span: Span, expectation: Expectation, expected: &Ty) {
        let manifest = self.manifest();
        if !is_assignable(manifest, &actual, expected) {
            // Absence is the reason only if the value would fit once
            // present: `string?` where `number` is needed is a string.
            let possibly_absent = match relation::expand(manifest, &actual) {
                Ty::Optional(present) => {
                    !matches!(relation::expand(manifest, expected), Ty::Optional(_))
                        && is_assignable(manifest, &present, expected)
                }
                _ => false,
            };
            self.facts.push(Fact::TypeMismatch {
                expectation,
                expected: expected.clone(),
                possibly_absent,
                actual,
                span,
            });
        }
    }

    /// The common type of `left` and `right`, or a [`Fact::NoCommonType`]
    /// at `span`.
    fn common(&mut self, left: Ty, right: Ty, span: Span, combination: Combination) -> Option<Ty> {
        let common = join(self.manifest(), &left, &right);
        if common.is_none() {
            self.facts.push(Fact::NoCommonType {
                combination,
                left,
                right,
                span,
            });
        }
        common
    }

    fn binary(
        &mut self,
        operator: BinaryOperator,
        left: &Expression,
        right: &Expression,
        span: Span,
        place: Place<'m>,
    ) -> Option<Ty> {
        use BinaryOperator::*;
        let (needs, result) = match operator {
            Coalesce => return self.coalesce(left, right, span, place),
            Mul | Div | Mod | Add | Sub => (Ty::Number, Ty::Number),
            Lt | Le | Gt | Ge => (Ty::Number, Ty::Boolean),
            And | Or => (Ty::Boolean, Ty::Boolean),
            // Strict equality: the operands need a common type, and
            // nothing else.
            Eq | Ne => {
                let left_ty = self.expression(left, place);
                let right_ty = self.expression(right, place);
                if let (Some(left_ty), Some(right_ty)) = (left_ty, right_ty) {
                    // A string literal against an enum is a literal site (T3): the
                    // literal must be one of its values. Nothing else narrows.
                    let site = match (left, right) {
                        (
                            Expression::Literal {
                                value: Literal::String(text),
                                span,
                            },
                            _,
                        ) => self.variant(text, *span, &right_ty),
                        (
                            _,
                            Expression::Literal {
                                value: Literal::String(text),
                                span,
                            },
                        ) => self.variant(text, *span, &left_ty),
                        _ => None,
                    };
                    if site.is_none() {
                        self.common(left_ty, right_ty, span, Combination::Equality(operator));
                    }
                }
                return Some(Ty::Boolean);
            }
        };
        let operator = Operator::Binary(operator);
        self.operand(left, place, operator, &needs);
        self.operand(right, place, operator, &needs);
        Some(result)
    }

    /// `[]` is `list<nothing>`; otherwise the elements' common type, or a
    /// [`Fact::NoCommonType`] at the first element that doesn't fit the
    /// ones before it. An element with an error is left out of the join,
    /// so the others are still compared with each other, but the array
    /// then has no type.
    fn array(&mut self, elements: &[Expression], place: Place<'m>) -> Option<Ty> {
        let types: Vec<Option<Ty>> = elements
            .iter()
            .map(|element| self.expression(element, place))
            .collect();
        // `None` once an element has no common type with those before it.
        let mut common = Some(Ty::Nothing);
        let mut complete = true;
        for (element, ty) in elements.iter().zip(types) {
            let Some(ty) = ty else {
                complete = false;
                continue;
            };
            if let Some(so_far) = common {
                common = self.common(so_far, ty, element.span(), Combination::Elements);
            }
        }
        let common = common.filter(|_| complete)?;
        Some(Ty::List(Box::new(common)))
    }

    /// An exact record with every key required, each typed by its value.
    /// As with attributes, the last occurrence of a repeated key is the
    /// one that counts; each earlier one is reported, and its value is
    /// still checked.
    fn object(&mut self, members: &[ObjectMember], place: Place<'m>) -> Option<Ty> {
        let last = last_occurrences(members);
        let mut fields = BTreeMap::new();
        let mut complete = true;
        for (index, member) in members.iter().enumerate() {
            let ty = self.expression(&member.value, place);
            if self.shadowed(members, index, &last) {
                continue;
            }
            match ty {
                Some(ty) => {
                    fields.insert(member.key.clone(), FieldTy { ty, required: true });
                }
                None => complete = false,
            }
        }
        complete.then_some(Ty::Record(fields))
    }

    /// Whether `members[index]` is shadowed by a later member with the
    /// same key, which `last` (from [`last_occurrences`]) says. If it is,
    /// reports it.
    fn shadowed(
        &mut self,
        members: &[ObjectMember],
        index: usize,
        last: &BTreeMap<&str, usize>,
    ) -> bool {
        let member = &members[index];
        let kept = last[member.key.as_str()];
        if kept == index {
            return false;
        }
        self.facts.push(Fact::DuplicateObjectKey {
            key: member.key.clone(),
            span: member.key_span,
            last: members[kept].key_span,
        });
        true
    }

    fn event_value(&mut self, name: &str, span: Span, place: Place<'m>) -> Option<Ty> {
        if name != "event" {
            self.facts.push(Fact::UnknownSpecialValue {
                name: name.to_string(),
                span,
                candidates: vec!["event".to_string()],
            });
            return None;
        }
        match place {
            Place::Value => {
                self.facts.push(Fact::EventValueOutsideHandler { span });
                None
            }
            Place::HandlerArgument(Payload::Value(ty)) => Some(Ty::from(ty)),
            Place::HandlerArgument(Payload::None { component, event }) => {
                self.facts.push(Fact::EventHasNoPayload {
                    component: component.to_string(),
                    event: event.to_string(),
                    span,
                });
                None
            }
            Place::HandlerArgument(Payload::Unknown) => None,
        }
    }
}

/// The index of each key's last occurrence in `members`: the one that
/// counts when a key is repeated.
fn last_occurrences(members: &[ObjectMember]) -> BTreeMap<&str, usize> {
    members
        .iter()
        .enumerate()
        .map(|(index, member)| (member.key.as_str(), index))
        .collect()
}
