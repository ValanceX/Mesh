//! Inferring the contracts of a program's composites.
//!
//! A composite's contract (its props, scope and events) is already stated
//! by the program: its props are what its occurrences pass, with the types
//! of the arguments they pass; its events are the handlers its template
//! names that are not commands, with the type of the argument each
//! forwards. [`infer_components`] reads those and adds the entries to the
//! manifest, so nobody writes them. A component the manifest already
//! declares is never touched: a declaration overrides inference.
//!
//! It adds no rule of its own and reports nothing. Where the occurrences
//! disagree, the first one's type is kept, and the ordinary checks then
//! report the others; any other problem is reported by the compile that
//! follows with the manifest this returns.
//!
//! Two passes, because the information flows both ways: props flow from
//! callers to callees, and event payloads from callees to callers.

use std::collections::{BTreeMap, BTreeSet};

use mesh_analysis::Ty;
use mesh_semantic::{AttributeValue, Child, Element, Expression};
use serde_json::{json, Map, Value};

use crate::{compile_with, CompileOptions};

/// The reserved structural tags: each is declared, when a source uses it and the manifest does not, as the language defines it.
fn reserved() -> Vec<(&'static str, Value)> {
    let none = || entry(Map::new(), Map::new(), Map::new());
    let props = |props: Value| json!({ "props": props, "events": {}, "commands": {}, "scope": {} });

    vec![
        ("mesh-slot", none()),
        ("mesh-switch", none()),
        ("mesh-default", none()),
        (
            "mesh-case",
            props(json!({ "when": { "type": { "kind": "boolean" }, "required": true } })),
        ),
        (
            "mesh-if",
            props(json!({ "when": { "type": { "kind": "boolean" }, "required": true } })),
        ),
        (
            "mesh-each",
            props(json!({
                "items": { "type": { "kind": "list", "element": { "kind": "any" } }, "required": true },
                "as": { "type": { "kind": "string" }, "required": true },
                "key": { "type": { "kind": "any" }, "required": true }
            })),
        ),
    ]
}

/// What one occurrence of a composite passes: each prop, with its type.
type Occurrence = BTreeMap<String, Ty>;

