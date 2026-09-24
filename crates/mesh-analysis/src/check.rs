//! The analysis walk: one pass over the IR, from the root element down,
//! in source order within each element. It resolves names, types every
//! expression, and records facts as it goes.

use crate::relation::{self, is_assignable, join};
use crate::types::{FieldTy, Ty};
use crate::{Analysis, Combination, Expectation, Fact, Operator, Resolution, Target, Typed};
use mesh_manifest::{Component, Manifest, Template, Type};
use mesh_semantic::{AttributeValue, Child, Element, Expression, Literal, ObjectMember};
use mesh_syntax::{BinaryOperator, Span, UnaryOperator};
use std::collections::BTreeMap;

pub(crate) fn analyze(ir: &Element, template: Template<'_>) -> Analysis {
    let mut walker = Walker {
        template,
        facts: Vec::new(),
        resolutions: Vec::new(),
        types: Vec::new(),
    };
    walker.element(ir);
    let Walker {
        mut facts,
        resolutions,
        types,
        ..
    } = walker;
    // Stable, so facts that start at the same byte keep the order the
    // walk found them in.
    facts.sort_by_key(|fact| fact.span().start_byte);
    Analysis {
        facts,
        resolutions,
        types,
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
}

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
        let manifest = self.manifest();
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
            // The prop's declared type, if the prop resolves.
            let mut expected = None;
            if let Some((name, component)) = component {
                match component.props.get_key_value(&attribute.name) {
                    Some((prop, field)) => {
                        self.resolve(
                            attribute.name_span,
                            Target::Prop {
                                component: name.to_string(),
                                prop: prop.clone(),
                            },
                        );
                        let expectation = Expectation::Prop {
                            component: name.to_string(),
                            prop: prop.clone(),
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
            match (&attribute.value, expected) {
                (AttributeValue::String { span, .. }, Some((expected, expectation))) => {
                    self.expect(Ty::String, *span, expectation, &expected);
                }
                (AttributeValue::String { .. }, None) => {}
                (AttributeValue::Expression(expression), Some((expected, expectation))) => {
                    self.check(expression, Place::Value, &expected, expectation);
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

        for child in &element.children {
            match child {
                Child::Text { .. } => {}
                Child::Expression(expression) => {
                    self.expression(expression, Place::Value);
                }
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
        let commands = &self.template.component().commands;
        let parameters = match commands.get_key_value(command) {
            Some((name, declared)) => {
                self.resolve(command_span, Target::Command(name.clone()));
                if declared.parameters.len() == arguments.len() {
                    Some(&declared.parameters)
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
            None => {
                self.facts.push(Fact::UnknownCommand {
                    command: command.to_string(),
                    span: command_span,
                    candidates: commands.keys().cloned().collect(),
                });
                None
            }
        };
        match parameters {
            Some(parameters) => {
                for (argument, parameter) in arguments.iter().zip(parameters) {
                    let expectation = Expectation::Argument {
                        command: command.to_string(),
                        parameter: parameter.name.clone(),
                    };
                    self.check(argument, place, &Ty::from(&parameter.ty), expectation);
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
                None
            }
            Ty::Any => Some(Ty::Any),
            Ty::Record(fields) => match fields.get(property) {
                Some(field) => Some(relation::read(manifest, field)),
                None => {
                    self.facts.push(Fact::UnknownMember {
                        object: object_ty,
                        property: property.to_string(),
                        span: property_span,
                        candidates: fields.keys().cloned().collect(),
                    });
                    None
                }
            },
            _ => {
                self.facts.push(Fact::UnknownMember {
                    object: object_ty,
                    property: property.to_string(),
                    span: property_span,
                    candidates: Vec::new(),
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
            Mul | Div | Mod | Add | Sub => (Ty::Number, Ty::Number),
            Lt | Le | Gt | Ge => (Ty::Number, Ty::Boolean),
            And | Or => (Ty::Boolean, Ty::Boolean),
            // Strict equality: the operands need a common type, and
            // nothing else.
            Eq | Ne => {
                let left = self.expression(left, place);
                let right = self.expression(right, place);
                if let (Some(left), Some(right)) = (left, right) {
                    self.common(left, right, span, Combination::Equality(operator));
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
