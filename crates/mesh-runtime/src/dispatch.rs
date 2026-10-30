//! Dispatch (docs/manual/runtime.md, "The dispatch lifecycle"): a render,
//! a handler identifier and a payload in; a command intent out, or
//! diagnostics.

use crate::boundary::{output, Inputs};
use crate::diagnostic::{Location, PathSegment, RuntimeCode, RuntimeDiagnostic};
use crate::eval::Scope;
use crate::program::{
    self, alternatives, uses_conditional, uses_repeat, Program, Step, Valid, ALTERNATIVES,
    COMPOSITE, CONDITIONAL, FRAGMENT, FRAGMENT_COMPONENT, NODE, SLOT,
};
use crate::render::{
    bind, is_live, render_children, snapshot_values, statics, Link, Occurrence, Render, RenderChild,
};
use crate::tree::{Intent, NamedIntent};
use crate::types::fits;
use crate::value::{HostRecord, HostValue, Units, Value};
use mesh_template::{Element, Expression};
use std::collections::BTreeMap;
use std::rc::Rc;

/// Where a handler is: the composite occurrences on the way to its node,
/// each in the template it's in, then the node and the event.
struct Site<'v> {
    /// (the template's component, the composite occurrence in it, and its key when its component
    /// has a unit).
    composites: Vec<(&'v str, &'v Element, Option<String>)>,
    /// The template the node is in.
    component: &'v str,
    node: &'v Element,
    event: &'v str,
    /// The scope's values at the node, when a render recorded them: a
    /// repeated node's include its item, which the program alone can't say.
    values: Option<&'v BTreeMap<String, Value>>,
    /// The composite occurrences around the node with the scopes they were
    /// written in, when a render recorded them (a handler that forwards an
    /// event needs them); a walk of the program derives them from `composites`.
    chain: Option<&'v [Link<'v>]>,
    /// The unit occurrence whose template the node is in, when a render recorded it.
    instance: Option<Rc<Occurrence>>,
}

/// Every handler of the program, by identifier: a walk of its structure,
/// with no values, computing keys exactly as render does.
fn sites(valid: &Valid) -> BTreeMap<String, Site<'_>> {
    let mut found = BTreeMap::new();
    let root = &valid.templates[&valid.root];
    walk(
        valid,
        &valid.root,
        &root.root,
        0,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut Vec::new(),
        &mut found,
    );
    found
}

/// The composite occurrences being walked, innermost last: for each, how
/// many composites were on the way when it began, the component whose
/// template it is in, and the occurrence, whose children a slot places.
type Frames<'v> = Vec<(usize, &'v str, &'v Element)>;

#[allow(clippy::too_many_arguments)]
fn walk<'v>(
    valid: &'v Valid,
    component: &'v str,
    element: &'v Element,
    position: usize,
    path: &mut Vec<Step>,
    composites: &mut Vec<(&'v str, &'v Element, Option<String>)>,
    frames: &mut Frames<'v>,
    found: &mut BTreeMap<String, Site<'v>>,
) {
    if let Some(template) = valid.templates.get(&element.component) {
        path.push(Step {
            position,
            kind: COMPOSITE,
            component: element.component.clone(),
        });
        frames.push((composites.len(), component, element));
        let instance = valid
            .component(&element.component)
            .unit
            .is_some()
            .then(|| program::key(&valid.identity, path));
        composites.push((component, element, instance));
        walk(
            valid,
            &template.component,
            &template.root,
            0,
            path,
            composites,
            frames,
            found,
        );
        composites.pop();
        frames.pop();
        path.pop();
        return;
    }
    if element.component == FRAGMENT_COMPONENT {
        // It makes no node: its content's sites are below a step of its own.
        path.push(Step {
            position,
            kind: FRAGMENT,
            component: String::new(),
        });
        walk_children(valid, component, element, path, composites, frames, found);
        path.pop();
        return;
    }
    path.push(Step {
        position,
        kind: NODE,
        component: element.component.clone(),
    });
    let key = program::key(&valid.identity, path);
    let around: Vec<(&str, &Element)> = composites.iter().map(|(c, o, _)| (*c, *o)).collect();
    for binding in &element.events {
        // A handler that forwards an event nothing handles has no identifier.
        if !is_live(valid, component, &binding.command, &around) {
            continue;
        }
        found.insert(
            program::handler(&valid.identity, &key, &binding.event),
            Site {
                composites: composites.clone(),
                component,
                node: element,
                event: &binding.event,
                values: None,
                chain: None,
                instance: None,
            },
        );
    }
    walk_children(valid, component, element, path, composites, frames, found);
    path.pop();
}

