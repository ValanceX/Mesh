//! Shared by the runtime's tests: one model, templates compiled from MPRX
//! by the real compiler, and shortcuts for render and dispatch.

#![allow(dead_code)]

pub mod host;
pub mod renderer;

use mesh_compiler::check;
use mesh_runtime::{HostRecord, HostValue, Intent, Program, Render, RuntimeDiagnostic};

/// The model every test uses unless it says otherwise.
pub const MODEL: &str = include_str!("../programs/model.json");

/// A snapshot for `view` in which every value is ordinary.
pub const SNAPSHOT: &str = include_str!("../programs/snapshot.json");

/// The template of `component` whose source is `source`, compiled against
/// `model`. Panics if it doesn't compile.
pub fn compile_with(model: &str, component: &str, source: &str) -> String {
    let model = check::Model::load(model, component).expect("the model loads");
    let compiled = check::template(source, &model);
    let template = compiled
        .template
        .unwrap_or_else(|| panic!("{component} compiles: {:#?}", compiled.diagnostics));
    mesh_template::to_json(&template)
}

/// The path of child indexes from `element` to the first element named `component`, at or under it, depth first.
fn path_to(element: &serde_json::Value, component: &str) -> Option<Vec<usize>> {
    if element["component"] == component {
        return Some(Vec::new());
    }
    for (index, child) in element["children"].as_array()?.iter().enumerate() {
        if let Some(inner) = child.get("element") {
            if let Some(mut path) = path_to(inner, component) {
                path.insert(0, index);
                return Some(path);
            }
        }
    }
    None
}

fn element_at<'a>(element: &'a mut serde_json::Value, path: &[usize]) -> &'a mut serde_json::Value {
    match path.split_first() {
        None => element,
        Some((index, rest)) => element_at(&mut element["children"][*index]["element"], rest),
    }
}

/// Edits the first element named `component` in a compiled template (as JSON).
pub fn edit_first(
    template: &mut serde_json::Value,
    component: &str,
    edit: impl FnOnce(&mut serde_json::Value),
) {
    let path = path_to(&template["root"], component)
        .unwrap_or_else(|| panic!("the template has a `{component}`"));
    edit(element_at(&mut template["root"], &path));
}

/// The template of `component` compiled from `source`, then edited as JSON. For a
/// malformed construct the language can no longer write: control flow is syntax, so
/// a conditional with three alternatives, or a repeat with no body, is made by
/// editing the template of a well-formed one.
pub fn compile_edited(
    model: &str,
    component: &str,
    source: &str,
    edit: impl FnOnce(&mut serde_json::Value),
) -> String {
    let mut template: serde_json::Value =
        serde_json::from_str(&compile_with(model, component, source))
            .expect("a compiled template is JSON");
    edit(&mut template);
    template.to_string()
}

pub fn compile(component: &str, source: &str) -> String {
    compile_with(MODEL, component, source)
}

pub fn snapshot(text: &str) -> HostRecord {
    HostRecord::from_json(text).expect("the snapshot is JSON")
}

/// Renders the program `root` + `templates` against `snapshot`.
pub fn try_render(
    root: &str,
    templates: &[String],
    snapshot_json: &str,
) -> Result<Render, Vec<RuntimeDiagnostic>> {
    let texts: Vec<&str> = templates.iter().map(String::as_str).collect();
    mesh_runtime::render(
        &Program {
            root,
            templates: &texts,
        },
        MODEL,
        &snapshot(snapshot_json),
    )
}

/// Renders `view` from `source`, against the ordinary snapshot.
pub fn view(source: &str) -> Result<Render, Vec<RuntimeDiagnostic>> {
    try_render("view", &[compile("view", source)], SNAPSHOT)
}

pub fn codes(diagnostics: &[RuntimeDiagnostic]) -> Vec<&'static str> {
    diagnostics.iter().map(|d| d.code).collect()
}

/// The handler identifier of the first node (depth first) with a binding
/// for `event`.
pub fn handler(render: &Render, event: &str) -> String {
    fn find(node: &mesh_runtime::Node, event: &str) -> Option<String> {
        if let Some(id) = node.events.get(event) {
            return Some(id.clone());
        }
        node.children.iter().find_map(|child| match child {
            mesh_runtime::TreeChild::Node(child) => find(child, event),
            mesh_runtime::TreeChild::Text { .. } => None,
        })
    }
    find(&render.tree().root, event).expect("a node binds the event")
}

pub fn dispatch(
    render: &Render,
    event: &str,
    payload: Option<&str>,
) -> Result<Intent, Vec<RuntimeDiagnostic>> {
    let payload = payload.map(|text| HostValue::from_json(text).expect("the payload is JSON"));
    mesh_runtime::dispatch(render, &handler(render, event), payload.as_ref())
}