/// `manifest` (its text) with an entry added for each component in
/// `sources` that it does not declare, and for each reserved tag
/// (`mesh-slot`, `mesh-switch`, `mesh-case`, `mesh-default`, `mesh-if`,
/// `mesh-each`) that a source uses and it does not declare. `None` when there is nothing to add, or
/// when the manifest cannot be read (the compile reports that).
pub fn infer_components(
    manifest: &str,
    root: &str,
    sources: &[(String, String)],
) -> Option<String> {
    let mut document: Value = serde_json::from_str(manifest).ok()?;
    let components = document.get_mut("components")?.as_object_mut()?;
    let declared: BTreeSet<String> = components.keys().cloned().collect();
    let names: BTreeSet<&str> = sources.iter().map(|(name, _)| name.as_str()).collect();
    let inferred: BTreeSet<&str> = names
        .iter()
        .copied()
        .filter(|name| !declared.contains(*name) && !name.starts_with("mesh-"))
        .collect();
    let mut changed = false;

    for (tag, declaration) in reserved() {
        // A switch is written as `mesh-if`s, so a program that has one uses `mesh-if`.
        let used = sources.iter().any(|(_, source)| {
            opens(source, tag) || (tag == "mesh-if" && opens(source, "mesh-switch"))
        });

        if used && !declared.contains(tag) {
            components.insert(tag.to_string(), declaration);
            changed = true;
        }
    }

    if inferred.is_empty() {
        return changed.then(|| document.to_string());
    }

    // Who uses whom, from the templates alone.
    let mut elements: BTreeMap<&str, Element> = BTreeMap::new();
    for (name, source) in sources {
        if let Some(ir) = mesh_semantic::lower(&mesh_parser::parse(source).ast?).ir {
            elements.insert(name.as_str(), ir);
        }
    }
    let uses: BTreeMap<&str, BTreeSet<&str>> = elements
        .iter()
        .map(|(name, ir)| {
            let mut used = BTreeSet::new();
            tags(ir, &names, &mut used);
            used.remove(name);
            (*name, used)
        })
        .collect();

    // Callers first: a reverse postorder from the root. A cycle is cut where it closes; the program check reports it.
    let mut seen = BTreeSet::new();
    let mut post = Vec::new();
    visit(root, &uses, &mut seen, &mut post);
    let callers_first: Vec<&str> = post.iter().rev().copied().collect();

    let sources: BTreeMap<&str, &str> = sources
        .iter()
        .map(|(name, source)| (name.as_str(), source.as_str()))
        .collect();

    // Pass 1, callers first: props. A component's turn comes after every caller's, so the occurrences it is given are all in.
    let mut occurrences: BTreeMap<&str, Vec<Occurrence>> = BTreeMap::new();
    for name in &callers_first {
        let text = document.to_string();
        let manifest = mesh_manifest::load(&text).ok()?;

        if inferred.contains(name) {
            let Some(given) = occurrences.get(name) else {
                continue;
            };
            let (props, scope) = props_of(&manifest, given);
            let components = document.get_mut("components")?.as_object_mut()?;
            components.insert((*name).to_string(), entry(props, Map::new(), scope));
            changed = true;
        }

        let text = document.to_string();
        let manifest = mesh_manifest::load(&text).ok()?;
        let Ok(template) = manifest.template(name) else {
            continue;
        };
        let compiled = compile_with(sources[name], &CompileOptions::with_template(template));
        let (Some(ir), Some(analysis)) = (&compiled.ir, &compiled.analysis) else {
            continue;
        };

        collect_occurrences(ir, analysis, &inferred, &mut occurrences);
    }

    // Pass 2, callees first: events. A forwarded event's payload is read from the callee's entry, so the callee goes before its callers.
    for name in callers_first.iter().rev() {
        if !inferred.contains(name) {
            continue;
        }

        let text = document.to_string();
        let manifest = mesh_manifest::load(&text).ok()?;
        let Ok(template) = manifest.template(name) else {
            continue;
        };
        let compiled = compile_with(sources[name], &CompileOptions::with_template(template));
        let (Some(ir), Some(analysis)) = (&compiled.ir, &compiled.analysis) else {
            continue;
        };
        let events = events_of(ir, analysis);

        if !events.is_empty() {
            let component = document
                .get_mut("components")?
                .get_mut(*name)?
                .as_object_mut()?;
            component.insert("events".to_string(), Value::Object(events));
        }
    }

    changed.then(|| document.to_string())
}

/// Whether `source` opens an element named `tag` (`<mesh-slot` but not `<mesh-slots`).
fn opens(source: &str, tag: &str) -> bool {
    let opening = format!("<{tag}");

    source.match_indices(&opening).any(|(at, _)| {
        source[at + opening.len()..]
            .chars()
            .next()
            .is_none_or(|next| !(next.is_alphanumeric() || next == '-'))
    })
}

fn visit<'a>(
    name: &'a str,
    uses: &BTreeMap<&'a str, BTreeSet<&'a str>>,
    seen: &mut BTreeSet<&'a str>,
    post: &mut Vec<&'a str>,
) {
    if !seen.insert(name) {
        return;
    }

    if let Some(used) = uses.get(name) {
        for next in used {
            visit(next, uses, seen, post);
        }
    }

    post.push(name);
}

fn tags<'a>(element: &'a Element, names: &BTreeSet<&'a str>, found: &mut BTreeSet<&'a str>) {
    if let Some(name) = names.get(element.name.as_str()) {
        found.insert(name);
    }

    for child in &element.children {
        if let Child::Element(child) = child {
            tags(child, names, found);
        }
    }
}

fn entry(
    props: Map<String, Value>,
    events: Map<String, Value>,
    scope: Map<String, Value>,
) -> Value {
    json!({ "props": props, "events": events, "commands": {}, "scope": scope })
}

fn collect_occurrences<'n>(
    element: &Element,
    analysis: &mesh_analysis::Analysis,
    inferred: &BTreeSet<&'n str>,
    found: &mut BTreeMap<&'n str, Vec<Occurrence>>,
) {
    if let Some(name) = inferred.get(element.name.as_str()).copied() {
        let mut passed = Occurrence::new();

        for attribute in &element.attributes {
            let ty = match &attribute.value {
                AttributeValue::String { .. } => Ty::String,
                AttributeValue::Expression(expression) => analysis
                    .type_at(expression.span())
                    .cloned()
                    .unwrap_or(Ty::Any),
            };

            passed.insert(attribute.name.clone(), ty);
        }

        found.entry(name).or_default().push(passed);
    }

    for child in &element.children {
        if let Child::Element(child) = child {
            collect_occurrences(child, analysis, inferred, found);
        }
    }
}