/// The sites among `element`'s children, in the template of `component`.
fn walk_children<'v>(
    valid: &'v Valid,
    component: &'v str,
    element: &'v Element,
    path: &mut Vec<Step>,
    composites: &mut Vec<(&'v str, &'v Element, Option<String>)>,
    frames: &mut Frames<'v>,
    found: &mut BTreeMap<String, Site<'v>>,
) {
    for (index, child) in render_children(element).into_iter().enumerate() {
        match child {
            RenderChild::Element(child) => {
                walk(
                    valid, component, child, index, path, composites, frames, found,
                );
            }
            // Every alternative is a site, whichever a snapshot would choose:
            // identity is a function of the program, not of the values.
            RenderChild::Conditional(conditional) => {
                for (alternative, element) in alternatives(conditional).into_iter().enumerate() {
                    path.push(Step {
                        position: index,
                        kind: CONDITIONAL,
                        component: ALTERNATIVES[alternative].to_string(),
                    });
                    walk(
                        valid, component, element, 0, path, composites, frames, found,
                    );
                    path.pop();
                }
            }
            // The occurrence's children, placed here, are the *caller's*: in
            // its template, with its composites on the way, and with the
            // enclosing composites' frames, not this one's.
            RenderChild::Slot(name) => {
                if let Some((on_the_way, caller, occurrence)) = frames.pop() {
                    let hosts = composites.split_off(on_the_way);
                    path.push(Step {
                        position: index,
                        kind: SLOT,
                        component: name.to_string(),
                    });
                    let content = if name.is_empty() {
                        Some(occurrence)
                    } else {
                        program::fill_for(occurrence, name)
                    };
                    if let Some(content) = content {
                        walk_children(valid, caller, content, path, composites, frames, found);
                    }
                    path.pop();
                    composites.extend(hosts);
                    frames.push((on_the_way, caller, occurrence));
                }
            }
            // A repeat's sites are per item, and an item's key needs values:
            // the walk has none. `dispatch_from` takes a render's record for
            // a program that has one.
            RenderChild::Repeat(_) | RenderChild::Run(_) | RenderChild::Render(_) => {}
        }
    }
}

/// Every handler identifier in a tree.
fn handlers(node: &crate::tree::Node) -> std::collections::BTreeSet<String> {
    let mut found: std::collections::BTreeSet<String> = node.events.values().cloned().collect();
    for child in &node.children {
        if let crate::tree::TreeChild::Node(child) = child {
            found.extend(handlers(child));
        }
    }
    found
}

/// Dispatches the event whose handler is `handler`, with `payload`
/// (absent when `None`), against the render the renderer had drawn.
pub fn dispatch(
    render: &Render,
    handler: &str,
    payload: Option<&HostValue>,
) -> Result<Intent, Vec<RuntimeDiagnostic>> {
    dispatch_named(render, handler, payload).map(|named| named.intent)
}

/// [`dispatch`], with the name of each argument written as a plain path (`to`, `item.id`).
pub fn dispatch_named(
    render: &Render,
    handler: &str,
    payload: Option<&HostValue>,
) -> Result<NamedIntent, Vec<RuntimeDiagnostic>> {
    let (templates, root) = render.program();
    dispatch_named_from_units(
        &Program {
            root,
            templates: &templates,
        },
        &render.model,
        &render.host_snapshot(),
        &render.units,
        handler,
        payload,
    )
}

