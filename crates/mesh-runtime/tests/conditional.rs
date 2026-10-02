//! The §9.10 conditional tracer: can one program produce render trees whose
//! structure differs, while identity keeps the contract's rules?
//!
//! PROVISIONAL. The conditional here is `<mesh-if when={…}>`, an ordinary
//! element of a component the model declares, with one or two element
//! children (the alternatives); the runtime, not the compiler, gives it
//! meaning, and it is never a node of the tree. Its spelling is not the
//! language's decision. What these tests pin is the *semantics*:
//! alternatives are distinct sites, an identity doesn't depend on where a
//! node lands in the rendered children, a node that disappears is gone and
//! one that returns has the same identity, and a handler belongs to its
//! node's identity and its event. The §9.10 vectors
//! (`examples/conformance/identity/`) remain the contract; this shows the
//! implementation can satisfy it.

mod common;

use common::{codes, compile_with, snapshot};
use mesh_runtime::{dispatch, Node, Program, Render, TreeChild};
use std::collections::BTreeMap;

const MODEL: &str = r#"{
  "version": 1, "types": {},
  "components": {
    "page":    { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "note":    { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "button":  { "props": {}, "events": { "tap": {}, "hold": {} }, "commands": {}, "scope": {} },
    "chip":    { "props": {}, "events": { "tap": {} }, "commands": {}, "scope": {} },
    "mesh-if": { "props": { "when": { "type": { "kind": "boolean" }, "required": true } }, "events": {}, "commands": {}, "scope": {} },
    "view": { "props": {}, "events": {},
      "commands": { "pick": { "parameters": [] }, "drop": { "parameters": [] }, "hold": { "parameters": [] } },
      "scope": { "show": { "kind": "boolean" }, "lead": { "kind": "boolean" } } },
    "solo": { "props": {}, "events": {},
      "commands": { "pick": { "parameters": [] } },
      "scope": { "show": { "kind": "boolean" } } },
    "twin": { "props": {}, "events": {}, "commands": {}, "scope": { "show": { "kind": "boolean" } } },
    "bad":  { "props": {}, "events": {}, "commands": {}, "scope": { "show": { "kind": "boolean" } } }
  }
}"#;

/// Two alternatives of different components, between static siblings, with a
/// second conditional before them that can insert a sibling.
const VIEW: &str = "<page>\
  <mesh-if when={lead}><note>lead</note></mesh-if>\
  <note>before</note>\
  <mesh-if when={show}>\
    <button on.tap={pick()} on.hold={hold()}>A</button>\
    <chip on.tap={drop()}>B</chip>\
  </mesh-if>\
  <note>after</note>\
</page>";

/// One alternative: when `show` is false there is no occurrence.
const SOLO: &str = "<page>\
  <note>before</note>\
  <mesh-if when={show}><button on.tap={pick()}>A</button></mesh-if>\
  <note>after</note>\
</page>";

/// Two alternatives of the SAME component in the same place.
const TWIN: &str = "<page><mesh-if when={show}><note>one</note><note>two</note></mesh-if></page>";

fn render(root: &str, source: &str, values: &str) -> Render {
    let template = compile_with(MODEL, root, source);
    let templates = [template.as_str()];
    mesh_runtime::render(
        &Program {
            root,
            templates: &templates,
        },
        MODEL,
        &snapshot(values),
    )
    .unwrap_or_else(|diagnostics| panic!("{root} renders: {diagnostics:#?}"))
}

fn view(show: bool, lead: bool) -> Render {
    render("view", VIEW, &format!(r#"{{"show":{show},"lead":{lead}}}"#))
}

fn solo(show: bool) -> Render {
    render("solo", SOLO, &format!(r#"{{"show":{show}}}"#))
}

/// One node of a tree: what it is, where it ended up among its parent's
/// rendered children, and its handlers.
#[derive(Debug, Clone)]
struct Part {
    key: String,
    component: String,
    text: String,
    rendered_index: usize,
    events: BTreeMap<String, String>,
}

fn parts(render: &Render) -> Vec<Part> {
    fn go(node: &Node, index: usize, out: &mut Vec<Part>) {
        let text = node
            .children
            .iter()
            .filter_map(|child| match child {
                TreeChild::Text { text, .. } => Some(text.as_str()),
                TreeChild::Node(_) => None,
            })
            .collect::<String>();
        out.push(Part {
            key: node.key.clone(),
            component: node.component.clone(),
            text,
            rendered_index: index,
            events: node.events.clone(),
        });
        for (i, child) in node.children.iter().enumerate() {
            if let TreeChild::Node(child) = child {
                go(child, i, out);
            }
        }
    }
    let mut out = Vec::new();
    go(&render.tree().root, 0, &mut out);
    out
}

/// The node of `component` showing `text`, if the render has one.
fn find(render: &Render, component: &str, text: &str) -> Option<Part> {
    parts(render)
        .into_iter()
        .find(|part| part.component == component && part.text == text)
}

fn must(render: &Render, component: &str, text: &str) -> Part {
    find(render, component, text).unwrap_or_else(|| panic!("no {component} `{text}`"))
}

fn keys(render: &Render) -> Vec<String> {
    parts(render).into_iter().map(|part| part.key).collect()
}

// --- T1: the same branch, the same identity -----------------------------------

#[test]
fn a_branch_has_a_deterministic_identity() {
    let first = view(true, false);
    let again = view(true, false);
    assert_eq!(
        keys(&first),
        keys(&again),
        "same program and snapshot, same identities"
    );
    assert_eq!(first.tree().to_json(), again.tree().to_json());

    // Not a function of what was rendered before.
    let _other = view(false, true);
    let later = view(true, false);
    assert_eq!(
        must(&first, "button", "A").key,
        must(&later, "button", "A").key
    );

    // The conditional is never a node: the tree has what it chose.
    assert!(parts(&first).iter().all(|part| part.component != "mesh-if"));
}

// --- T2: alternatives are distinct sites ---------------------------------------

#[test]
fn the_alternatives_are_distinct_sites() {
    let on = view(true, false);
    let off = view(false, false);
    let a = must(&on, "button", "A");
    let b = must(&off, "chip", "B");

    assert!(find(&on, "chip", "B").is_none() && find(&off, "button", "A").is_none());
    assert_ne!(
        a.key, b.key,
        "A and B are different nodes though they share a place"
    );
    assert_ne!(a.events["tap"], b.events["tap"]);
    // They share a rendered position, which is exactly why position can't be identity.
    assert_eq!(a.rendered_index, b.rendered_index);
}

#[test]
fn alternatives_of_one_component_are_still_distinct_sites() {
    let on = render("twin", TWIN, r#"{"show":true}"#);
    let off = render("twin", TWIN, r#"{"show":false}"#);
    let (one, two) = (must(&on, "note", "one"), must(&off, "note", "two"));

    assert_eq!(one.component, two.component);
    assert_eq!(one.rendered_index, two.rendered_index);
    assert_ne!(
        one.key, two.key,
        "the same component in the same place is two sites, not one"
    );
}

// --- T3, T4: disappearance and return -----------------------------------------

#[test]
fn a_branch_that_is_off_has_no_node() {
    let on = solo(true);
    let off = solo(false);

    assert!(find(&on, "button", "A").is_some());
    assert!(
        find(&off, "button", "A").is_none(),
        "no render node remains for A"
    );
    assert!(
        parts(&off).iter().all(|part| part.events.is_empty()),
        "and none of its handlers"
    );
    // What was static stays, and keeps its identity.
    assert_eq!(
        must(&on, "note", "before").key,
        must(&off, "note", "before").key
    );
    assert_eq!(
        must(&on, "note", "after").key,
        must(&off, "note", "after").key
    );
}

#[test]
fn a_returning_branch_has_the_same_identity_and_is_a_new_occurrence() {
    let (first, gone, back) = (solo(true), solo(false), solo(true));
    let key = must(&first, "button", "A").key;

    // The occurrence ends: by key, the tree that renderers see removes it …
    assert!(!keys(&gone).contains(&key));
    // … and when it returns it is the same identity, so, by key, it is created.
    assert!(keys(&back).contains(&key));
    let removed: Vec<_> = keys(&first)
        .into_iter()
        .filter(|k| !keys(&gone).contains(k))
        .collect();
    let created: Vec<_> = keys(&back)
        .into_iter()
        .filter(|k| !keys(&gone).contains(k))
        .collect();
    assert_eq!(removed, std::slice::from_ref(&key));
    assert!(created.contains(&key));

    // The runtime carries nothing from the first occurrence into the third:
    // a render is a function of the program and the snapshot alone.
    assert_eq!(first.tree().to_json(), back.tree().to_json());
}

// --- T5: the most important case ----------------------------------------------

#[test]
fn a_sibling_inserted_before_does_not_change_identity() {
    let plain = view(true, false);
    let inserted = view(true, true);

    for (component, text) in [("button", "A"), ("note", "before"), ("note", "after")] {
        let (was, now) = (
            must(&plain, component, text),
            must(&inserted, component, text),
        );
        assert_eq!(was.key, now.key, "{component} `{text}` keeps its identity");
        assert_eq!(was.events, now.events);
        // The test means something only if the rendered position did move.
        assert_eq!(
            now.rendered_index,
            was.rendered_index + 1,
            "{component} `{text}` moved one place in the rendered children"
        );
    }
    let lead = must(&inserted, "note", "lead");
    assert!(
        !keys(&plain).contains(&lead.key),
        "the inserted node is new"
    );
}

// --- T6: handler identity follows node identity --------------------------------

#[test]
fn a_handler_is_its_nodes_identity_and_its_event() {
    let plain = view(true, false);
    let inserted = view(true, true);
    let off = view(false, false);
    let a = must(&plain, "button", "A");
    let b = must(&off, "chip", "B");

    // One node, two events: two handlers.
    assert_ne!(a.events["tap"], a.events["hold"]);
    // The same identity and event give the same handler wherever the node is rendered.
    assert_eq!(a.events, must(&inserted, "button", "A").events);
    // Another node's `tap` is another handler.
    assert_ne!(a.events["tap"], b.events["tap"]);

    // Dispatch finds each by identity, whichever render it came from …
    let intent = |render: &Render, handler: &str| {
        dispatch(render, handler, None).map(|intent| intent.command)
    };
    assert_eq!(intent(&plain, &a.events["tap"]).unwrap(), "pick");
    assert_eq!(intent(&inserted, &a.events["tap"]).unwrap(), "pick");
    assert_eq!(intent(&plain, &a.events["hold"]).unwrap(), "hold");
    assert_eq!(intent(&off, &b.events["tap"]).unwrap(), "drop");
    // … and a handler whose node isn't in the render is not valid against it.
    let refused =
        |result: Result<String, Vec<mesh_runtime::RuntimeDiagnostic>>| codes(&result.unwrap_err());
    assert_eq!(
        refused(intent(&off, &a.events["tap"])),
        ["runtime-unknown-handler"]
    );
    assert_eq!(
        refused(intent(&plain, &b.events["tap"])),
        ["runtime-unknown-handler"]
    );
}

// --- the render-v1 boundary ----------------------------------------------------

#[test]
fn every_tree_is_valid_render_v1_unchanged() {
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../schemas/render-v1.schema.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    for render in [
        view(true, false),
        view(false, false),
        view(true, true),
        view(false, true),
        solo(true),
        solo(false),
        render("twin", TWIN, r#"{"show":true}"#),
        render("twin", TWIN, r#"{"show":false}"#),
    ] {
        let tree: serde_json::Value = serde_json::from_str(&render.tree().to_json()).unwrap();
        assert!(validator.is_valid(&tree), "{tree}");
        let all = keys(&render);
        assert_eq!(
            all.len(),
            all.iter().collect::<std::collections::BTreeSet<_>>().len(),
            "keys are unique"
        );
    }
}

// --- the provisional conditional is checked ------------------------------------

#[test]
fn a_malformed_conditional_is_refused() {
    let refuse = |source: &str| {
        let template = compile_with(MODEL, "bad", source);
        let templates = [template.as_str()];
        mesh_runtime::check_program(
            &Program {
                root: "bad",
                templates: &templates,
            },
            MODEL,
        )
    };
    for source in [
        // three alternatives
        "<page><mesh-if when={show}><note>a</note><note>b</note><note>c</note></mesh-if></page>",
        // text among the alternatives
        "<page><mesh-if when={show}><note>a</note>text</mesh-if></page>",
        // a conditional as an alternative
        "<page><mesh-if when={show}><mesh-if when={show}><note>a</note></mesh-if></mesh-if></page>",
        // no alternative at all
        "<page><mesh-if when={show}></mesh-if></page>",
        // a conditional can't be a template's root
        "<mesh-if when={show}><note>a</note></mesh-if>",
    ] {
        let diagnostics = refuse(source);
        assert_eq!(
            codes(&diagnostics),
            ["assembly-malformed-template"],
            "{source}"
        );
    }
    // A condition that isn't a boolean never gets as far as the runtime: the compiler's
    // ordinary prop check refuses it.
    let model = mesh_compiler::check::Model::load(MODEL, "bad").unwrap();
    let compiled = mesh_compiler::check::template(
        "<page><mesh-if when=\"yes\"><note>a</note></mesh-if></page>",
        &model,
    );
    assert!(compiled.template.is_none() && !compiled.diagnostics.is_empty());
}
