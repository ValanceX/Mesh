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
            "insert" => {
                let parent = find(&mut tree["root"], patch["parent"].as_str().unwrap());
                let children = parent["children"].as_array_mut().unwrap();
                let at = match patch.get("before") {
                    Some(before) => children
                        .iter()
                        .position(|child| child["key"] == *before)
                        .unwrap_or_else(|| panic!("before {before} is a child at this point")),
                    None => children.len(),
                };
                assert!(
                    !children
                        .iter()
                        .any(|child| child["key"] == patch["node"]["key"]),
                    "an inserted key is new"
                );
                children.insert(at, patch["node"].clone());
            }
            "remove" => {
                let (parent, at) = parent_of(&mut tree["root"], patch["key"].as_str().unwrap());
                parent["children"].as_array_mut().unwrap().remove(at);
            }
            "move" => {
                let (parent, from) = parent_of(&mut tree["root"], patch["key"].as_str().unwrap());
                let children = parent["children"].as_array_mut().unwrap();
                let moved = children.remove(from);
                let at = match patch.get("before") {
                    Some(before) => children
                        .iter()
                        .position(|child| child["key"] == *before)
                        .unwrap_or_else(|| panic!("before {before} is a sibling at this point")),
                    None => children.len(),
                };
                children.insert(at, moved);
            }
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