/// [`dispatch`], for a host that keeps a render's inputs itself instead
/// of its [`Render`]: the program, model and snapshot of a render that
/// succeeded, exactly as they were given to it. The WebAssembly module is
/// such a host, since a `Render` can't cross into JavaScript.
///
/// It is the same operation: everything is validated again (I14), and
/// nothing is rendered. Given inputs no render was made from, it gives
/// what dispatch against such a render would, or diagnostics.
pub fn dispatch_from(
    program: &Program<'_>,
    model: &str,
    snapshot: &HostRecord,
    handler: &str,
    payload: Option<&HostValue>,
) -> Result<Intent, Vec<RuntimeDiagnostic>> {
    dispatch_from_units(
        program,
        model,
        snapshot,
        &Rc::new(Units::default()),
        handler,
        payload,
    )
}

/// [`dispatch_from`], for a render made with the state of unit occurrences: they are given again, as
/// they were, so that the handler's arguments are evaluated against what the person saw.
pub fn dispatch_from_units(
    program: &Program<'_>,
    model: &str,
    snapshot: &HostRecord,
    units: &Rc<Units>,
    handler: &str,
    payload: Option<&HostValue>,
) -> Result<Intent, Vec<RuntimeDiagnostic>> {
    dispatch_named_from_units(program, model, snapshot, units, handler, payload)
        .map(|named| named.intent)
}

