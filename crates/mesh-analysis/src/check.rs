//! The analysis walk: one pass over the IR, from the root element down,
//! in source order within each element.

use crate::{Analysis, Fact, Resolution, Target};
use mesh_manifest::{Component, Template};
use mesh_semantic::{AttributeValue, Child, Element, Expression};
use mesh_syntax::Span;

pub(crate) fn analyze(ir: &Element, template: Template<'_>) -> Analysis {
    let mut walker = Walker {
        template,
        facts: Vec::new(),
        resolutions: Vec::new(),
    };
    walker.element(ir);
    let Walker {
        mut facts,
        resolutions,
        ..
    } = walker;
    // Stable, so facts that start at the same byte keep the order the
    // walk found them in.
    facts.sort_by_key(|fact| fact.span().start_byte);
    Analysis { facts, resolutions }
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
    /// The event declares a payload.
    Value,
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
}

impl<'m> Walker<'m> {
    fn resolve(&mut self, span: Span, target: Target) {
        self.resolutions.push(Resolution { span, target });
    }

    /// Checks one element as an instance of the component its tag names,
    /// then its attribute values, its handlers and its children.
    fn element(&mut self, element: &Element) {
        let manifest = self.template.manifest();
        let component = match manifest.components().get_key_value(&element.name) {
            Some((name, component)) => {
                self.resolve(element.name_span, Target::Component(name.clone()));
                Some((name.as_str(), component))
            }
            None => {
                self.facts.push(Fact::UnknownComponent {
                    name: element.name.clone(),
                    span: element.name_span,
                    candidates: manifest.components().keys().cloned().collect(),
                });
                None
            }
        };

        if let Some((name, component)) = component {
            self.missing_props(element, name, component);
        }

        for attribute in &element.attributes {
            if let Some((name, component)) = component {
                if component.props.contains_key(&attribute.name) {
                    self.resolve(
                        attribute.name_span,
                        Target::Prop {
                            component: name.to_string(),
                            prop: attribute.name.clone(),
                        },
                    );
                } else {
                    self.facts.push(Fact::UnknownProp {
                        component: name.to_string(),
                        prop: attribute.name.clone(),
                        span: attribute.name_span,
                        candidates: component.props.keys().cloned().collect(),
                    });
                }
            }
            match &attribute.value {
                AttributeValue::String { .. } => {}
                AttributeValue::Expression(expression) => {
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
                            Some(_) => Payload::Value,
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

        for child in &element.children {
            match child {
                Child::Text { .. } => {}
                Child::Expression(expression) => self.expression(expression, Place::Value),
                Child::Element(child) => self.element(child),
            }
        }
    }

    /// Every prop `component` declares required that `element` doesn't
    /// supply, in the manifest's order, each at the tag name.
    fn missing_props(&mut self, element: &Element, name: &str, component: &Component) {
        for (prop, field) in &component.props {
            let supplied = element
                .attributes
                .iter()
                .any(|attribute| &attribute.name == prop);
            if field.required && !supplied {
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
            } => self.command(
                command,
                *command_span,
                arguments,
                *span,
                Place::HandlerArgument(payload),
            ),
            other => self.facts.push(Fact::HandlerNotCommand {
                event: event.to_string(),
                span: other.span(),
            }),
        }
    }

    /// A command in handler position: resolves its name and checks its
    /// arity, then walks its arguments in `place`.
    fn command(
        &mut self,
        command: &str,
        command_span: Span,
        arguments: &[Expression],
        span: Span,
        place: Place<'m>,
    ) {
        let commands = &self.template.component().commands;
        match commands.get_key_value(command) {
            Some((name, declared)) => {
                self.resolve(command_span, Target::Command(name.clone()));
                if declared.parameters.len() != arguments.len() {
                    self.facts.push(Fact::CommandArityMismatch {
                        command: command.to_string(),
                        expected: declared.parameters.len(),
                        found: arguments.len(),
                        span,
                    });
                }
            }
            None => self.facts.push(Fact::UnknownCommand {
                command: command.to_string(),
                span: command_span,
                candidates: commands.keys().cloned().collect(),
            }),
        }
        for argument in arguments {
            self.expression(argument, place);
        }
    }

    fn expression(&mut self, expression: &Expression, place: Place<'m>) {
        match expression {
            Expression::Literal { .. } => {}
            Expression::Reference { name, span } => {
                let scope = &self.template.component().scope;
                match scope.get_key_value(name) {
                    Some((name, _)) => self.resolve(*span, Target::Scope(name.clone())),
                    None => self.facts.push(Fact::UnknownReference {
                        name: name.clone(),
                        span: *span,
                        candidates: scope.keys().cloned().collect(),
                    }),
                }
            }
            // The property is resolved against the object's type, which
            // is expression typing's job.
            Expression::MemberAccess { object, .. } => self.expression(object, place),
            Expression::Unary { operand, .. } => self.expression(operand, place),
            Expression::Binary { left, right, .. } => {
                self.expression(left, place);
                self.expression(right, place);
            }
            Expression::Conditional {
                condition,
                consequent,
                alternate,
                ..
            } => {
                self.expression(condition, place);
                self.expression(consequent, place);
                self.expression(alternate, place);
            }
            Expression::Array { elements, .. } => {
                for element in elements {
                    self.expression(element, place);
                }
            }
            Expression::Object { members, .. } => {
                for member in members {
                    self.expression(&member.value, place);
                }
            }
            // A handler's own command never gets here (see `handler`), so
            // this one is misplaced. It is reported once, and neither its
            // name nor its arguments are checked.
            Expression::Command { command, span, .. } => {
                self.facts.push(Fact::CommandOutsideHandler {
                    command: command.clone(),
                    span: *span,
                });
            }
            Expression::EventValue { name, span } => self.event_value(name, *span, place),
        }
    }

    fn event_value(&mut self, name: &str, span: Span, place: Place<'m>) {
        if name != "event" {
            self.facts.push(Fact::UnknownSpecialValue {
                name: name.to_string(),
                span,
                candidates: vec!["event".to_string()],
            });
            return;
        }
        match place {
            Place::Value => self.facts.push(Fact::EventValueOutsideHandler { span }),
            Place::HandlerArgument(Payload::None { component, event }) => {
                self.facts.push(Fact::EventHasNoPayload {
                    component: component.to_string(),
                    event: event.to_string(),
                    span,
                });
            }
            Place::HandlerArgument(Payload::Value | Payload::Unknown) => {}
        }
    }
}
