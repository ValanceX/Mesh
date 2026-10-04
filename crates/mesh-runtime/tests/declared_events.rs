//! The declared events of a validated program (`mesh_runtime::declared_events`).
//!
//! The declarations are a fact about the program, not a render: they come from the same
//! `program::validate` as `check_program`, walked with the validation's own element traversal.
//! The template-v1 documents are used here only as a test oracle.

mod common;

use common::{codes, compile_with, snapshot};
use mesh_runtime::{
    check_program, declared_events, declared_events_to_json, render, DeclaredEvent, Node, Program,
    TreeChild,
};
use mesh_template::{Child, Element};

const MODEL: &str = r#"{
  "version": 1, "types": {},
  "components": {
    "page":    { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "note":    { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "row":     { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "button":  { "props": {}, "events": { "tap": {} }, "commands": {}, "scope": {} },
    "mesh-if": { "props": { "when": { "type": { "kind": "boolean" }, "required": true } }, "events": {}, "commands": {}, "scope": {} },
    "mesh-each": { "props": {
        "items": { "type": { "kind": "list", "element": { "kind": "any" } }, "required": true },
        "as":    { "type": { "kind": "string" }, "required": true },
        "key":   { "type": { "kind": "any" }, "required": true } },
      "events": {}, "commands": {}, "scope": {} },
    "card":    { "props": {}, "events": {}, "commands": { "pick": { "parameters": [] } }, "scope": {} },
    "simple":  { "props": {}, "events": {}, "commands": { "go": { "parameters": [] } }, "scope": {} },
    "twice":   { "props": {}, "events": {}, "commands": { "go": { "parameters": [] } }, "scope": {} },
    "users":   { "props": {}, "events": {}, "commands": { "refresh": { "parameters": [] } }, "scope": {} },
    "other":   { "props": {}, "events": {}, "commands": { "reset": { "parameters": [] } }, "scope": {} },
    "cond":    { "props": {}, "events": {}, "commands": { "go": { "parameters": [] } }, "scope": { "show": { "kind": "boolean" } } },
    "list":    { "props": {}, "events": {},
      "commands": { "open": { "parameters": [ { "name": "id", "type": { "kind": "any" } } ] } },
      "scope": { "items": { "kind": "list", "element": { "kind": "record", "fields": { "id": { "type": { "kind": "any" }, "required": true } } } } } }
  }
}"#;

const SIMPLE: &str = "<page><button on.tap={go()}>go</button></page>";
const TWICE: &str = "<page><button on.tap={go()}>a</button><button on.tap={go()}>b</button></page>";
const CARD: &str = "<button on.tap={pick()}>pick</button>";
const USERS: &str = "<page><card /><card /><button on.tap={refresh()}>refresh</button></page>";
const OTHER: &str = "<page><card /><button on.tap={reset()}>reset</button></page>";
const COND: &str = "<page><note>x</note><mesh-if when={show}><button on.tap={go()}>shown</button></mesh-if></page>";
const LIST: &str = "<page><mesh-each items={items} as=\"item\" key={item.id}><row><button on.tap={open(item.id)}>o</button></row></mesh-each></page>";

struct Case {
    root: &'static str,
    sources: Vec<(&'static str, &'static str)>,
}

impl Case {
    fn new(root: &'static str, sources: &[(&'static str, &'static str)]) -> Case {
        Case {
            root,
            sources: sources.to_vec(),
        }
    }
    fn templates(&self) -> Vec<String> {
        self.sources
            .iter()
            .map(|(component, source)| compile_with(MODEL, component, source))
            .collect()
    }
    fn events(&self) -> Result<Vec<DeclaredEvent>, Vec<mesh_runtime::RuntimeDiagnostic>> {
        let templates = self.templates();
        let texts: Vec<&str> = templates.iter().map(String::as_str).collect();
        declared_events(
            &Program {
                root: self.root,
                templates: &texts,
            },
            MODEL,
        )
    }
}

/// (component, event, command, span as `[start byte, end byte, start utf16, end utf16]`), sorted.
type Row = (String, String, String, [usize; 4]);

fn rows(events: &[DeclaredEvent]) -> Vec<Row> {
    let mut rows: Vec<Row> = events
        .iter()
        .map(|e| {
            (
                e.component.clone(),
                e.event.clone(),
                e.command.clone(),
                [
                    e.span.start.byte,
                    e.span.end.byte,
                    e.span.start.utf16,
                    e.span.end.utf16,
                ],
            )
        })
        .collect();
    rows.sort();
    rows
}

/// The oracle: the template-v1 documents themselves, read by the template crate and walked here, independently of the runtime.
fn oracle(templates: &[String]) -> Vec<Row> {
    fn walk(component: &str, element: &Element, out: &mut Vec<Row>) {
        for b in &element.events {
            out.push((
                component.to_string(),
                b.event.clone(),
                b.command.clone(),
                [
                    b.span.start.byte,
                    b.span.end.byte,
                    b.span.start.utf16,
                    b.span.end.utf16,
                ],
            ));
        }
        for child in &element.children {
            if let Child::Element { element } = child {
                walk(component, element, out);
            }
        }
    }
    let mut out = Vec::new();
    for text in templates {
        let template = mesh_template::from_json(text).expect("a template");
        walk(&template.component, &template.root, &mut out);
    }
    out.sort();
    out
}

fn keys(events: &[DeclaredEvent]) -> Vec<String> {
    let mut keys: Vec<String> = events
        .iter()
        .map(|e| format!("{}/{}", e.component, e.command))
        .collect();
    keys.sort();
    keys
}

fn handlers(node: &Node) -> usize {
    node.events.len()
        + node
            .children
            .iter()
            .map(|c| match c {
                TreeChild::Node(n) => handlers(n),
                TreeChild::Text { .. } => 0,
            })
            .sum::<usize>()
}

#[test]
fn a_simple_event() {
    let case = Case::new("simple", &[("simple", SIMPLE)]);
    let events = case.events().expect("valid");
    assert_eq!(events.len(), 1);
    assert_eq!(
        (
            events[0].component.as_str(),
            events[0].event.as_str(),
            events[0].command.as_str()
        ),
        ("simple", "tap", "go")
    );
    assert_eq!(rows(&events), oracle(&case.templates()));
}

#[test]
fn a_composite_templates_event_is_included_once_however_often_it_is_used() {
    let case = Case::new("users", &[("users", USERS), ("card", CARD)]);
    let events = case.events().expect("valid");
    assert_eq!(keys(&events), vec!["card/pick", "users/refresh"]); // `card` occurs twice in `users`, its template once
    assert_eq!(rows(&events), oracle(&case.templates()));
}

#[test]
fn an_event_in_an_inactive_mesh_if_alternative_is_declared() {
    let case = Case::new("cond", &[("cond", COND)]);
    let events = case.events().expect("valid");
    assert_eq!(keys(&events), vec!["cond/go"]);
    assert_eq!(rows(&events), oracle(&case.templates()));

    // The render for `show = false` has no handler at all: the declaration is the program's, not the render's.
    let templates = case.templates();
    let texts: Vec<&str> = templates.iter().map(String::as_str).collect();
    let program = Program {
        root: "cond",
        templates: &texts,
    };
    let hidden = render(&program, MODEL, &snapshot(r#"{ "show": false }"#)).expect("renders");
    let shown = render(&program, MODEL, &snapshot(r#"{ "show": true }"#)).expect("renders");
    assert_eq!(handlers(&hidden.tree().root), 0);
    assert_eq!(handlers(&shown.tree().root), 1);
}

#[test]
fn an_event_in_a_mesh_each_body_is_declared_once_not_per_item() {
    let case = Case::new("list", &[("list", LIST)]);
    let events = case.events().expect("valid");
    assert_eq!(keys(&events), vec!["list/open"]);
    assert_eq!(rows(&events), oracle(&case.templates()));

    // Three items render three handlers (per-item runtime identity); the program declares the event once.
    let templates = case.templates();
    let texts: Vec<&str> = templates.iter().map(String::as_str).collect();
    let program = Program {
        root: "list",
        templates: &texts,
    };
    let three = render(
        &program,
        MODEL,
        &snapshot(r#"{ "items": [ { "id": "a" }, { "id": "b" }, { "id": "c" } ] }"#),
    )
    .expect("renders");
    assert_eq!(handlers(&three.tree().root), 3);
}

#[test]
fn several_programs_keep_their_own_occurrences_and_nothing_is_deduplicated() {
    // Two "views" over the same composite: each program reports its own, `card/pick` appearing in both.
    let users = Case::new("users", &[("users", USERS), ("card", CARD)])
        .events()
        .expect("valid");
    let other = Case::new("other", &[("other", OTHER), ("card", CARD)])
        .events()
        .expect("valid");
    assert_eq!(keys(&users), vec!["card/pick", "users/refresh"]);
    assert_eq!(keys(&other), vec!["card/pick", "other/reset"]);

    // One program raising the same command from two bindings: two occurrences, distinct spans.
    let twice = Case::new("twice", &[("twice", TWICE)])
        .events()
        .expect("valid");
    assert_eq!(twice.len(), 2);
    assert_eq!(keys(&twice), vec!["twice/go", "twice/go"]);
    assert_ne!(twice[0].span, twice[1].span);
}

#[test]
fn spans_are_the_templates_own() {
    let case = Case::new("simple", &[("simple", SIMPLE)]);
    let events = case.events().expect("valid");
    let span = events[0].span;
    assert_eq!(&SIMPLE[span.start.byte..span.end.byte], "on.tap={go()}");
    assert_eq!(rows(&events), oracle(&case.templates()));

    // Every fixture's inventory equals the oracle, spans included.
    for case in [
        Case::new("users", &[("users", USERS), ("card", CARD)]),
        Case::new("cond", &[("cond", COND)]),
        Case::new("list", &[("list", LIST)]),
        Case::new("twice", &[("twice", TWICE)]),
    ] {
        assert_eq!(
            rows(&case.events().expect("valid")),
            oracle(&case.templates())
        );
    }
}

#[test]
fn an_invalid_program_gives_exactly_check_programs_diagnostics() {
    let simple = compile_with(MODEL, "simple", SIMPLE);
    let card = compile_with(MODEL, "card", CARD);
    let users = compile_with(MODEL, "users", USERS);

    // (root, templates, model): a missing root template, a duplicate template, another manifest, unreadable template text, and a bad manifest.
    let other_model = common::MODEL;
    let invalid: Vec<(&str, Vec<String>, &str)> = vec![
        ("users", vec![card.clone()], MODEL),
        ("simple", vec![simple.clone(), simple.clone()], MODEL),
        ("simple", vec![simple.clone()], other_model),
        ("simple", vec!["not a template".to_string()], MODEL),
        ("simple", vec![simple.clone()], "not a manifest"),
    ];
    for (root, templates, model) in invalid {
        let texts: Vec<&str> = templates.iter().map(String::as_str).collect();
        let program = Program {
            root,
            templates: &texts,
        };
        let checked = check_program(&program, model);
        assert!(
            !checked.is_empty(),
            "a program that must be invalid: {root}"
        );
        assert_eq!(
            declared_events(&program, model),
            Err(checked.clone()),
            "{root}: {:?}",
            codes(&checked)
        );
    }

    // A valid program: check_program has nothing to say and the inventory is there.
    let texts = [users.as_str(), card.as_str()];
    let program = Program {
        root: "users",
        templates: &texts,
    };
    assert!(check_program(&program, MODEL).is_empty());
    assert!(declared_events(&program, MODEL).is_ok());
}

#[test]
fn a_component_with_no_template_is_a_primitive_and_declares_nothing() {
    // `card` is in the manifest with a command, but no template is supplied: MESH treats it as a primitive (the program decides), so its command is not declared.
    let case = Case::new("users", &[("users", USERS)]);
    assert_eq!(keys(&case.events().expect("valid")), vec!["users/refresh"]);
}

#[test]
fn the_order_is_by_component_then_document_order_whatever_order_the_templates_arrive_in() {
    let forward = Case::new("users", &[("users", USERS), ("card", CARD)]);
    let backward = Case::new("users", &[("card", CARD), ("users", USERS)]);
    let (a, b) = (
        forward.events().expect("valid"),
        backward.events().expect("valid"),
    );
    assert_eq!(a, b);
    let components: Vec<&str> = a.iter().map(|e| e.component.as_str()).collect();
    assert_eq!(components, ["card", "users"]);

    // Within a template, document order.
    let twice = Case::new("twice", &[("twice", TWICE)])
        .events()
        .expect("valid");
    assert!(twice[0].span.start.byte < twice[1].span.start.byte);
}

#[test]
fn the_json_form_is_the_semantic_result_directly() {
    let case = Case::new("users", &[("users", USERS), ("card", CARD)]);
    let events = case.events().expect("valid");
    let json: serde_json::Value =
        serde_json::from_str(&declared_events_to_json(&events)).expect("JSON");
    let expected: Vec<serde_json::Value> = events
        .iter()
        .map(|e| {
            serde_json::json!({
                "component": e.component,
                "event": e.event,
                "command": e.command,
                "span": {
                    "start": { "byte": e.span.start.byte, "utf16": e.span.start.utf16 },
                    "end": { "byte": e.span.end.byte, "utf16": e.span.end.utf16 },
                },
            })
        })
        .collect();
    assert_eq!(json, serde_json::Value::Array(expected));
    assert_eq!(declared_events_to_json(&[]), "[]");
}