/// [`dispatch_from_units`], with the name of each argument written as a plain path.
pub fn dispatch_named_from_units(
    program: &Program<'_>,
    model: &str,
    snapshot: &HostRecord,
    units: &Rc<Units>,
    handler: &str,
    payload: Option<&HostValue>,
) -> Result<NamedIntent, Vec<RuntimeDiagnostic>> {
    // 1. Program validation, of the render's own copies (I14).
    let valid = program::validate(program, model)?;

    // 2. The handler identifier alone: the payload's type depends on it.
    let prefix = format!("{}.", program::handler_prefix(&valid.identity));
    if !handler.starts_with(&prefix) {
        return Err(vec![RuntimeDiagnostic::new(
            RuntimeCode::HANDLER_OTHER_PROGRAM,
            "this handler identifier isn't one of this render's program",
            Location::Handler,
        )]);
    }
    // A program with a repeat has handlers per item, and which items there
    // are is the snapshot's to say (spec §9.10.7): its handlers are the
    // render's, with the scope each had. Without one, they are a function of
    // the program, found without values.
    let repeats = uses_repeat(&valid.templates) || program::uses_snippets(&valid.templates);
    let recorded;
    let sites;
    let site = if repeats {
        recorded = crate::render::tree(&valid, snapshot, true, None, units)?.sites;
        recorded.get(handler).map(|found| Site {
            composites: Vec::new(),
            component: &found.component,
            node: found.node,
            event: found.event,
            values: Some(&found.values),
            chain: Some(&found.chain),
            instance: found.instance.clone(),
        })
    } else {
        sites = self::sites(&valid);
        sites.get(handler).map(|found| Site {
            composites: found.composites.clone(),
            component: found.component,
            node: found.node,
            event: found.event,
            values: None,
            chain: None,
            instance: None,
        })
    };
    let Some(site) = site.as_ref() else {
        return Err(vec![RuntimeDiagnostic::new(
            RuntimeCode::UNKNOWN_HANDLER,
            "this handler identifier names no handler in this render's program",
            Location::Handler,
        )]);
    };
    // With a conditional, a program has handlers its render may not: an
    // identifier is valid against the render only if its node is in the tree
    // (spec §9.10.7). Presence is rendering's to say, so ask it. When the
    // inputs can't be rendered there is no tree to check against, and step 3
    // reports what is wrong with them.
    if uses_conditional(&valid.templates) {
        if let Ok(rendered) = crate::render::render_units(program, model, snapshot, units) {
            if !handlers(&rendered.tree().root).contains(handler) {
                return Err(vec![RuntimeDiagnostic::new(
                    RuntimeCode::UNKNOWN_HANDLER,
                    "this handler identifier names no handler in this render: its node isn't in the tree",
                    Location::Handler,
                )]);
            }
        }
    }

    // 3. The render's snapshot and the payload, together.
    let mut inputs = Inputs::new(&valid.manifest);
    let mut root_values = snapshot_values(&valid, snapshot, &mut inputs);
    if let Some(values) = site.values {
        root_values = values.clone();
    }
    let event = &valid.component(&site.node.component).events[site.event];
    let payload_type = event.payload.as_ref();
    let payload_value = match payload_type {
        None => {
            if payload.is_some() {
                inputs.diagnostics.push(RuntimeDiagnostic::new(
                    RuntimeCode::UNEXPECTED_PAYLOAD,
                    format!(
                        "`{}`'s `{}` event has no payload, but one was given",
                        site.node.component, site.event
                    ),
                    Location::Input(vec![PathSegment::Name("$event".into())]),
                ));
            }
            Value::Absent
        }
        Some(ty) => inputs.value(payload, ty, &mut vec![PathSegment::Name("$event".into())]),
    };
    let diagnostics = inputs.sorted();
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    // 4. Evaluation: each composite's scope on the way, as render derived
    // it, then the handler's arguments, left to right. A handler that
    // forwards an event of its component goes on, out through the
    // occurrences around it, until one binds the event to a command.
    let evaluate = || -> Result<NamedIntent, RuntimeDiagnostic> {
        let mut values = root_values;
        // The occurrences around the node, outermost first, with the scope
        // each was written in.
        let mut around: Vec<Around<'_>> = Vec::new();
        // The unit occurrence whose template the handler's node is in.
        let mut owner = site.instance.clone();
        match site.chain {
            Some(recorded) => {
                for link in recorded {
                    around.push((
                        link.component.clone(),
                        link.occurrence,
                        link.values.clone(),
                        link.instance.clone(),
                    ));
                }
            }
            None => {
                for (component, occurrence, key) in &site.composites {
                    let scope = Scope {
                        component,
                        values: &values,
                        payload: None,
                        statics: statics(&valid, component, None),
                    };
                    let (bound, instance) =
                        bind(&valid, &scope, occurrence, key.as_deref(), units)?;
                    owner.clone_from(&instance);
                    around.push((
                        (*component).to_string(),
                        *occurrence,
                        std::mem::replace(&mut values, bound),
                        instance,
                    ));
                }
            }
        }
        let scope = Scope {
            component: site.component,
            values: &values,
            payload: Some(&payload_value),
            statics: statics(&valid, site.component, payload_type),
        };
        let binding = site
            .node
            .events
            .iter()
            .find(|binding| binding.event == site.event)
            .expect("the site's event is one of its node's bindings");
        if valid
            .component(site.component)
            .commands
            .contains_key(&binding.command)
        {
            return intent_of(&valid, &scope, site.component, binding, owner.as_deref());
        }
        // A forward: the event is one of the component's own. Its value is
        // the handler's argument, if the event has a payload.
        let carried = forwarded(&valid, &scope, site.component, binding)?;
        Ok(deliver(
            &valid,
            &mut around,
            site.component.to_string(),
            &binding.command,
            carried,
        )?
        .expect("a live forward has an occurrence that binds it"))
    };
    evaluate().map_err(|diagnostic| vec![diagnostic])
}

/// A composite occurrence around the place an event is raised: the component whose template it is
/// in, the occurrence, the scope it was written in, and the occurrence of that template as a unit.
type Around<'v> = (
    String,
    &'v Element,
    BTreeMap<String, Value>,
    Option<Rc<Occurrence>>,
);

