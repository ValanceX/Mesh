//! The conformance vectors, natively: `examples/conformance/`.
//!
//! Each directory is a real program (its `program.json`, templates and
//! snapshot, against `examples/conformance/components.json`), the tree
//! the runtime renders from it (`expected.tree.json`, byte for byte), and
//! named cases about that tree:
//!
//! - `values/cases.json`: for chosen props, the value and the `propText`
//!   entry, each present or absent (§9.7.7, §9.8.2);
//! - `events/cases.json`: interactions, and the one binding and intent
//!   each resolves to, or none (§9.9).
//!
//! Nothing here needs a target: an interaction is its innermost node and
//! its applicable event per primitive. `MESH_BLESS=1` rewrites the trees
//! (never the cases); review every rewritten file. The runtime package's
//! `conformance.test.mjs` checks the same files in Node.

mod common;

use mesh_compiler::check;
use mesh_runtime::{
    resolve, HostRecord, Interaction, Node, Program, Render, ResolveError, Tree, TreeChild,
};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/conformance")
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|err| panic!("should read {}: {err}", path.display()))
}

fn json(path: &Path) -> Value {
    serde_json::from_str(&read(path)).expect("the file is JSON")
}

/// Renders the program in `examples/conformance/<name>/`, checks its tree
/// against the committed one (or rewrites it, blessing), and returns it.
fn render(name: &str) -> Render {
    let model = read(&dir().join("components.json"));
    let here = dir().join(name);
    let program = json(&here.join("program.json"));
    let root = program["root"].as_str().unwrap();
    let templates: Vec<String> = program["templates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|component| {
            let component = component.as_str().unwrap();
            let loaded = check::Model::load(&model, component).expect("the manifest loads");
            let compiled = check::template(&read(&here.join(format!("{component}.mprx"))), &loaded);
            assert!(
                compiled.diagnostics.is_empty(),
                "{component}: {:#?}",
                compiled.diagnostics
            );
            mesh_template::to_json(&compiled.template.expect("it compiles"))
        })
        .collect();
    let texts: Vec<&str> = templates.iter().map(String::as_str).collect();
    let snapshot = HostRecord::from_json(&read(&here.join("snapshot.json"))).unwrap();
    let render = mesh_runtime::render(
        &Program {
            root,
            templates: &texts,
        },
        &model,
        &snapshot,
    )
    .unwrap_or_else(|diagnostics| panic!("{name} renders: {diagnostics:#?}"));
    let tree = render.tree().to_json() + "\n";
    let expected = here.join("expected.tree.json");
    if std::env::var_os("MESH_BLESS").is_some() {
        fs::write(&expected, &tree).unwrap();
    } else {
        assert_eq!(read(&expected), tree, "{name}'s tree is the committed one");
    }
    render
}

fn nodes<'t>(node: &'t Node, out: &mut BTreeMap<&'t str, &'t Node>) {
    out.insert(&node.key, node);
    for child in &node.children {
        if let TreeChild::Node(child) = child {
            nodes(child, out);
        }
    }
}

fn texts(node: &Node, out: &mut BTreeMap<String, String>) {
    for child in &node.children {
        match child {
            TreeChild::Node(child) => texts(child, out),
            TreeChild::Text { key, text } => {
                out.insert(key.clone(), text.clone());
            }
        }
    }
}

/// `{ "value": v }` or `{ "absent": true }`, as MESH writes an intent's
/// arguments, for an optional entry.
fn entry<T: Into<Value> + Clone>(found: Option<&T>) -> Value {
    match found {
        Some(value) => serde_json::json!({ "value": value.clone().into() }),
        None => serde_json::json!({ "absent": true }),
    }
}

