//! `update` (docs/superpowers/specs/2026-10-06-mesh-fine-grained-reactivity-
//! and-composition.md, A3): the equivalence law (applying an update's
//! patches to the previous tree gives exactly the tree a full render of the
//! new snapshot gives), the work an update skips, and that a refused update
//! leaves the previous render valid.

mod common;

use common::{codes, compile_with, snapshot};
use mesh_runtime::{
    dispatch, evaluations, patches_to_json, render, update, HostValue, Program, Render,
};
use serde_json::Value;

const MODEL: &str = r#"{
  "version": 1, "types": {},
  "components": {
    "page": { "props": { "title": { "type": { "kind": "string" }, "required": true },
                         "count": { "type": { "kind": "number" }, "required": true },
                         "tag":   { "type": { "kind": "optional", "type": { "kind": "string" } }, "required": false } },
              "events": { "tap": {} }, "commands": {}, "scope": {} },
    "note": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "row":  { "props": { "label": { "type": { "kind": "string" }, "required": true },
                         "done":  { "type": { "kind": "boolean" }, "required": true } },
              "events": { "tap": {} }, "commands": {}, "scope": {} },
    "mesh-if": { "props": { "when": { "type": { "kind": "boolean" }, "required": true } }, "events": {}, "commands": {}, "scope": {} },
    "mesh-each": { "props": {
        "items": { "type": { "kind": "list", "element": { "kind": "any" } }, "required": true },
        "as":    { "type": { "kind": "string" }, "required": true },
        "key":   { "type": { "kind": "any" }, "required": true } },
      "events": {}, "commands": {}, "scope": {} },
    "view": { "props": {}, "events": {},
      "commands": { "pick": { "parameters": [ { "name": "id", "type": { "kind": "number" } } ] } },
      "scope": {
        "title": { "kind": "string" },
        "count": { "kind": "number" },
        "flag":  { "kind": "boolean" },
        "tag":   { "kind": "optional", "type": { "kind": "string" } },
        "items": { "kind": "list", "element": { "kind": "record", "fields": {
            "id": { "type": { "kind": "number" }, "required": true },
            "label": { "type": { "kind": "string" }, "required": true },
            "done": { "type": { "kind": "boolean" }, "required": true } } } } } }
  }
}"#;

const VIEW: &str = "<page title={title} count={count + 1} tag={tag}>\
  <note>{title} has {count}</note>\
  <mesh-if when={flag}><note>on</note><note>off</note></mesh-if>\
  <mesh-each items={items} as=\"item\" key={item.id}>\
    <row label={item.label} done={item.done} on.tap={pick(item.id)}>{item.label}</row>\
  </mesh-each>\
</page>";

fn program_render(snapshot_json: &str) -> Render {
    let template = compile_with(MODEL, "view", VIEW);
    let templates = [template.as_str()];
    render(
        &Program {
            root: "view",
            templates: &templates,
        },
        MODEL,
        &snapshot(snapshot_json),
    )
    .unwrap_or_else(|d| panic!("renders: {d:#?}"))
}

/// Resolves `render-v1`'s URL to the schema in this repository.
struct Local;

impl jsonschema::Retrieve for Local {
    fn retrieve(
        &self,
        uri: &jsonschema::Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let name = uri.as_str().rsplit('/').next().unwrap_or_default();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas");
        Ok(serde_json::from_str(&std::fs::read_to_string(
            dir.join(name),
        )?)?)
    }
}

/// A validator for `render-patch-v1`; its `$ref`s to `render-v1` resolve
/// to the file.
fn patch_schema() -> jsonschema::Validator {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas");
    let schema: Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("render-patch-v1.schema.json")).unwrap(),
    )
    .unwrap();
    jsonschema::options()
        .with_retriever(Local)
        .build(&schema)
        .expect("render-patch-v1 is a schema")
}

