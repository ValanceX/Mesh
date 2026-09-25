//! Shared by the runtime's tests: one model, templates compiled from MPRX
//! by the real compiler, and shortcuts for render and dispatch.

#![allow(dead_code)]

use mesh_compiler::check;
use mesh_runtime::{HostRecord, HostValue, Intent, Program, Render, RuntimeDiagnostic};

/// The model every test uses unless it says otherwise.
pub const MODEL: &str = r#"{
  "version": 1,
  "types": {
    "User": { "kind": "record", "fields": {
      "name": { "type": { "kind": "string" }, "required": true },
      "avatar": { "type": { "kind": "string" }, "required": false },
      "active": { "type": { "kind": "boolean" }, "required": true }
    } },
    "Point": { "kind": "record", "fields": {
      "x": { "type": { "kind": "number" }, "required": true },
      "y": { "type": { "kind": "number" }, "required": true }
    } }
  },
  "components": {
    "page": { "props": { "title": { "type": { "kind": "string" }, "required": false } },
              "events": {}, "commands": {}, "scope": {} },
    "text": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "probe": {
      "props": {
        "any": { "type": { "kind": "any" }, "required": false },
        "maybeAny": { "type": { "kind": "optional", "type": { "kind": "any" } }, "required": false },
        "num": { "type": { "kind": "number" }, "required": false },
        "maybeNum": { "type": { "kind": "optional", "type": { "kind": "number" } }, "required": false },
        "str": { "type": { "kind": "string" }, "required": false },
        "maybeStr": { "type": { "kind": "optional", "type": { "kind": "string" } }, "required": false },
        "flag": { "type": { "kind": "boolean" }, "required": false },
        "none": { "type": { "kind": "null" }, "required": false },
        "numbers": { "type": { "kind": "list", "element": { "kind": "number" } }, "required": false },
        "user": { "type": { "kind": "named", "name": "User" }, "required": false }
      },
      "events": {
        "click": { "payload": { "kind": "named", "name": "Point" } },
        "tap": {},
        "pick": { "payload": { "kind": "any" } },
        "maybe": { "payload": { "kind": "optional", "type": { "kind": "string" } } }
      },
      "commands": {}, "scope": {}
    },
    "view": {
      "props": {}, "events": {},
      "commands": {
        "select": { "parameters": [{ "name": "user", "type": { "kind": "named", "name": "User" } }] },
        "save": { "parameters": [] },
        "setCount": { "parameters": [{ "name": "count", "type": { "kind": "number" } }] },
        "setName": { "parameters": [{ "name": "name", "type": { "kind": "optional", "type": { "kind": "string" } } }] },
        "take": { "parameters": [{ "name": "value", "type": { "kind": "any" } }] },
        "place": { "parameters": [{ "name": "point", "type": { "kind": "named", "name": "Point" } }] }
      },
      "scope": {
        "user": { "kind": "named", "name": "User" },
        "maybeUser": { "kind": "optional", "type": { "kind": "named", "name": "User" } },
        "name": { "kind": "string" },
        "maybeName": { "kind": "optional", "type": { "kind": "string" } },
        "count": { "kind": "number" },
        "flag": { "kind": "boolean" },
        "anything": { "kind": "any" },
        "maybeAnything": { "kind": "optional", "type": { "kind": "any" } },
        "users": { "kind": "list", "element": { "kind": "named", "name": "User" } },
        "numbers": { "kind": "list", "element": { "kind": "number" } },
        "maybeNumbers": { "kind": "list", "element": { "kind": "optional", "type": { "kind": "number" } } },
        "nothing": { "kind": "null" }
      }
    },
    "card": {
      "props": {
        "user": { "type": { "kind": "named", "name": "User" }, "required": true },
        "compact": { "type": { "kind": "optional", "type": { "kind": "boolean" } }, "required": false },
        "extra": { "type": { "kind": "any" }, "required": false }
      },
      "events": {},
      "commands": { "selectUser": { "parameters": [{ "name": "user", "type": { "kind": "named", "name": "User" } }] } },
      "scope": {
        "user": { "kind": "named", "name": "User" },
        "compact": { "kind": "optional", "type": { "kind": "boolean" } }
      }
    },
    "frame": {
      "props": { "user": { "type": { "kind": "named", "name": "User" }, "required": true } },
      "events": {}, "commands": {},
      "scope": { "user": { "kind": "named", "name": "User" } }
    }
  }
}"#;

/// A snapshot for `view` in which every value is ordinary.
pub const SNAPSHOT: &str = r#"{
  "user": { "name": "Ada", "avatar": "ada.png", "active": true },
  "name": "Ada",
  "count": 3,
  "flag": true,
  "anything": { "deep": [1, "two", null] },
  "users": [{ "name": "Ada", "active": true }, { "name": "Grace", "active": false }],
  "numbers": [1, 2.5],
  "maybeNumbers": [1, 2],
  "nothing": null
}"#;

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