/// Delivers the event `name` of `component`, carrying `carried`, out through the occurrences in
/// `around` (innermost last) until one binds it to a command of the template it is written in:
/// that command's intent. `None` when an occurrence on the way does not bind the event, or there
/// is none left: nothing handles it.
fn deliver<'v>(
    valid: &Valid,
    around: &mut Vec<Around<'v>>,
    mut component: String,
    mut name: &'v str,
    mut carried: Value,
) -> Result<Option<NamedIntent>, RuntimeDiagnostic> {
    loop {
        let Some((caller, occurrence, caller_values, _)) = around.pop() else {
            return Ok(None);
        };
        // The template the event is forwarded into is the one around the occurrence just left.
        let owner = around.last().and_then(|(.., instance)| instance.clone());
        let Some(bound) = occurrence.events.iter().find(|bound| bound.event == name) else {
            return Ok(None);
        };
        let event_type = valid.component(&component).events[name].payload.as_ref();
        let scope = Scope {
            component: &caller,
            values: &caller_values,
            payload: Some(&carried),
            statics: statics(valid, &caller, event_type),
        };
        if valid
            .component(&caller)
            .commands
            .contains_key(&bound.command)
        {
            return intent_of(valid, &scope, &caller, bound, owner.as_deref()).map(Some);
        }
        carried = forwarded(valid, &scope, &caller, bound)?;
        name = bound.command.as_str();
        component = caller;
    }
}

/// The command intent of `binding`, a binding of a command of `component`, in `scope`.
fn intent_of(
    valid: &Valid,
    scope: &Scope<'_, '_>,
    component: &str,
    binding: &mesh_template::EventBinding,
    owner: Option<&Occurrence>,
) -> Result<NamedIntent, RuntimeDiagnostic> {
    let command = &valid.component(component).commands[&binding.command];
    let mut arguments = Vec::with_capacity(binding.arguments.len());
    for (argument, parameter) in binding.arguments.iter().zip(&command.parameters) {
        let value = scope.eval(argument)?;
        let span = argument.span();
        if !fits(&valid.manifest, &value, &parameter.ty) {
            return Err(scope.error(
                RuntimeCode::ARGUMENT_MISMATCH,
                format!(
                    "this argument, {}, doesn't fit `{}`'s parameter `{}`",
                    value.kind(),
                    binding.command,
                    parameter.name
                ),
                span,
            ));
        }
        arguments.push(match value {
            Value::Absent => None,
            value => Some(output(&value).map_err(|why| scope.error(why.code, why.message, span))?),
        });
    }
    // A command of the component's unit is for the occurrence whose template this is.
    let of_unit = valid
        .component(component)
        .unit
        .as_ref()
        .is_some_and(|unit| unit.commands.contains(&binding.command));
    let owner = owner.filter(|_| of_unit);
    Ok(NamedIntent {
        intent: Intent {
            component: component.to_string(),
            command: binding.command.clone(),
            arguments,
            #[allow(deprecated)]
            instance: owner.map(|owner| owner.key.clone()),
            controlled: owner.map_or_else(Vec::new, |owner| owner.controlled.clone()),
        },
        names: binding.arguments.iter().map(path_name).collect(),
    })
}

/// The last segment of `argument` when it is a plain path: a scope name (`to`), or fields of one
/// (`item.id`). `None` for any other expression, `$event` included.
fn path_name(argument: &Expression) -> Option<String> {
    fn rooted(expression: &Expression) -> bool {
        match expression {
            Expression::Scope { .. } => true,
            Expression::Member { object, .. } => rooted(object),
            _ => false,
        }
    }
    match argument {
        Expression::Scope { name, .. } => Some(name.clone()),
        Expression::Member { object, field, .. } if rooted(object) => Some(field.clone()),
        _ => None,
    }
}

/// The payload a forwarding `binding` gives the event it names (an event of
/// `component`): its argument in `scope`, checked against the event's
/// payload type, or absent for an event with none.
fn forwarded(
    valid: &Valid,
    scope: &Scope<'_, '_>,
    component: &str,
    binding: &mesh_template::EventBinding,
) -> Result<Value, RuntimeDiagnostic> {
    let Some(argument) = binding.arguments.first() else {
        return Ok(Value::Absent);
    };
    let value = scope.eval(argument)?;
    let declared = valid.component(component).events[&binding.command]
        .payload
        .as_ref()
        .expect("a forward with an argument has an event with a payload");
    if !fits(&valid.manifest, &value, declared) {
        return Err(scope.error(
            RuntimeCode::ARGUMENT_MISMATCH,
            format!(
                "this argument, {}, doesn't fit the payload of `{component}`'s event `{}`",
                value.kind(),
                binding.command
            ),
            argument.span(),
        ));
    }
    Ok(value)
}

