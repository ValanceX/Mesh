//! Render trees, keys and handler identifiers (D4, D6, I13), and the
//! program identity they're salted with (docs/manual/runtime.md).

mod common;

use common::{compile, snapshot, try_render, view, SNAPSHOT};
use mesh_runtime::{Node, Render, TreeChild};
use serde_json::Value;
use sha2::{Digest, Sha256};

const PAGE: &str = r#"<page title={name}>
  <text>Hello, {name}!</text>
  <probe num={count} on.tap={save()} />
  <probe num={count + 1} on.tap={save()} on.pick={take($event)} />
  <text>{maybeName}</text>
</page>"#;

fn schema() -> jsonschema::Validator {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/render-v1.schema.json");
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    jsonschema::draft202012::new(&schema).unwrap()
}

/// Every key and handler identifier in a tree, in document order.
fn identities(node: &Node, keys: &mut Vec<String>, handlers: &mut Vec<String>) {
    keys.push(node.key.clone());
    handlers.extend(node.events.values().cloned());
    for child in &node.children {
        match child {
            TreeChild::Node(child) => identities(child, keys, handlers),
            TreeChild::Text { key, .. } => keys.push(key.clone()),
        }
    }
}

fn all(render: &Render) -> (Vec<String>, Vec<String>) {
    let (mut keys, mut handlers) = (Vec::new(), Vec::new());
    identities(&render.tree().root, &mut keys, &mut handlers);
    (keys, handlers)
}

/// The tree's shape: components, props' and events' names, and text runs,
/// without values, keys or handler identifiers.
fn shape(node: &Node) -> String {
    let children: Vec<String> = node
        .children
        .iter()
        .map(|child| match child {
            TreeChild::Node(child) => shape(child),
            TreeChild::Text { .. } => "text".into(),
        })
        .collect();
    format!(
        "{}{:?}{:?}[{}]",
        node.component,
        node.props.keys().collect::<Vec<_>>(),
        node.events.keys().collect::<Vec<_>>(),
        children.join(",")
    )
}

#[test]
fn a_tree_is_render_v1() {
    let render = view(PAGE).unwrap();
    let document: Value = serde_json::from_str(&render.tree().to_json()).unwrap();
    let errors: Vec<String> = schema()
        .iter_errors(&document)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{errors:#?}");
}

#[test]
fn text_runs_join_adjacent_text_and_interpolations_and_are_kept_when_empty() {
    let render = view(PAGE).unwrap();
    let texts: Vec<Vec<String>> = render
        .tree()
        .root
        .children
        .iter()
        .filter_map(|child| match child {
            TreeChild::Node(node) if node.component == "text" => Some(
                node.children
                    .iter()
                    .map(|c| match c {
                        TreeChild::Text { text, .. } => text.clone(),
                        TreeChild::Node(_) => panic!("text holds only a run"),
                    })
                    .collect(),
            ),
            _ => None,
        })
        .collect();
    assert_eq!(
        texts,
        [vec!["Hello, Ada!".to_string()], vec![String::new()]]
    );
}

#[test]
fn keys_and_handler_identifiers_are_unique_and_have_their_shape() {
    let render = view(PAGE).unwrap();
    let (keys, handlers) = all(&render);
    let mut unique = keys.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), keys.len(), "keys are unique");
    let mut unique = handlers.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), handlers.len(), "two probes' `tap`s differ");
    assert_eq!(handlers.len(), 3);
    for key in &keys {
        assert!(key.len() == 23 && key.starts_with('k'), "{key}");
    }
    for handler in &handlers {
        assert!(
            handler.len() == 35 && handler.starts_with('h') && &handler[12..13] == ".",
            "{handler}"
        );
    }
}

#[test]
fn every_render_of_a_program_has_the_same_structure_keys_and_handlers() {
    let first = view(PAGE).unwrap();
    let other = SNAPSHOT
        .replace(
            "\"name\": \"Ada\",\n",
            "\"name\": \"Grace\",\n  \"maybeName\": \"G\",\n",
        )
        .replace("\"count\": 3", "\"count\": -12.5");
    let second = try_render("view", &[compile("view", PAGE)], &other).unwrap();
    assert_ne!(first.tree(), second.tree(), "the values differ");
    assert_eq!(all(&first), all(&second));
    assert_eq!(shape(&first.tree().root), shape(&second.tree().root));
}

#[test]
fn keys_are_salted_with_the_program() {
    let a = view(PAGE).unwrap();
    let b = view(&PAGE.replace("Hello", "Hi")).unwrap();
    assert_eq!(
        shape(&a.tree().root),
        shape(&b.tree().root),
        "the same structure"
    );
    let (keys_a, handlers_a) = all(&a);
    let (keys_b, handlers_b) = all(&b);
    for (x, y) in keys_a.iter().zip(&keys_b) {
        assert_ne!(x, y, "a key is stable only within one program");
    }
    for (x, y) in handlers_a.iter().zip(&handlers_b) {
        assert_ne!(
            x[..12],
            y[..12],
            "the program prefix changes with the program"
        );
    }
}

