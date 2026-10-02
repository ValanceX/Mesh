//! Dispatch (docs/manual/runtime.md, "The dispatch lifecycle"): a render,
//! a handler identifier and a payload in; a command intent out, or
//! diagnostics.

use crate::boundary::{output, Inputs};
use crate::diagnostic::{Location, PathSegment, RuntimeCode, RuntimeDiagnostic};
use crate::eval::Scope;
use crate::program::{
    self, alternatives, uses_conditional, uses_repeat, Program, Step, Valid, ALTERNATIVES,
    COMPOSITE, CONDITIONAL, NODE,
};
use crate::render::{bind, render_children, snapshot_values, statics, Render, RenderChild};
use crate::tree::Intent;
use crate::types::fits;
use crate::value::{HostRecord, HostValue, Value};
use mesh_template::Element;
use std::collections::BTreeMap;

/// Where a handler is: the composite occurrences on the way to its node,
/// each in the template it's in, then the node and the event.
struct Site<'v> {
    /// (the template's component, the composite occurrence in it).
    composites: Vec<(&'v str, &'v Element)>,
    /// The template the node is in.
    component: &'v str,
    node: &'v Element,
    event: &'v str,
    /// The scope's values at the node, when a render recorded them: a
    /// repeated node's include its item, which the program alone can't say.
    values: Option<&'v BTreeMap<String, Value>>,
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
        &mut found,
    );
    found
}

fn walk<'v>(
    valid: &'v Valid,
    component: &'v str,
    element: &'v Element,
    position: usize,
    path: &mut Vec<Step>,
    composites: &mut Vec<(&'v str, &'v Element)>,
    found: &mut BTreeMap<String, Site<'v>>,
) {
    if let Some(template) = valid.templates.get(&element.component) {
        path.push(Step {
            position,
            kind: COMPOSITE,
            component: element.component.clone(),
        });
        composites.push((component, element));
        walk(
            valid,
            &template.component,
            &template.root,
            0,
            path,
            composites,
            found,
        );
        composites.pop();
        path.pop();
        return;
    }
    path.push(Step {
        position,
        kind: NODE,
        component: element.component.clone(),
    });
    let key = program::key(&valid.identity, path);
    for binding in &element.events {
        found.insert(
            program::handler(&valid.identity, &key, &binding.event),
            Site {
                composites: composites.clone(),
                component,
                node: element,
                event: &binding.event,
                values: None,
            },
        );
    }
    for (index, child) in render_children(element).into_iter().enumerate() {
        match child {
            RenderChild::Element(child) => {
                walk(valid, component, child, index, path, composites, found);
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
                    walk(valid, component, element, 0, path, composites, found);
                    path.pop();
                }
            }
            // A repeat's sites are per item, and an item's key needs values:
            // the walk has none. `dispatch_from` takes a render's record for
            // a program that has one.
            RenderChild::Repeat(_) | RenderChild::Run(_) => {}
        }
    }
    path.pop();
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
    let (templates, root) = render.program();
    dispatch_from(
        &Program {
            root,
            templates: &templates,
        },
        &render.model,
        &render.snapshot,
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
    let repeats = uses_repeat(&valid.templates);
    let recorded;
    let sites;
    let site = if repeats {
        recorded = crate::render::tree(&valid, snapshot, true)?.1;
        recorded.get(handler).map(|found| Site {
            composites: Vec::new(),
            component: &found.component,
            node: found.node,
            event: found.event,
            values: Some(&found.values),
        })
    } else {
        sites = self::sites(&valid);
        sites.get(handler).map(|found| Site {
            composites: found.composites.clone(),
            component: found.component,
            node: found.node,
            event: found.event,
            values: None,
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
        if let Ok(rendered) = crate::render::render(program, model, snapshot) {
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
    // it, then the handler's arguments, left to right.
    let evaluate = || -> Result<Intent, RuntimeDiagnostic> {
        let mut values = root_values;
        for (component, occurrence) in &site.composites {
            let scope = Scope {
                component,
                values: &values,
                payload: None,
                statics: statics(&valid, component, None),
            };
            values = bind(&valid, &scope, occurrence)?;
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
        let command = &valid.component(site.component).commands[&binding.command];
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
                value => {
                    Some(output(&value).map_err(|why| scope.error(why.code, why.message, span))?)
                }
            });
        }
        Ok(Intent {
            component: site.component.to_string(),
            command: binding.command.clone(),
            arguments,
        })
    };
    evaluate().map_err(|diagnostic| vec![diagnostic])
}