/// [`raise_from_units`], against a [`Render`] the host holds.
pub fn raise(
    render: &Render,
    occurrence: &str,
    event: &str,
    payload: Option<&HostValue>,
) -> Result<Option<Intent>, Vec<RuntimeDiagnostic>> {
    raise_named(render, occurrence, event, payload).map(|named| named.map(|named| named.intent))
}

/// [`raise`], with the name of each argument written as a plain path.
pub fn raise_named(
    render: &Render,
    occurrence: &str,
    event: &str,
    payload: Option<&HostValue>,
) -> Result<Option<NamedIntent>, Vec<RuntimeDiagnostic>> {
    let (templates, root) = render.program();
    raise_named_from_units(
        &Program {
            root,
            templates: &templates,
        },
        &render.model,
        &render.host_snapshot(),
        &render.units,
        occurrence,
        event,
        payload,
    )
}

/// Raises the event `event` of the unit occurrence `instance`, carrying `payload` (absent when
/// `None`), against the render the host holds: what the occurrence's unit announces to its caller
/// (docs/manual/runtime.md, "Units"). The result is the intent of the handler the caller bound to
/// the event, found as a handler is when the event is forwarded out through occurrences; `None` when
/// the caller bound none. Nothing is rendered.
///
/// `Err` when the program, snapshot or units are not valid, the instance is not in the render, its
/// component declares no such event, or the payload does not fit the event's.
pub fn raise_from_units(
    program: &Program<'_>,
    model: &str,
    snapshot: &HostRecord,
    units: &Rc<Units>,
    occurrence: &str,
    event: &str,
    payload: Option<&HostValue>,
) -> Result<Option<Intent>, Vec<RuntimeDiagnostic>> {
    raise_named_from_units(program, model, snapshot, units, occurrence, event, payload)
        .map(|named| named.map(|named| named.intent))
}

/// [`raise_from_units`], with the name of each argument written as a plain path.
pub fn raise_named_from_units(
    program: &Program<'_>,
    model: &str,
    snapshot: &HostRecord,
    units: &Rc<Units>,
    occurrence: &str,
    event: &str,
    payload: Option<&HostValue>,
) -> Result<Option<NamedIntent>, Vec<RuntimeDiagnostic>> {
    let valid = program::validate(program, model)?;
    let rendered = crate::render::tree(&valid, snapshot, true, None, units)?;
    let Some(found) = rendered.occurrences.get(occurrence) else {
        return Err(vec![RuntimeDiagnostic::new(
            RuntimeCode::UNKNOWN_OCCURRENCE,
            "this key names no unit occurrence in this render",
            Location::Handler,
        )]);
    };
    let component = found.occurrence.component.as_str();
    let Some(declared) = valid.component(component).events.get(event) else {
        return Err(vec![RuntimeDiagnostic::new(
            RuntimeCode::UNKNOWN_EVENT,
            format!("`{component}` declares no event `{event}`"),
            Location::Handler,
        )]);
    };
    let mut inputs = Inputs::new(&valid.manifest);
    let carried = match (&declared.payload, payload) {
        (None, None) => Value::Absent,
        (None, Some(_)) => {
            inputs.report(
                RuntimeCode::UNEXPECTED_PAYLOAD,
                &[PathSegment::Name("$event".into())],
                format!("`{component}`'s `{event}` event has no payload, but one was given"),
            );
            Value::Absent
        }
        (Some(ty), payload) => {
            inputs.value(payload, ty, &mut vec![PathSegment::Name("$event".into())])
        }
    };
    let diagnostics = inputs.sorted();
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let mut around: Vec<Around<'_>> = found
        .chain
        .iter()
        .map(|link| {
            (
                link.component.clone(),
                link.occurrence,
                link.values.clone(),
                link.instance.clone(),
            )
        })
        .collect();
    around.push((
        found.component.clone(),
        found.occurrence,
        found.values.clone(),
        Some(Rc::clone(&found.instance)),
    ));
    deliver(&valid, &mut around, component.to_string(), event, carried).map_err(|d| vec![d])
}