/// The props and scope of a composite given `occurrences`: a prop every occurrence passes is required, with the first type seen (joined with the
/// others where they are compatible); a prop some leave out is optional.
fn props_of(
    manifest: &mesh_manifest::Manifest,
    occurrences: &[Occurrence],
) -> (Map<String, Value>, Map<String, Value>) {
    let mut types: BTreeMap<&str, Ty> = BTreeMap::new();

    for occurrence in occurrences {
        for (name, ty) in occurrence {
            let ty = plain(ty);
            let merged = match types.get(name.as_str()) {
                Some(seen) => {
                    mesh_analysis::join(manifest, seen, &ty).unwrap_or_else(|| seen.clone())
                }
                None => ty,
            };

            types.insert(name, merged);
        }
    }

    let mut props = Map::new();
    let mut scope = Map::new();

    for (name, ty) in types {
        let required = occurrences
            .iter()
            .all(|occurrence| occurrence.contains_key(name));
        let ty = if required || matches!(ty, Ty::Optional(_)) {
            ty
        } else {
            Ty::Optional(Box::new(ty))
        };
        let json = type_json(&ty);

        props.insert(
            name.to_string(),
            json!({ "type": json, "required": required }),
        );
        scope.insert(name.to_string(), json);
    }

    (props, scope)
}

/// The events a composite's template emits: each handler naming something that is not a command, with the type of the one argument it forwards.
fn events_of(element: &Element, analysis: &mesh_analysis::Analysis) -> Map<String, Value> {
    let mut events = Map::new();
    collect_events(element, analysis, &mut events);
    events
}

fn collect_events(
    element: &Element,
    analysis: &mesh_analysis::Analysis,
    events: &mut Map<String, Value>,
) {
    for binding in &element.event_bindings {
        if let Expression::Command {
            command, arguments, ..
        } = &binding.handler
        {
            if events.contains_key(command) {
                continue;
            }

            let declared = match arguments.as_slice() {
                [] => json!({}),
                [argument] => {
                    let ty = analysis.type_at(argument.span()).map_or(Ty::Any, plain);
                    json!({ "payload": type_json(&ty) })
                }
                _ => continue,
            };

            events.insert(command.clone(), declared);
        }
    }

    for child in &element.children {
        if let Child::Element(child) = child {
            collect_events(child, analysis, events);
        }
    }
}

/// A type a manifest can write: the two analysis-only types become `any`.
fn plain(ty: &Ty) -> Ty {
    match ty {
        Ty::Nothing | Ty::Void => Ty::Any,
        Ty::List(element) => Ty::List(Box::new(plain(element))),
        Ty::Optional(inner) => Ty::Optional(Box::new(plain(inner))),
        Ty::Record(fields) => Ty::Record(
            fields
                .iter()
                .map(|(name, field)| {
                    (
                        name.clone(),
                        mesh_analysis::FieldTy {
                            ty: plain(&field.ty),
                            required: field.required,
                        },
                    )
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

fn type_json(ty: &Ty) -> Value {
    match ty {
        Ty::String => json!({ "kind": "string" }),
        Ty::Number => json!({ "kind": "number" }),
        Ty::Boolean => json!({ "kind": "boolean" }),
        Ty::Null => json!({ "kind": "null" }),
        Ty::Any | Ty::Nothing | Ty::Void => json!({ "kind": "any" }),
        Ty::List(element) => json!({ "kind": "list", "element": type_json(element) }),
        Ty::Named(name) => json!({ "kind": "named", "name": name }),
        Ty::Optional(inner) => json!({ "kind": "optional", "type": type_json(inner) }),
        Ty::Record(fields) => {
            let fields: Map<String, Value> = fields
                .iter()
                .map(|(name, field)| {
                    (
                        name.clone(),
                        json!({ "type": type_json(&field.ty), "required": field.required }),
                    )
                })
                .collect();

            json!({ "kind": "record", "fields": fields })
        }
    }
}