/// A reference applier for `render-patch-v1`, over render-v1 JSON. A
/// renderer is tested the same way: apply, then compare with a full render.
fn apply(tree: &mut Value, patches: &Value) {
    assert_eq!(patches["format"], "mesh-render-patch");
    assert_eq!(patches["version"], 1);
    for patch in patches["patches"].as_array().expect("a list") {
        match patch["op"].as_str().expect("an op") {
            "replace" => *tree = patch["tree"].clone(),
            "setProp" => {
                let node = find(&mut tree["root"], patch["key"].as_str().unwrap());
                node["props"][patch["prop"].as_str().unwrap()] = patch["value"].clone();
                match patch.get("propText") {
                    Some(text) => node["propText"][patch["prop"].as_str().unwrap()] = text.clone(),
                    None => {
                        if let Some(map) = node.get_mut("propText").and_then(Value::as_object_mut) {
                            map.remove(patch["prop"].as_str().unwrap());
                            if map.is_empty() {
                                node.as_object_mut().unwrap().remove("propText");
                            }
                        }
                    }
                }
            }
            "removeProp" => {
                let node = find(&mut tree["root"], patch["key"].as_str().unwrap());
                let prop = patch["prop"].as_str().unwrap();
                node["props"].as_object_mut().unwrap().remove(prop);
                if let Some(map) = node.get_mut("propText").and_then(Value::as_object_mut) {
                    map.remove(prop);
                    if map.is_empty() {
                        node.as_object_mut().unwrap().remove("propText");
                    }
                }
            }
            "setText" => {
                *find(&mut tree["root"], patch["key"].as_str().unwrap()) = {
                    let mut text = find(&mut tree["root"], patch["key"].as_str().unwrap()).clone();
                    text["text"] = patch["text"].clone();
                    text
                };
            }
            other => panic!("unknown op {other}"),
        }
    }
}

/// The node or text with `key`.
fn find<'t>(node: &'t mut Value, key: &str) -> &'t mut Value {
    fn path(node: &Value, key: &str) -> Option<Vec<usize>> {
        if node["key"] == key {
            return Some(vec![]);
        }
        for (index, child) in node["children"].as_array()?.iter().enumerate() {
            if let Some(mut rest) = path(child, key) {
                rest.insert(0, index);
                return Some(rest);
            }
        }
        None
    }
    let steps = path(node, key).unwrap_or_else(|| panic!("no node {key}"));
    let mut at = node;
    for step in steps {
        at = &mut at["children"][step];
    }
    at
}

/// A small deterministic generator, so the test needs no dependency.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// A random snapshot, kept near `base` so that most updates change a little.
fn random_snapshot(rng: &mut Rng, base: Option<&Value>) -> Value {
    let mut out = base.cloned().unwrap_or_else(
        || serde_json::json!({ "title": "t", "count": 0, "flag": false, "items": [] }),
    );
    for _ in 0..=rng.below(3) {
        match rng.below(7) {
            0 => out["title"] = format!("title {}", rng.below(4)).into(),
            1 => out["count"] = (rng.below(5) as f64 - 2.0).into(),
            2 => out["flag"] = (rng.below(2) == 1).into(),
            3 => {
                if rng.below(2) == 0 {
                    out["tag"] = format!("tag {}", rng.below(3)).into();
                } else {
                    out.as_object_mut().unwrap().remove("tag");
                }
            }
            4 => {
                let n = rng.below(5);
                out["items"] = (0..n)
                    .map(|i| serde_json::json!({ "id": i, "label": format!("l{}", rng.below(3)), "done": rng.below(2) == 1 }))
                    .collect::<Vec<_>>()
                    .into();
            }
            5 => {
                if let Some(items) = out["items"].as_array_mut() {
                    if !items.is_empty() {
                        let at = rng.below(items.len() as u64) as usize;
                        items[at]["label"] = format!("l{}", rng.below(3)).into();
                        items[at]["done"] = (rng.below(2) == 1).into();
                    }
                }
            }
            _ => {}
        }
    }
    out
}

