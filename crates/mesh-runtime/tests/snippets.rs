//! Snippets (decisions U1 to U14): a piece of template as a value over the scope it was declared in,
//! passed to a composite or named locally, rendered where its holder chooses. The program here is
//! `examples/conformance/snippets`; the vectors there pin identity, and these tests pin the law that
//! an update is a render over random snapshots, the limits, dispatch from inside a snippet, and the
//! assembly rules that keep a snippet away from a primitive and out of the host's snapshot.

mod common;

use common::{codes, compile_with, snapshot};
use mesh_runtime::{dispatch, render, update, Node, Program, Render, TreeChild};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/conformance/snippets")
}

fn read(name: &str) -> String {
    fs::read_to_string(dir().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The conformance program's model and its compiled templates, compiled once.
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

fn program_render(snapshot_json: &str) -> Render {
    let (model, templates) = program();
    let texts: Vec<&str> = templates.iter().map(String::as_str).collect();
    render(
        &Program {
            root: "screen",
            templates: &texts,
        },
        model,
        &snapshot(snapshot_json),
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

/// A random tree of `{ id, name, children }`, ids distinct among siblings, `depth` levels at most.
fn tree(rng: &mut Rng, depth: usize, id: u64) -> Value {
    let count = if depth == 0 { 0 } else { rng.below(3) };
    let mut ids: Vec<u64> = (0..6).collect();
    let children: Vec<Value> = (0..count)
        .map(|_| {
            let id = ids.remove(rng.below(ids.len()));
            tree(rng, depth - 1, id)
        })
        .collect();
    json!({ "id": id, "name": format!("n{}", rng.below(5)), "children": children })
}

fn random_snapshot(rng: &mut Rng) -> Value {
    let mut ids: Vec<u64> = (1..8).collect();
    let people: Vec<Value> = (0..rng.below(5))
        .map(|_| {
            let id = ids.remove(rng.below(ids.len()));
            json!({ "id": id, "name": format!("p{}", rng.below(4)) })
        })
        .collect();
    json!({
        "title": format!("t{}", rng.below(3)),
        "subtitle": format!("s{}", rng.below(3)),
        "people": people,
        "tree": tree(rng, 3, 0),
    })
}

#[test]
fn an_update_is_a_render_over_random_snippet_programs() {
    let mut rng = Rng(11);
    let mut checked = 0;
    for _ in 0..30 {
        let mut current = program_render(&random_snapshot(&mut rng).to_string());
        for _ in 0..15 {
            let state = random_snapshot(&mut rng);
            let updated = update(&current, &snapshot(&state.to_string()))
                .unwrap_or_else(|d| panic!("updates: {d:#?}"));
            let fresh = program_render(&state.to_string());
            assert_eq!(updated.render.tree(), fresh.tree(), "tree for {state}");
            current = updated.render;
            checked += 1;
        }
    }
    assert_eq!(checked, 450);
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
fn an_event_in_a_snippet_is_the_declaring_templates_and_finds_its_row_again() {
    let state = json!({
        "title": "t", "subtitle": "s", "tree": { "id": 0, "name": "r", "children": [] },
        "people": [{ "id": 7, "name": "Ada" }, { "id": 9, "name": "Grace" }]
    });
    let current = program_render(&state.to_string());
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
    "text": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "view": { "props": {}, "events": {}, "commands": {},
      "scope": { "count": { "kind": "number" } } }
  }
}"#;

/// `body` rendered inside a page, for `count`.
fn view(body: &str, count: u32) -> Result<Render, Vec<mesh_runtime::RuntimeDiagnostic>> {
    let template = compile_with(MODEL, "view", &format!("<page>{body}</page>"));
    render(
        &Program {
            root: "view",
            templates: &[template.as_str()],
        },
        MODEL,
        &snapshot(&format!("{{\"count\":{count}}}")),
    )
}

const COUNTDOWN: &str = "{#snippet down(n)}{#if n > 0}<text>{n}</text>{@render down(n - 1)}{/if}{/snippet}{@render down(count)}";

#[test]
fn snippets_nest_to_64_levels_and_no_further() {
    let rendered = view(COUNTDOWN, 63).expect("63 renders below the first is within 64");
    assert_eq!(
        node_count(&rendered.tree().root),
        1 + 63,
        "the page and one text per level"
    );
    let refused = view(COUNTDOWN, 64).expect_err("65 levels is past the limit");
    assert_eq!(codes(&refused), ["runtime-snippet-depth"]);
}

fn node_count(node: &Node) -> usize {
    1 + node
        .children
        .iter()
        .map(|child| match child {
            TreeChild::Node(child) => node_count(child),
            TreeChild::Text { .. } => 0,
        })
        .sum::<usize>()
}

#[test]
fn a_snippet_that_renders_itself_with_the_same_argument_fails_closed() {
    let refused = view(
        "{#snippet loop(n)}{@render loop(n)}{/snippet}{@render loop(1)}",
        0,
    )
    .expect_err("it never ends");
    assert_eq!(codes(&refused), ["runtime-snippet-depth"]);
}

#[test]
fn a_render_of_a_snippet_that_is_not_there_renders_nothing() {
    // The optional footer of the conformance card is left out in one of its uses.
    let rendered = program_render(
        r#"{"title":"t","subtitle":"s","people":[],"tree":{"id":0,"name":"r","children":[]}}"#,
    );
    assert!(node_count(&rendered.tree().root) > 1);
}

mod assembly {
    use super::*;

    /// A model in which `panel` takes a snippet, and a template of `view` that gives it one.
    fn model(panel_has_template: bool) -> (String, Vec<String>) {
        let model = r#"{
          "version": 1, "types": {},
          "components": {
            "page": { "props": {}, "events": {}, "commands": {}, "scope": {} },
            "text": { "props": {}, "events": {}, "commands": {}, "scope": {} },
            "panel": { "props": { "body": { "type": { "kind": "snippet", "params": [] }, "required": true } },
                       "events": {}, "commands": {},
                       "scope": { "body": { "kind": "snippet", "params": [] } } },
            "view": { "props": {}, "events": {}, "commands": {}, "scope": {} }
          }
        }"#;
        let mut templates = vec![compile_with(
            model,
            "view",
            "<page><panel>{#snippet body()}<text>hi</text>{/snippet}</panel></page>",
        )];
        if panel_has_template {
            templates.push(compile_with(
                model,
                "panel",
                "<page>{@render body()}</page>",
            ));
        }
        (model.to_string(), templates)
    }

    fn run(model: &str, templates: &[String], root: &str) -> Result<Render, Vec<String>> {
        let texts: Vec<&str> = templates.iter().map(String::as_str).collect();
        render(
            &Program {
                root,
                templates: &texts,
            },
            model,
            &snapshot("{}"),
        )
        .map_err(|d| codes(&d).iter().map(|c| c.to_string()).collect())
    }

    #[test]
    fn a_composite_is_given_a_snippet() {
        let (model, templates) = model(true);
        assert!(run(&model, &templates, "view").is_ok());
    }

    #[test]
    fn a_primitive_cannot_be_given_one() {
        let (model, templates) = model(false);
        assert_eq!(
            run(&model, &templates, "view").unwrap_err(),
            ["assembly-snippet-to-primitive"]
        );
    }

    #[test]
    fn the_root_scope_holds_no_snippet() {
        let model = r#"{
          "version": 1, "types": {},
          "components": {
            "page": { "props": {}, "events": {}, "commands": {}, "scope": {} },
            "view": { "props": {}, "events": {}, "commands": {},
                      "scope": { "body": { "kind": "snippet", "params": [] } } }
          }
        }"#;
        let template = compile_with(model, "view", "<page>{@render body()}</page>");
        assert_eq!(
            run(model, &[template], "view").unwrap_err(),
            ["assembly-snippet-root-scope"]
        );
    }
}