/// The node holding `key` among its children, and `key`'s index there.
fn parent_of<'t>(node: &'t mut Value, key: &str) -> (&'t mut Value, usize) {
    fn path(node: &Value, key: &str) -> Option<Vec<usize>> {
        for (index, child) in node["children"].as_array()?.iter().enumerate() {
            if child["key"] == key {
                return Some(vec![index]);
            }
            if let Some(mut rest) = path(child, key) {
                rest.insert(0, index);
                return Some(rest);
            }
        }
        None
    }
    let mut steps = path(node, key).unwrap_or_else(|| panic!("no part {key}"));
    let last = steps.pop().unwrap();
    let mut at = node;
    for step in steps {
        at = &mut at["children"][step];
    }
    (at, last)
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
        match rng.below(9) {
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
                let mut ids: Vec<u64> = (0..8).collect();
                let n = rng.below(6) as usize;
                let mut items = Vec::new();
                for _ in 0..n {
                    let id = ids.remove(rng.below(ids.len() as u64) as usize);
                    items.push(serde_json::json!({ "id": id, "label": format!("l{}", rng.below(3)), "done": rng.below(2) == 1 }));
                }
                out["items"] = items.into();
            }
            6 => {
                // Reorder: swap two items.
                if let Some(items) = out["items"].as_array_mut() {
                    if items.len() > 1 {
                        let (a, b) = (
                            rng.below(items.len() as u64) as usize,
                            rng.below(items.len() as u64) as usize,
                        );
                        items.swap(a, b);
                    }
                }
            }
            7 => {
                // Remove one item, or insert one with an unused id at a random place.
                if let Some(items) = out["items"].as_array_mut() {
                    if !items.is_empty() && rng.below(2) == 0 {
                        items.remove(rng.below(items.len() as u64) as usize);
                    } else if let Some(id) =
                        (0..8u64).find(|id| !items.iter().any(|item| item["id"] == *id))
                    {
                        let at = rng.below(items.len() as u64 + 1) as usize;
                        items.insert(
                            at,
                            serde_json::json!({ "id": id, "label": "new", "done": false }),
                        );
                    }
                }
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
fn an_update_with_nothing_changed_has_no_patches_and_evaluates_nothing() {
    let state =
        r#"{"title":"t","count":1,"flag":true,"items":[{"id":1,"label":"a","done":false}]}"#;
    let current = program_render(state);
    let before = evaluations();
    let updated = update(&current, &snapshot(state)).expect("updates");
    assert!(updated.patches.is_empty());
    // Nothing the root reads has changed, so the whole tree is the previous
    // one, shared: not a single expression is evaluated.
    assert_eq!(evaluations() - before, 0);
    assert!(std::rc::Rc::ptr_eq(
        &updated.render.tree().root,
        &current.tree().root
    ));
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

fn ops(from: &str, to: &str) -> Vec<String> {
    let updated = update(&program_render(from), &snapshot(to)).expect("updates");
    let doc: Value = serde_json::from_str(&patches_to_json(&updated.patches)).unwrap();
    doc["patches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["op"].as_str().unwrap().to_string())
        .collect()
}

fn list(ids: &[u32]) -> String {
    let items: Vec<String> = ids
        .iter()
        .map(|id| format!(r#"{{"id":{id},"label":"l{id}","done":false}}"#))
        .collect();
    format!(
        r#"{{"title":"t","count":1,"flag":true,"items":[{}]}}"#,
        items.join(",")
    )
}

#[test]
fn a_conditional_switching_is_a_remove_and_an_insert_not_a_replace() {
    let a = r#"{"title":"t","count":1,"flag":true,"items":[]}"#;
    let b = r#"{"title":"t","count":1,"flag":false,"items":[]}"#;
    assert_eq!(ops(a, b), ["remove", "insert"]);
}

#[test]
fn a_list_gaining_losing_and_reordering_items_patches_by_key() {
    assert_eq!(ops(&list(&[1, 2, 3]), &list(&[1, 2, 3, 4])), ["insert"]);
    assert_eq!(ops(&list(&[1, 2, 3]), &list(&[1, 3])), ["remove"]);
    assert_eq!(ops(&list(&[1, 3]), &list(&[1, 2, 3])), ["insert"]);
    // One item moved to the front is one move, whatever the length.
    assert_eq!(
        ops(&list(&[1, 2, 3, 4, 5]), &list(&[5, 1, 2, 3, 4])),
        ["move"]
    );
    // A new order uses moves only, never a rebuild.
    let reversed = ops(&list(&[1, 2, 3, 4]), &list(&[4, 3, 2, 1]));
    assert!(reversed.iter().all(|op| op == "move"), "{reversed:?}");
    // Nothing but the changed item's own props and text change when it stays put.
    assert_eq!(
        ops(&list(&[1, 2, 3]), &list(&[1, 2, 3]).replace("l2", "L2")),
        ["setProp", "setText"]
    );
}

#[test]
fn a_long_list_rotated_is_one_move_and_a_shuffle_is_only_moves() {
    let ids: Vec<u32> = (0..1000).collect();
    let mut rotated = ids.clone();
    rotated.rotate_right(1);
    assert_eq!(ops(&list(&ids), &list(&rotated)), ["move"]);
    // A deterministic shuffle: every kept item is moved or left, never rebuilt.
    let mut shuffled = ids.clone();
    let mut rng = Rng(3);
    for at in (1..shuffled.len()).rev() {
        shuffled.swap(at, rng.below(at as u64 + 1) as usize);
    }
    let shuffle_ops = ops(&list(&ids), &list(&shuffled));
    assert!(shuffle_ops.iter().all(|op| op == "move"), "only moves");
    let mut tree: Value =
        serde_json::from_str(&program_render(&list(&ids)).tree().to_json()).unwrap();
    let updated =
        update(&program_render(&list(&ids)), &snapshot(&list(&shuffled))).expect("updates");
    apply(
        &mut tree,
        &serde_json::from_str(&patches_to_json(&updated.patches)).unwrap(),
    );
    let full: Value =
        serde_json::from_str(&program_render(&list(&shuffled)).tree().to_json()).unwrap();
    assert_eq!(tree, full);
}

#[test]
fn a_moved_item_keeps_its_changes_and_an_inserted_one_brings_its_subtree() {
    let a = list(&[1, 2, 3]);
    let b = list(&[3, 1, 9]).replace("l1", "L1");
    let mut tree: Value = serde_json::from_str(&program_render(&a).tree().to_json()).unwrap();
    let updated = update(&program_render(&a), &snapshot(&b)).expect("updates");
    let patches: Value = serde_json::from_str(&patches_to_json(&updated.patches)).unwrap();
    apply(&mut tree, &patches);
    let full: Value = serde_json::from_str(&program_render(&b).tree().to_json()).unwrap();
    assert_eq!(tree, full);
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

/// A custom element is a primitive like any other: the manifest declares its
/// tag, props and events, and a list or a record prop is carried natively,
/// in render-v1 and in `setProp`, never as text (§9.7.8, §9.8.7).
#[test]
fn a_custom_elements_list_and_record_props_are_carried_natively() {
    const MODEL: &str = r#"{
      "version": 1, "types": {},
      "components": {
        "my-chart": {
          "props": {
            "points": { "type": { "kind": "list", "element": { "kind": "record", "fields": {
                "x": { "type": { "kind": "number" }, "required": true },
                "y": { "type": { "kind": "number" }, "required": true } } } }, "required": true },
            "options": { "type": { "kind": "record", "fields": {
                "title": { "type": { "kind": "string" }, "required": true } } }, "required": true }
          },
          "events": { "pointSelected": { "payload": { "kind": "record", "fields": {
              "index": { "type": { "kind": "number" }, "required": true } } } } },
          "commands": {}, "scope": {} },
        "view": { "props": {}, "events": {},
          "commands": { "show": { "parameters": [ { "name": "at", "type": { "kind": "record", "fields": {
              "index": { "type": { "kind": "number" }, "required": true } } } } ] } },
          "scope": {
            "points": { "kind": "list", "element": { "kind": "record", "fields": {
                "x": { "type": { "kind": "number" }, "required": true },
                "y": { "type": { "kind": "number" }, "required": true } } } },
            "title": { "kind": "string" } } }
      }
    }"#;
    let template = compile_with(
        MODEL,
        "view",
        "<my-chart points={points} options={{ title: title }} on.pointSelected={show($event)} />",
    );
    let texts = [template.as_str()];
    let program = Program {
        root: "view",
        templates: &texts,
    };
    let first = render(
        &program,
        MODEL,
        &snapshot(r#"{"points":[{"x":1,"y":2}],"title":"a"}"#),
    )
    .unwrap_or_else(|d| panic!("renders: {d:#?}"));
    let root: Value = serde_json::from_str(&first.tree().to_json()).unwrap();
    // Carried natively: a list and a record in `props`, and no text for either.
    assert_eq!(
        root["root"]["props"]["points"],
        serde_json::json!([{ "x": 1, "y": 2 }])
    );
    assert_eq!(
        root["root"]["props"]["options"],
        serde_json::json!({ "title": "a" })
    );
    assert!(root["root"].get("propText").is_none());

    let updated = update(
        &first,
        &snapshot(r#"{"points":[{"x":1,"y":2},{"x":3,"y":4}],"title":"a"}"#),
    )
    .expect("updates");
    let doc: Value = serde_json::from_str(&patches_to_json(&updated.patches)).unwrap();
    assert_eq!(doc["patches"].as_array().unwrap().len(), 1, "{doc}");
    assert_eq!(doc["patches"][0]["op"], "setProp");
    assert_eq!(doc["patches"][0]["prop"], "points");
    assert_eq!(
        doc["patches"][0]["value"],
        serde_json::json!([{ "x": 1, "y": 2 }, { "x": 3, "y": 4 }])
    );
    assert!(doc["patches"][0].get("propText").is_none());
    assert!(patch_schema().is_valid(&doc));
}