#[test]
fn the_program_identity_ignores_compiler_whitespace_and_key_order() {
    let template = compile("view", PAGE);
    let reference = all(&try_render("view", std::slice::from_ref(&template), SNAPSHOT).unwrap());
    // Another compiler version, pretty-printed: the same program.
    let mut value: Value = serde_json::from_str(&template).unwrap();
    value["compiler"] = Value::String("0.0.1-other".into());
    let pretty = serde_json::to_string_pretty(&value).unwrap();
    assert_eq!(
        all(&try_render("view", &[pretty], SNAPSHOT).unwrap()),
        reference
    );
    // A span moved (the source reformatted): another program.
    let moved = template.replacen("\"byte\":0,", "\"byte\":1,", 1);
    assert_ne!(moved, template);
    assert_ne!(
        all(&try_render("view", &[moved], SNAPSHOT).unwrap()),
        reference
    );
}

#[test]
fn no_key_or_handler_identifier_holds_a_name() {
    let render = view(PAGE).unwrap();
    let (keys, handlers) = all(&render);
    for identity in keys.iter().chain(&handlers) {
        let lower = identity.to_lowercase();
        for name in [
            "view", "page", "probe", "text", "save", "take", "name", "count", "tap", "pick",
        ] {
            assert!(!lower.contains(name), "{identity} contains {name}");
        }
    }
}

/// I13: a tree holds exactly primitive component names, props with
/// values, text runs, event names with handler identifiers, and keys.
#[test]
fn a_tree_holds_only_what_i13_lists() {
    fn walk(node: &Value) {
        let object = node.as_object().unwrap();
        let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
        keys.sort_unstable();
        match object["type"].as_str().unwrap() {
            "node" => {
                assert_eq!(
                    keys,
                    ["children", "component", "events", "key", "props", "type"]
                );
                for handler in object["events"].as_object().unwrap().values() {
                    assert!(handler.is_string());
                }
                object["children"].as_array().unwrap().iter().for_each(walk);
            }
            "text" => assert_eq!(keys, ["key", "text", "type"]),
            other => panic!("{other}"),
        }
    }
    let render = view(PAGE).unwrap();
    let document: Value = serde_json::from_str(&render.tree().to_json()).unwrap();
    assert_eq!(document.as_object().unwrap().len(), 3);
    walk(&document["root"]);
    let text = render.tree().to_json();
    for leak in [
        "$event",
        "save",
        "take",
        "\"scope\"",
        "\"span\"",
        "view",
        "template",
        "compiler",
    ] {
        assert!(!text.contains(leak), "the tree leaks {leak}");
    }
}

// --- the manual's encodings, recomputed ------------------------------------

fn canonical(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&mesh_runtime::number_to_text(n.as_f64().unwrap())),
        Value::String(s) => out.push_str(&serde_json::to_string(s).unwrap()),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                canonical(item, out);
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut members: Vec<_> = map.iter().collect();
            members.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
            out.push('{');
            for (i, (k, v)) in members.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(k).unwrap());
                out.push(':');
                canonical(v, out);
            }
            out.push('}');
        }
    }
}

fn string(hash: &mut Sha256, text: &str) {
    hash.update(u32::try_from(text.len()).unwrap().to_be_bytes());
    hash.update(text.as_bytes());
}

fn base64url(bytes: &[u8]) -> String {
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut bits = String::new();
    for byte in bytes {
        bits.push_str(&format!("{byte:08b}"));
    }
    bits.as_bytes()
        .chunks(6)
        .map(|chunk| {
            let mut n = 0;
            for (i, bit) in chunk.iter().enumerate() {
                n |= usize::from(*bit == b'1') << (5 - i);
            }
            char::from(A[n])
        })
        .collect()
}

/// A key and a handler identifier, computed from runtime.md's layout
/// alone, equal the runtime's.
#[test]
fn keys_and_handlers_match_the_manual() {
    let source = "<probe on.tap={save()} />";
    let template = compile("view", source);
    let render = try_render("view", std::slice::from_ref(&template), SNAPSHOT).unwrap();

    let mut value: Value = serde_json::from_str(&template).unwrap();
    value.as_object_mut().unwrap().remove("compiler");
    let mut text = String::new();
    canonical(&value, &mut text);
    let digest = Sha256::digest(text.as_bytes());

    let mut hash = Sha256::new();
    string(&mut hash, "mesh-program-v1");
    string(&mut hash, "view");
    hash.update(1u32.to_be_bytes());
    string(&mut hash, "view");
    hash.update(digest);
    let identity = hash.finalize();

    let mut hash = Sha256::new();
    string(&mut hash, "mesh-key-v1");
    hash.update(identity);
    hash.update(1u32.to_be_bytes());
    hash.update(0u32.to_be_bytes());
    hash.update([0x01]);
    string(&mut hash, "probe");
    let key = format!("k{}", base64url(&hash.finalize()[..16]));
    assert_eq!(render.tree().root.key, key);

    let mut hash = Sha256::new();
    string(&mut hash, "mesh-handler-v1");
    hash.update(identity);
    string(&mut hash, &key);
    string(&mut hash, "tap");
    let handler = format!(
        "h{}.{}",
        base64url(&identity[..8]),
        base64url(&hash.finalize()[..16])
    );
    assert_eq!(render.tree().root.events["tap"], handler);
}

#[test]
fn a_render_keeps_its_own_copy_of_the_snapshot() {
    let mut record = snapshot(SNAPSHOT);
    let template = compile("view", PAGE);
    let render = mesh_runtime::render(
        &mesh_runtime::Program {
            root: "view",
            templates: &[&template],
        },
        common::MODEL,
        &record,
    )
    .unwrap();
    let before = render.tree().clone();
    record.0.clear();
    assert_eq!(render.tree(), &before);
}
