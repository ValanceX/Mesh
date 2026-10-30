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
    resolve, HostRecord, HostValue, Interaction, Node, Program, Render, ResolveError,
    RuntimeDiagnostic, Tree, TreeChild,
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
    let here = dir().join(name);
    let render = render_with(name, &read(&here.join("snapshot.json")))
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

/// A program's manifest: its own `components.json` if the directory has
/// one (`enums/` does, so the shared one, and with it every other
/// program's keys, is untouched), else the shared one.
fn model_of(here: &Path) -> PathBuf {
    let own = here.join("components.json");
    if own.is_file() {
        own
    } else {
        dir().join("components.json")
    }
}

/// The program's render for a snapshot given as JSON text, or why it
/// doesn't render.
fn render_with(name: &str, snapshot_text: &str) -> Result<Render, Vec<RuntimeDiagnostic>> {
    let here = dir().join(name);
    let model = &read(&model_of(&here));
    let program = json(&here.join("program.json"));
    let root = program["root"].as_str().unwrap();
    let templates: Vec<String> = program["templates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|component| {
            let component = component.as_str().unwrap();
            let loaded = check::Model::load(model, component).expect("the manifest loads");
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
    let snapshot = HostRecord::from_json(snapshot_text).unwrap();
    mesh_runtime::render(
        &Program {
            root,
            templates: &texts,
        },
        model,
        &snapshot,
    )
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

/// `arguments/`: an intent's argument names. Each case is an event reported on a node, with a
/// payload or none, and the intent it gives: an argument written as a plain path carries its last
/// segment as `name`, any other has none.
#[test]
fn arguments_carry_their_names_as_committed() {
    let render = render("arguments");
    let mut all = BTreeMap::new();
    nodes(&render.tree().root, &mut all);
    let mut failures = Vec::new();
    for case in json(&dir().join("arguments/cases.json"))
        .as_array()
        .unwrap()
    {
        let handler = &all[case["node"].as_str().unwrap()].events[case["event"].as_str().unwrap()];
        let payload = case
            .get("payload")
            .map(|p| HostValue::from_json(&p.to_string()).unwrap());
        let intent = mesh_runtime::dispatch_named(&render, handler, payload.as_ref())
            .unwrap_or_else(|d| panic!("{}: {d:#?}", case["case"]));
        let actual = serde_json::from_str::<Value>(&intent.to_json()).unwrap();
        if actual != case["intent"] {
            failures.push(format!("{}: {actual}", case["case"]));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// `enums/`: the first program with its own manifest. Its `cases.json` is
/// `{ props, rejects, dispatches }`:
///
/// - `props`: a node's prop, present (`value`) or `absent`;
/// - `rejects`: a snapshot that doesn't render, and the first diagnostic's
///   code and location;
/// - `dispatches`: an event reported on a node with a payload, and the
///   intent it gives, or the diagnostic's code and location.
mod enums {
    use super::*;

    /// The `location` member of the diagnostic as the runtime writes it.
    fn location(diagnostic: &RuntimeDiagnostic) -> Value {
        let model = read(&dir().join("enums/components.json"));
        let document: Value = serde_json::from_str(&mesh_runtime::to_json(
            std::slice::from_ref(diagnostic),
            &model,
        ))
        .unwrap();
        document["diagnostics"][0]["location"].clone()
    }

    #[test]
    fn the_props_are_as_committed() {
        let render = render("enums");
        let mut all = BTreeMap::new();
        nodes(&render.tree().root, &mut all);
        let cases = json(&dir().join("enums/cases.json"));
        let mut failures = Vec::new();
        for case in cases["props"].as_array().unwrap() {
            let node = all[case["node"].as_str().unwrap()];
            let value = entry(node.props.get(case["prop"].as_str().unwrap()));
            if value != case["value"] {
                failures.push(format!("{}: {value}", case["case"]));
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }

    #[test]
    fn a_value_that_is_not_a_variant_is_refused_at_the_boundary() {
        let cases = json(&dir().join("enums/cases.json"));
        let mut failures = Vec::new();
        for case in cases["rejects"].as_array().unwrap() {
            let snapshot = case["snapshot"].to_string();
            match render_with("enums", &snapshot) {
                Ok(_) => failures.push(format!("{}: rendered", case["case"])),
                Err(diagnostics) => {
                    let first = &diagnostics[0];
                    if first.code != case["code"] || location(first) != case["location"] {
                        failures.push(format!(
                            "{}: {} at {}",
                            case["case"],
                            first.code,
                            location(first)
                        ));
                    }
                }
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }

    #[test]
    fn an_event_whose_payload_or_argument_is_not_a_variant_is_refused() {
        let render = render("enums");
        let mut all = BTreeMap::new();
        nodes(&render.tree().root, &mut all);
        let cases = json(&dir().join("enums/cases.json"));
        let mut failures = Vec::new();
        for case in cases["dispatches"].as_array().unwrap() {
            let node = all[case["node"].as_str().unwrap()];
            let handler = &node.events[case["event"].as_str().unwrap()];
            let payload = case
                .get("payload")
                .map(|p| HostValue::from_json(&p.to_string()).unwrap());
            let actual = match mesh_runtime::dispatch(&render, handler, payload.as_ref()) {
                Ok(intent) => {
                    serde_json::json!({ "intent": serde_json::from_str::<Value>(&intent.to_json()).unwrap() })
                }
                Err(diagnostics) => serde_json::json!({
                    "code": diagnostics[0].code,
                    "location": location(&diagnostics[0]),
                }),
            };
            if actual != case["expect"] {
                failures.push(format!("{}: {actual}", case["case"]));
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }
}

/// `snippets/`: typed template values (decisions U1 to U14), the program of `snippets/components.json`
/// rendered for a snapshot and then for each `step-N.snapshot.json` after it, each against its committed
/// tree (`expected.tree.json`, `step-N.expected.tree.json`). `cases.json` is `{ distinct, same, … }`:
/// relations between the keys of nodes, which a node's place in a tree names by the indexes of the
/// nodes (not text runs) on the way down from the root.
/// The vectors of a directory of programs that use snippets: every step's tree, update equal to render, and the keys the cases pin.
macro_rules! snippet_vectors {
    ($name:ident, $dir:literal) => {
        mod $name {
            use super::*;

            fn steps() -> Vec<(String, String)> {
                let mut steps = vec![(
                    "snapshot.json".to_string(),
                    "expected.tree.json".to_string(),
                )];
                let mut at = 1;
                while dir()
                    .join(format!("{}/step-{at}.snapshot.json", $dir))
                    .is_file()
                {
                    steps.push((
                        format!("step-{at}.snapshot.json"),
                        format!("step-{at}.expected.tree.json"),
                    ));
                    at += 1;
                }
                steps
            }

            /// Renders `snapshot` of the program, comparing with the committed tree `expected` (or blessing).
            fn rendered(snapshot: &str, expected: &str) -> Render {
                let here = dir().join($dir);
                let render = render_with($dir, &read(&here.join(snapshot)))
                    .unwrap_or_else(|d| panic!("{snapshot} renders: {d:#?}"));
                let tree = render.tree().to_json() + "\n";
                if std::env::var_os("MESH_BLESS").is_some() {
                    fs::write(here.join(expected), &tree).unwrap();
                } else {
                    assert_eq!(
                        read(&here.join(expected)),
                        tree,
                        "{snapshot}'s tree is the committed one"
                    );
                }
                render
            }

            fn key_at(tree: &Tree, path: &[usize]) -> String {
                let mut node = &tree.root;
                for step in path {
                    let child = node
                        .children
                        .iter()
                        .filter_map(|child| match child {
                            TreeChild::Node(child) => Some(child),
                            TreeChild::Text { .. } => None,
                        })
                        .nth(*step)
                        .unwrap_or_else(|| panic!("no node {path:?}"));
                    node = child;
                }
                node.key.clone()
            }

            fn path(value: &Value) -> Vec<usize> {
                value
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|step| step.as_u64().unwrap() as usize)
                    .collect()
            }

            #[test]
            fn every_step_renders_its_committed_tree() {
                for (snapshot, expected) in steps() {
                    rendered(&snapshot, &expected);
                }
            }

            #[test]
            fn updating_from_each_step_to_the_next_is_the_render_of_the_next() {
                let all = steps();
                let mut previous = rendered(&all[0].0, &all[0].1);
                for (snapshot, expected) in &all[1..] {
                    let fresh = rendered(snapshot, expected);
                    let record =
                        HostRecord::from_json(&read(&dir().join($dir).join(snapshot))).unwrap();
                    let updated = mesh_runtime::update(&previous, &record)
                        .unwrap_or_else(|d| panic!("{snapshot} updates: {d:#?}"));
                    assert_eq!(
                        updated.render.tree(),
                        fresh.tree(),
                        "an update to {snapshot} is its render"
                    );
                    previous = updated.render;
                }
            }

            #[test]
            fn keys_are_distinct_or_the_same_as_committed() {
                let all = steps();
                let trees: Vec<Render> = all.iter().map(|(s, e)| rendered(s, e)).collect();
                let cases = json(&dir().join(concat!($dir, "/cases.json")));
                let mut failures = Vec::new();
                for case in cases["distinct"].as_array().unwrap() {
                    let tree = trees[case["step"].as_u64().unwrap() as usize].tree();
                    let keys: Vec<String> = case["paths"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|p| key_at(tree, &path(p)))
                        .collect();
                    let mut unique = keys.clone();
                    unique.sort();
                    unique.dedup();
                    if unique.len() != keys.len() {
                        failures.push(format!("{}: two of {keys:?} are one key", case["case"]));
                    }
                }
                for case in cases["same"].as_array().unwrap() {
                    let at = |side: &str| {
                        let side = &case[side];
                        key_at(
                            trees[side["step"].as_u64().unwrap() as usize].tree(),
                            &path(&side["path"]),
                        )
                    };
                    if at("a") != at("b") {
                        failures.push(format!("{}: the keys differ", case["case"]));
                    }
                }
                for case in cases["different"].as_array().unwrap() {
                    let at = |side: &str| {
                        let side = &case[side];
                        key_at(
                            trees[side["step"].as_u64().unwrap() as usize].tree(),
                            &path(&side["path"]),
                        )
                    };
                    if at("a") == at("b") {
                        failures.push(format!("{}: the keys are one", case["case"]));
                    }
                }
                assert!(failures.is_empty(), "{failures:#?}");
            }
        }
    };
}

snippet_vectors!(snippets, "snippets");
snippet_vectors!(forwarder, "forwarder");
snippet_vectors!(passing, "passing");

/// `units/`: a component's state for one occurrence, which a render takes beside the snapshot (docs/manual/runtime.md, "Units").
/// `steps.json` is a list of `{ case, snapshot, units }`; each step's committed outputs are `step-N.expected.tree.json` and
/// `step-N.expected.occurrences.json` (the occurrences the render lists). An update from a step to the next, given the next's units,
/// is the render of the next.
mod units {
    use super::*;
    use mesh_runtime::Units;

    fn steps() -> Vec<Value> {
        json(&dir().join("units/steps.json"))
            .as_array()
            .unwrap()
            .clone()
    }

    fn compiled() -> (String, String, Vec<String>) {
        let here = dir().join("units");
        let model = read(&here.join("components.json"));
        let program = json(&here.join("program.json"));
        let templates = program["templates"]
            .as_array()
            .unwrap()
            .iter()
            .map(|component| {
                let component = component.as_str().unwrap();
                let loaded = check::Model::load(&model, component).expect("the manifest loads");
                let compiled =
                    check::template(&read(&here.join(format!("{component}.mprx"))), &loaded);
                assert!(
                    compiled.diagnostics.is_empty(),
                    "{component}: {:#?}",
                    compiled.diagnostics
                );
                mesh_template::to_json(&compiled.template.expect("it compiles"))
            })
            .collect();
        (
            program["root"].as_str().unwrap().to_string(),
            model,
            templates,
        )
    }

    fn render_step(step: &Value) -> Render {
        let (root, model, templates) = compiled();
        let texts: Vec<&str> = templates.iter().map(String::as_str).collect();
        let snapshot = HostRecord::from_json(&step["snapshot"].to_string()).unwrap();
        let units = Units::from_json(&step["units"].to_string()).unwrap();
        mesh_runtime::render_units(
            &Program {
                root: &root,
                templates: &texts,
            },
            &model,
            &snapshot,
            &units,
        )
        .unwrap_or_else(|d| panic!("{} renders: {d:#?}", step["case"]))
    }

    fn committed(at: usize, what: &str, text: &str) {
        let file = dir().join(format!("units/step-{at}.expected.{what}.json"));
        if std::env::var_os("MESH_BLESS").is_some() {
            fs::write(&file, text).unwrap();
        } else {
            assert_eq!(
                read(&file),
                text,
                "step {at}'s {what} are the committed ones"
            );
        }
    }

    #[test]
    fn every_step_renders_its_committed_tree_and_lists_its_committed_occurrences() {
        for (at, step) in steps().iter().enumerate() {
            let render = render_step(step);
            committed(at, "tree", &(render.tree().to_json() + "\n"));
            committed(at, "occurrences", &(render.occurrences_to_json() + "\n"));
        }
    }

    #[test]
    fn updating_from_each_step_to_the_next_with_its_units_is_the_render_of_the_next() {
        let all = steps();
        let mut previous = render_step(&all[0]);
        for step in &all[1..] {
            let record = HostRecord::from_json(&step["snapshot"].to_string()).unwrap();
            let units = Units::from_json(&step["units"].to_string()).unwrap();
            let updated = mesh_runtime::update_with_units(&previous, record, Some(&units))
                .unwrap_or_else(|d| panic!("{} updates: {d:#?}", step["case"]));
            let fresh = render_step(step);
            assert_eq!(
                updated.render.tree(),
                fresh.tree(),
                "an update to {} is its render",
                step["case"]
            );
            assert_eq!(
                updated.render.occurrences_to_json(),
                fresh.occurrences_to_json(),
                "and lists what it lists"
            );
            previous = updated.render;
        }
    }
}

/// `context/`: the context channel, values a host gives beside the snapshot that a component reads only if
/// its manifest entry `uses` them (docs/manual/runtime.md, "Context"). `steps.json` is a list of
/// `{ case, snapshot, context, changes }`; each step renders its committed tree (`step-N.expected.tree.json`),
/// and from the step before, an update by `changes` (whose paths into the context start `{ "context": name }`)
/// and an update with the whole snapshot and context are both the render of the step. `cases.json` is
/// `{ checks, manifests, rejects, dispatches }`: templates and manifests that fail to check, with the code;
/// context values a render refuses, with the first diagnostic's code and location; and the intent a handler that
/// passes a context value gives.
mod context {
    use super::*;
    use mesh_runtime::{Changes, Units};

    fn here() -> PathBuf {
        dir().join("context")
    }

    fn compiled() -> (String, String, Vec<String>) {
        let model = read(&here().join("components.json"));
        let program = json(&here().join("program.json"));
        let templates = program["templates"]
            .as_array()
            .unwrap()
            .iter()
            .map(|component| {
                let component = component.as_str().unwrap();
                let loaded = check::Model::load(&model, component).expect("the manifest loads");
                let compiled =
                    check::template(&read(&here().join(format!("{component}.mprx"))), &loaded);
                assert!(
                    compiled.diagnostics.is_empty(),
                    "{component}: {:#?}",
                    compiled.diagnostics
                );
                mesh_template::to_json(&compiled.template.expect("it compiles"))
            })
            .collect();
        (
            program["root"].as_str().unwrap().to_string(),
            model,
            templates,
        )
    }

    fn record(value: &Value) -> HostRecord {
        HostRecord::from_json(&value.to_string()).unwrap()
    }

    fn render_with(snapshot: &Value, context: &Value) -> Result<Render, Vec<RuntimeDiagnostic>> {
        let (root, model, templates) = compiled();
        let texts: Vec<&str> = templates.iter().map(String::as_str).collect();
        mesh_runtime::render_context(
            &Program {
                root: &root,
                templates: &texts,
            },
            &model,
            &record(snapshot),
            &Units::default(),
            &record(context),
        )
    }

    fn steps() -> Vec<Value> {
        json(&here().join("steps.json")).as_array().unwrap().clone()
    }

    fn cases() -> Value {
        json(&here().join("cases.json"))
    }

    #[test]
    fn every_step_renders_its_committed_tree() {
        for (at, step) in steps().iter().enumerate() {
            let render = render_with(&step["snapshot"], &step["context"])
                .unwrap_or_else(|d| panic!("{} renders: {d:#?}", step["case"]));
            let text = render.tree().to_json() + "\n";
            let file = here().join(format!("step-{at}.expected.tree.json"));
            if std::env::var_os("MESH_BLESS").is_some() {
                fs::write(&file, &text).unwrap();
            } else {
                assert_eq!(read(&file), text, "step {at}'s tree is the committed one");
            }
        }
    }

    #[test]
    fn an_update_by_changes_or_by_whole_inputs_is_the_render_of_the_next_step() {
        let all = steps();
        let mut by_changes = render_with(&all[0]["snapshot"], &all[0]["context"]).unwrap();
        let mut by_whole = by_changes.clone();
        for step in &all[1..] {
            let fresh = render_with(&step["snapshot"], &step["context"]).unwrap();
            let document =
                serde_json::json!({ "base": by_changes.version(), "changes": step["changes"] });
            let changes =
                Changes::from_host(&HostValue::from_json(&document.to_string()).unwrap()).unwrap();
            let updated = mesh_runtime::update_changes(&by_changes, &changes, None)
                .unwrap_or_else(|d| panic!("{} updates by changes: {d:#?}", step["case"]));
            assert_eq!(
                updated.render.tree(),
                fresh.tree(),
                "{}: by changes",
                step["case"]
            );
            assert_eq!(
                updated.render.context_to_json(),
                step["context"].to_string()
            );
            // The patches turn the previous tree into this one: a context change re-evaluates its readers.
            assert!(
                !updated.patches.is_empty(),
                "{}: something changed",
                step["case"]
            );
            let whole = mesh_runtime::update_with_context(
                &by_whole,
                record(&step["snapshot"]),
                None,
                Some(&record(&step["context"])),
            )
            .unwrap_or_else(|d| panic!("{} updates: {d:#?}", step["case"]));
            assert_eq!(whole.render.tree(), fresh.tree(), "{}: whole", step["case"]);
            by_changes = updated.render;
            by_whole = whole.render;
        }
    }

    #[test]
    fn templates_and_manifests_that_misuse_the_context_do_not_check() {
        let model = read(&here().join("components.json"));
        for case in cases()["checks"].as_array().unwrap() {
            let loaded = check::Model::load(&model, case["component"].as_str().unwrap()).unwrap();
            let compiled = check::template(case["source"].as_str().unwrap(), &loaded);
            let codes: Vec<&str> = compiled
                .diagnostics
                .iter()
                .map(|d| d.code.as_str())
                .collect();
            assert_eq!(codes, [case["code"].as_str().unwrap()], "{}", case["case"]);
        }
        for case in cases()["manifests"].as_array().unwrap() {
            let found = mesh_manifest::load(&case["manifest"].to_string()).unwrap_err();
            let codes: Vec<&str> = found.iter().map(|d| d.code.as_str()).collect();
            assert_eq!(codes, [case["code"].as_str().unwrap()], "{}", case["case"]);
        }
    }

    #[test]
    fn context_values_that_do_not_fit_are_refused_at_their_path() {
        for case in cases()["rejects"].as_array().unwrap() {
            let found = render_with(&case["snapshot"], &case["context"]).unwrap_err();
            let document: Value = serde_json::from_str(&mesh_runtime::to_json(
                &found,
                &read(&here().join("components.json")),
            ))
            .unwrap();
            let first = &document["diagnostics"][0];
            assert_eq!(first["code"], case["code"], "{}", case["case"]);
            assert_eq!(first["location"], case["location"], "{}", case["case"]);
        }
    }

    #[test]
    fn a_handler_that_passes_a_context_value_gives_the_committed_intent() {
        for case in cases()["dispatches"].as_array().unwrap() {
            let step = &steps()[case["step"].as_u64().unwrap() as usize];
            let render = render_with(&step["snapshot"], &step["context"]).unwrap();
            let mut all = BTreeMap::new();
            nodes(&render.tree().root, &mut all);
            let node = all
                .values()
                .find(|node| node.component == case["component"].as_str().unwrap())
                .unwrap();
            let handler = &node.events[case["event"].as_str().unwrap()];
            let intent = mesh_runtime::dispatch_named(&render, handler, None).unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(&intent.to_json()).unwrap(),
                case["intent"],
                "{}",
                case["case"]
            );
        }
    }
}