#[test]
fn applying_an_updates_patches_gives_the_full_render() {
    let mut rng = Rng(7);
    let mut checked = 0;
    let schema = patch_schema();
    for _ in 0..40 {
        let mut state = random_snapshot(&mut rng, None);
        let mut current = program_render(&state.to_string());
        for _ in 0..25 {
            state = random_snapshot(&mut rng, Some(&state));
            let next = snapshot(&state.to_string());
            let updated = update(&current, &next).unwrap_or_else(|d| panic!("updates: {d:#?}"));
            let full = program_render(&state.to_string());
            // The new render is the full render.
            assert_eq!(updated.render.tree(), full.tree(), "tree for {state}");
            // And the patches take the old tree to it.
            let mut tree: Value = serde_json::from_str(&current.tree().to_json()).unwrap();
            let patches: Value = serde_json::from_str(&patches_to_json(&updated.patches)).unwrap();
            assert!(
                schema.is_valid(&patches),
                "a render-patch-v1 document: {patches}"
            );
            apply(&mut tree, &patches);
            let expected: Value = serde_json::from_str(&full.tree().to_json()).unwrap();
            assert_eq!(tree, expected, "patched tree for {state}");
            current = updated.render;
            checked += 1;
        }
    }
    assert_eq!(checked, 1000);
}

#[test]
fn an_update_with_nothing_changed_has_no_patches_and_evaluates_nothing_it_can_reuse() {
    let state =
        r#"{"title":"t","count":1,"flag":true,"items":[{"id":1,"label":"a","done":false}]}"#;
    let current = program_render(state);
    let before = evaluations();
    let updated = update(&current, &snapshot(state)).expect("updates");
    assert!(updated.patches.is_empty());
    // Only the structural expressions are evaluated: `when` (1), `items`
    // (1) and `key` for the one item (`item.id` is a member of a scope
    // read, 2). Every prop and text is reused.
    assert_eq!(evaluations() - before, 4);
}

#[test]
fn one_changed_field_patches_one_prop_and_one_text() {
    let a = r#"{"title":"t","count":1,"flag":true,"items":[{"id":1,"label":"a","done":false},{"id":2,"label":"b","done":false}]}"#;
    let b = r#"{"title":"t","count":1,"flag":true,"items":[{"id":1,"label":"a","done":false},{"id":2,"label":"B","done":false}]}"#;
    let current = program_render(a);
    let updated = update(&current, &snapshot(b)).expect("updates");
    let doc: Value = serde_json::from_str(&patches_to_json(&updated.patches)).unwrap();
    let ops: Vec<&str> = doc["patches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["op"].as_str().unwrap())
        .collect();
    assert_eq!(ops, ["setProp", "setText"], "{doc}");
}

#[test]
fn a_change_of_structure_is_a_replace() {
    let a = r#"{"title":"t","count":1,"flag":true,"items":[]}"#;
    let b = r#"{"title":"t","count":1,"flag":false,"items":[]}"#;
    let updated = update(&program_render(a), &snapshot(b)).expect("updates");
    let doc: Value = serde_json::from_str(&patches_to_json(&updated.patches)).unwrap();
    assert_eq!(doc["patches"][0]["op"], "replace");
    assert_eq!(doc["patches"].as_array().unwrap().len(), 1);
}

#[test]
fn a_refused_update_leaves_the_previous_render_valid() {
    let a = r#"{"title":"t","count":1,"flag":true,"items":[{"id":1,"label":"a","done":false}]}"#;
    let current = program_render(a);
    let refused = update(
        &current,
        &snapshot(r#"{"title":1,"count":1,"flag":true,"items":[]}"#),
    );
    assert_eq!(
        codes(&refused.expect_err("a title must be a string")),
        ["runtime-value-mismatch"]
    );
    let handler = current.tree().root.children.iter().find_map(|c| match c {
        mesh_runtime::TreeChild::Node(n) if n.events.contains_key("tap") => {
            n.events.get("tap").cloned()
        }
        _ => None,
    });
    let handler = handler.or_else(|| {
        fn find(n: &mesh_runtime::Node) -> Option<String> {
            n.events.get("tap").cloned().or_else(|| {
                n.children.iter().find_map(|c| match c {
                    mesh_runtime::TreeChild::Node(n) => find(n),
                    _ => None,
                })
            })
        }
        find(&current.tree().root)
    });
    let intent = dispatch(&current, &handler.expect("a handler"), None).expect("still dispatches");
    assert_eq!(intent.command, "pick");
    let _ = HostValue::from_json("null");
}
