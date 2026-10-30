//! A composite whose whole template is `{@render children()}` (decision U13): it makes no node of its own, so
//! it forwards what its caller wrote, in place. The vectors are `examples/conformance/forwarder`; these tests pin
//! what a vector can't: a program cannot start with one, an event in forwarded content finds its handler, a
//! forwarder is a fragment to the template format, and an update is a render over random states.

mod common;

use common::{codes, compile_with, snapshot};
use mesh_runtime::{dispatch, render, update, Node, Program, Render, RuntimeDiagnostic, TreeChild};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/conformance/forwarder")
}

fn read(name: &str) -> String {
    fs::read_to_string(dir().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn program() -> &'static (String, Vec<String>) {
    static PROGRAM: std::sync::OnceLock<(String, Vec<String>)> = std::sync::OnceLock::new();
    PROGRAM.get_or_init(|| {
        let model = read("components.json");
        let listed: Value = serde_json::from_str(&read("program.json")).unwrap();
        let templates = listed["templates"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                let c = c.as_str().unwrap();
                compile_with(&model, c, &read(&format!("{c}.mprx")))
            })
            .collect();
        (model, templates)
    })
}

fn program_render(state: &Value) -> Render {
    let (model, templates) = program();
    let texts: Vec<&str> = templates.iter().map(String::as_str).collect();
    render(
        &Program {
            root: "screen",
            templates: &texts,
        },
        model,
        &snapshot(&state.to_string()),
    )
    .unwrap_or_else(|d| panic!("renders: {d:#?}"))
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn random_state(rng: &mut Rng) -> Value {
    let mut ids: Vec<u64> = (1..8).collect();
    let people: Vec<Value> = (0..rng.below(5))
        .map(|_| {
            let id = ids.remove(rng.below(ids.len()));
            json!({ "id": id, "name": format!("p{}", rng.below(4)) })
        })
        .collect();
    json!({ "title": format!("t{}", rng.below(3)), "flag": rng.below(2) == 0, "people": people })
}

#[test]
fn an_update_is_a_render_over_random_states_of_a_program_of_forwarders() {
    let mut rng = Rng(23);
    for _ in 0..40 {
        let mut current = program_render(&random_state(&mut rng));
        for _ in 0..15 {
            let state = random_state(&mut rng);
            let updated = update(&current, &snapshot(&state.to_string()))
                .unwrap_or_else(|d| panic!("updates: {d:#?}"));

            assert_eq!(
                updated.render.tree(),
                program_render(&state).tree(),
                "tree for {state}"
            );
            current = updated.render;
        }
    }
}

fn first_press(node: &Node) -> Option<String> {
    if let Some(id) = node.events.get("press") {
        return Some(id.clone());
    }
    node.children.iter().find_map(|child| match child {
        TreeChild::Node(child) => first_press(child),
        TreeChild::Text { .. } => None,
    })
}

#[test]
fn an_event_in_forwarded_content_is_the_callers_and_finds_its_item() {
    let state = json!({ "title": "t", "flag": true, "people": [{ "id": 7, "name": "Ada" }, { "id": 9, "name": "Grace" }] });
    let current = program_render(&state);
    let handler = first_press(&current.tree().root).expect("a row binds press");
    let intent = dispatch(&current, &handler, None).expect("dispatches");
    let intent: Value = serde_json::from_str(&intent.to_json()).unwrap();

    assert_eq!(
        intent["command"],
        json!({ "component": "screen", "name": "select" })
    );
    assert_eq!(intent["arguments"], json!([{ "value": 7, "name": "id" }]));
}

const MODEL: &str = r#"{
  "version": 1, "types": {},
  "components": {
    "page": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "fwd": { "props": { "children": { "type": { "kind": "snippet", "params": [] }, "required": true } }, "events": {}, "commands": {},
      "scope": { "children": { "kind": "snippet", "params": [] } } },
    "view": { "props": {}, "events": {}, "commands": {}, "scope": { "title": { "kind": "string" } } }
  }
}"#;

fn refused(root_template: &str) -> Vec<RuntimeDiagnostic> {
    let view = compile_with(MODEL, "view", root_template);
    let fwd = compile_with(MODEL, "fwd", "{@render children()}");

    render(
        &Program {
            root: "view",
            templates: &[view.as_str(), fwd.as_str()],
        },
        MODEL,
        &snapshot(r#"{"title":"t"}"#),
    )
    .expect_err("is refused")
}

#[test]
fn a_program_cannot_start_with_a_forwarder() {
    assert_eq!(
        codes(&refused("<fwd>{title}</fwd>")),
        ["assembly-root-fragment"]
    );
    let found = refused("<fwd>{title}</fwd>");
    assert!(
        found[0].message.contains("{@render"),
        "the message says what makes it one: {}",
        found[0].message
    );
}

#[test]
fn a_forwarder_inside_an_element_renders_what_its_caller_wrote() {
    let view = compile_with(
        MODEL,
        "view",
        "<page><fwd>{title}</fwd><fwd><page /></fwd></page>",
    );
    let fwd = compile_with(MODEL, "fwd", "{@render children()}");
    let rendered = render(
        &Program {
            root: "view",
            templates: &[view.as_str(), fwd.as_str()],
        },
        MODEL,
        &snapshot(r#"{"title":"t"}"#),
    )
    .expect("renders");
    let root = &rendered.tree().root;

    assert_eq!(root.component, "page");
    assert!(matches!(&root.children[0], TreeChild::Text { text, .. } if text == "t"));
    assert!(matches!(&root.children[1], TreeChild::Node(node) if node.component == "page"));
    assert_eq!(root.children.len(), 2);
}