#[test]
fn values_have_their_committed_props_and_prop_text() {
    let render = render("values");
    let tree: &Tree = render.tree();
    let mut all = BTreeMap::new();
    nodes(&tree.root, &mut all);
    let mut runs = BTreeMap::new();
    texts(&tree.root, &mut runs);
    let cases = json(&dir().join("values/cases.json"));
    let mut failures = Vec::new();
    for case in cases.as_array().unwrap() {
        let name = case["case"].as_str().unwrap();
        if let Some(states) = case.get("states") {
            failures.extend(states_differ(name, states, &written(tree)));
            continue;
        }
        if let Some(run) = case.get("textRun") {
            let actual = runs
                .get(run.as_str().unwrap())
                .expect("the text run exists");
            if case["text"] != *actual {
                failures.push(format!("{name}: text {actual:?}"));
            }
            continue;
        }
        let node = all
            .get(case["node"].as_str().unwrap())
            .unwrap_or_else(|| panic!("{name}: the node exists"));
        let prop = case["prop"].as_str().unwrap();
        let value = entry(node.props.get(prop));
        let text = match node.prop_text.get(prop) {
            Some(text) => serde_json::json!({ "text": text }),
            None => serde_json::json!({ "absent": true }),
        };
        if value != case["value"] || text != case["propText"] {
            failures.push(format!("{name}: prop {value}, propText {text}"));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Each node of the tree as written, by key: its `props`, and its
/// `propText` only if the member is there.
fn written(tree: &Tree) -> BTreeMap<String, Value> {
    fn walk(node: &Value, out: &mut BTreeMap<String, Value>) {
        if node["type"] != "node" {
            return;
        }
        let mut exactly = serde_json::json!({ "props": node["props"] });
        if let Some(text) = node.get("propText") {
            exactly["propText"] = text.clone();
        }
        out.insert(node["key"].as_str().unwrap().to_string(), exactly);
        node["children"]
            .as_array()
            .unwrap()
            .iter()
            .for_each(|c| walk(c, out));
    }
    let document: Value = serde_json::from_str(&tree.to_json()).unwrap();
    let mut out = BTreeMap::new();
    walk(&document["root"], &mut out);
    out
}

/// A `states` case: each state's node is exactly as given, and no two
/// states are written alike, so absent, `null` and a string can't be
/// confused.
fn states_differ(name: &str, states: &Value, written: &BTreeMap<String, Value>) -> Vec<String> {
    let mut failures = Vec::new();
    let states = states.as_array().unwrap();
    for state in states {
        let actual = &written[state["node"].as_str().unwrap()];
        if *actual != state["exactly"] {
            failures.push(format!("{name}: {} is {actual}", state["state"]));
        }
    }
    for (index, a) in states.iter().enumerate() {
        for b in &states[index + 1..] {
            if a["exactly"] == b["exactly"] {
                failures.push(format!(
                    "{name}: {} and {} are alike",
                    a["state"], b["state"]
                ));
            }
        }
    }
    failures
}

#[test]
fn every_list_and_record_prop_has_no_text_and_every_other_non_string_one_does() {
    // Over the whole values tree, not just the named cases: a list or
    // record never acquires a text, a string never gets a duplicate, and
    // a number, boolean or `null` always has one.
    let render = render("values");
    let mut all = BTreeMap::new();
    nodes(&render.tree().root, &mut all);
    let mut seen = [0usize; 3];
    for node in all.values() {
        for (prop, value) in &node.props {
            let text = node.prop_text.get(prop);
            match value {
                Value::Array(_) | Value::Object(_) | Value::String(_) => {
                    assert_eq!(text, None, "{}'s {prop}", node.component);
                    seen[0] += 1;
                }
                Value::Null | Value::Bool(_) | Value::Number(_) => {
                    assert!(text.is_some(), "{}'s {prop}", node.component);
                    seen[1] += 1;
                }
            }
        }
        for prop in node.prop_text.keys() {
            assert!(node.props.contains_key(prop), "no text without a prop");
            seen[2] += 1;
        }
    }
    assert!(seen.iter().all(|&count| count > 0), "{seen:?}");
}

#[test]
fn every_number_prop_text_is_the_content_text_of_the_same_value() {
    // One conversion for content and props: the text run interpolates the
    // same values the props hold.
    let render = render("values");
    let mut runs = BTreeMap::new();
    texts(&render.tree().root, &mut runs);
    let run = runs.values().next().expect("one text run");
    let expected = [
        "Ada",
        "42",
        "0.30000000000000004",
        "1e+21",
        "5e-324",
        "2.9802322387695312e-8",
        "0",
        "true",
        "false",
        "null",
        "[]",
    ]
    .join(", ");
    assert_eq!(run, &expected);
    for (value, text) in [
        (42.0, "42"),
        (0.30000000000000004, "0.30000000000000004"),
        (1e21, "1e+21"),
        (5e-324, "5e-324"),
        // 2^-25, exactly 2.98023223876953125e-8: two candidates are equally near.
        (2f64.powi(-25), "2.9802322387695312e-8"),
        (-0.0, "0"),
    ] {
        assert_eq!(mesh_runtime::number_to_text(value), text);
    }
}

#[test]
fn events_resolve_to_their_committed_binding_and_intent() {
    let render = render("events");
    let cases = json(&dir().join("events/cases.json"));
    let mut failures = Vec::new();
    for case in cases.as_array().unwrap() {
        let name = case["case"].as_str().unwrap();
        let interaction = Interaction {
            target: case["interaction"]["target"].as_str().unwrap().to_string(),
            applicable: serde_json::from_value(case["interaction"]["applicable"].clone())
                .expect("applicable is component to event"),
        };
        let resolved = resolve(render.tree(), &interaction).expect("the target exists");
        let actual = match &resolved {
            None => serde_json::json!({ "none": true }),
            Some(resolved) => {
                // The resolved handler is dispatched exactly once, with the
                // render whose tree it came from: one interaction, one
                // intent.
                let intent = mesh_runtime::dispatch(&render, &resolved.handler, None)
                    .unwrap_or_else(|d| panic!("{name}: dispatch: {d:#?}"));
                serde_json::json!({
                    "node": resolved.node,
                    "event": resolved.event,
                    "handler": resolved.handler,
                    "intent": serde_json::from_str::<Value>(&intent.to_json()).unwrap(),
                })
            }
        };
        if actual != case["expect"] {
            failures.push(format!(
                "{name}:\n  expected {}\n  actual   {actual}",
                case["expect"]
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn resolution_never_reaches_more_than_one_binding_on_the_path() {
    // Every node and text run as the target, with every combination of
    // applicable events for the components that bind any: resolution
    // gives at most one binding, and it is the innermost qualifying one.
    let render = render("events");
    let tree = render.tree();
    let mut all = BTreeMap::new();
    nodes(&tree.root, &mut all);
    let mut targets: Vec<String> = all.keys().map(|key| key.to_string()).collect();
    let mut runs = BTreeMap::new();
    texts(&tree.root, &mut runs);
    targets.extend(runs.into_keys());
    let choices: [(&str, &[&str]); 3] = [
        ("card", &["click", "dismiss"]),
        ("row", &["click", "select"]),
        ("button", &["click", "press"]),
    ];
    let (mut resolved, mut several) = (0, 0);
    for target in &targets {
        for card in [None, Some(0), Some(1)] {
            for row in [None, Some(0), Some(1)] {
                for button in [None, Some(0), Some(1)] {
                    let mut applicable = BTreeMap::new();
                    for ((component, events), pick) in choices.iter().zip([card, row, button]) {
                        if let Some(pick) = pick {
                            applicable.insert(component.to_string(), events[pick].to_string());
                        }
                    }
                    let interaction = Interaction {
                        target: target.clone(),
                        applicable: applicable.clone(),
                    };
                    // Every node on the path that would qualify, innermost
                    // first. Resolution picks the first, and only it.
                    let qualifying: Vec<&Node> = path(&tree.root, target)
                        .into_iter()
                        .rev()
                        .filter(|node| {
                            applicable
                                .get(&node.component)
                                .is_some_and(|event| node.events.contains_key(event))
                        })
                        .collect();
                    let hit = resolve(tree, &interaction).unwrap();
                    match (qualifying.first(), &hit) {
                        (None, None) => {}
                        (Some(first), Some(hit)) => {
                            assert_eq!(first.key, hit.node);
                            let event = &applicable[&first.component];
                            assert_eq!(&hit.event, event);
                            assert_eq!(first.events.get(event), Some(&hit.handler));
                            resolved += 1;
                            if qualifying.len() > 1 {
                                several += 1;
                            }
                        }
                        _ => panic!("{target} {applicable:?}: {hit:?}"),
                    }
                }
            }
        }
    }
    // Some interactions had several qualifying bindings on their path, and
    // still reached only one.
    assert!(resolved > 0 && several > 0, "{resolved} {several}");
}

/// The nodes from the root down to the one `target` names, or the parent
/// of the text run it names.
fn path<'t>(node: &'t Node, target: &str) -> Vec<&'t Node> {
    if node.key == target
        || node
            .children
            .iter()
            .any(|child| matches!(child, TreeChild::Text { key, .. } if key == target))
    {
        return vec![node];
    }
    for child in &node.children {
        if let TreeChild::Node(child) = child {
            let mut below = path(child, target);
            if !below.is_empty() {
                below.insert(0, node);
                return below;
            }
        }
    }
    Vec::new()
}

#[test]
fn an_interaction_on_a_key_the_tree_lacks_is_refused() {
    let render = render("events");
    let interaction = Interaction {
        target: "kAAAAAAAAAAAAAAAAAAAAAA".to_string(),
        applicable: BTreeMap::new(),
    };
    assert_eq!(
        resolve(render.tree(), &interaction),
        Err(ResolveError::UnknownTarget)
    );
}
