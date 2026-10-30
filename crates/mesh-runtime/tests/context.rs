//! The context channel (docs/manual/runtime.md, "Context"): values a host gives beside the snapshot, read only by
//! the components whose manifest entry `uses` them. The conformance vectors (`examples/conformance/context/`) pin
//! the trees; these pin the rest: what an update keeps, the paths a change may take, and that the older entry
//! points are the same operations with no context.

mod common;

use common::{codes, compile_with, snapshot};
use mesh_runtime::{
    render, render_context, update_changes, update_with_context, Changes, HostValue, Program,
    Render, TreeChild, Units,
};
use std::rc::Rc;

const MODEL: &str = r#"{
  "version": 1, "types": {},
  "context": { "theme": { "kind": "string" } },
  "components": {
    "box": { "props": { "tone": { "type": { "kind": "string" }, "required": false } }, "events": {}, "commands": {}, "scope": {} },
    "text": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "badge": { "props": {}, "events": {}, "commands": {}, "scope": {}, "uses": ["theme"] },
    "page": { "props": {}, "events": {}, "commands": {}, "scope": { "title": { "kind": "string" } } }
  }
}"#;

const PAGE: &str = "<box><text>{title}</text><badge /></box>";
const BADGE: &str = "<box tone={theme} />";

fn templates() -> Vec<String> {
    vec![
        compile_with(MODEL, "page", PAGE),
        compile_with(MODEL, "badge", BADGE),
    ]
}

fn with(title: &str, theme: Option<&str>) -> Result<Render, Vec<mesh_runtime::RuntimeDiagnostic>> {
    let templates = templates();
    let texts: Vec<&str> = templates.iter().map(String::as_str).collect();
    let context = match theme {
        Some(theme) => snapshot(&format!(r#"{{ "theme": "{theme}" }}"#)),
        None => snapshot("{}"),
    };
    render_context(
        &Program {
            root: "page",
            templates: &texts,
        },
        MODEL,
        &snapshot(&format!(r#"{{ "title": "{title}" }}"#)),
        &Units::default(),
        &context,
    )
}

fn changes(render: &Render, edits: &str) -> Changes {
    let text = format!(r#"{{ "base": {}, "changes": {edits} }}"#, render.version());
    Changes::from_host(&HostValue::from_json(&text).unwrap()).unwrap()
}

/// The root's children: the text, then the badge's node.
fn children(render: &Render) -> Vec<Rc<mesh_runtime::Node>> {
    render
        .tree()
        .root
        .children
        .iter()
        .filter_map(|child| match child {
            TreeChild::Node(node) => Some(Rc::clone(node)),
            TreeChild::Text { .. } => None,
        })
        .collect()
}

#[test]
fn an_update_that_changes_no_context_value_keeps_what_does_not_read_the_snapshot() {
    let first = with("a", Some("light")).unwrap();
    let second = update_changes(
        &first,
        &changes(
            &first,
            r#"[{ "op": "set", "path": ["title"], "value": "b" }]"#,
        ),
        None,
    )
    .unwrap()
    .render;
    // The badge reads only the context, which is as it was: its node is the very same.
    assert!(Rc::ptr_eq(&children(&first)[1], &children(&second)[1]));
    // A change to the context renders it again, with the new value.
    let third = update_changes(
        &second,
        &changes(
            &second,
            r#"[{ "op": "set", "path": [{ "context": "theme" }], "value": "dark" }]"#,
        ),
        None,
    )
    .unwrap();
    assert_eq!(children(&third.render)[1].props["tone"], "dark");
    assert_eq!(third.render.context_to_json(), r#"{"theme":"dark"}"#);
    assert_eq!(third.render.tree(), with("b", Some("dark")).unwrap().tree());
    // And so does an update given the whole context; one given none keeps the render's.
    let fourth = update_with_context(
        &third.render,
        snapshot(r#"{ "title": "b" }"#),
        None,
        Some(&snapshot(r#"{ "theme": "light" }"#)),
    )
    .unwrap();
    assert_eq!(children(&fourth.render)[1].props["tone"], "light");
    let kept =
        update_with_context(&fourth.render, snapshot(r#"{ "title": "c" }"#), None, None).unwrap();
    assert_eq!(kept.render.context_to_json(), r#"{"theme":"light"}"#);
}

#[test]
fn a_change_reaches_the_context_only_from_its_root_and_only_a_declared_name() {
    let first = with("a", Some("light")).unwrap();
    for edits in [
        // Not a name of the context.
        r#"[{ "op": "set", "path": [{ "context": "locale" }], "value": "en" }]"#,
        // A context name is not a scope name of the snapshot, and the root doesn't use it here.
        r#"[{ "op": "set", "path": ["theme"], "value": "dark" }]"#,
        // A value that doesn't fit its type.
        r#"[{ "op": "set", "path": [{ "context": "theme" }], "value": 1 }]"#,
        // A required value can't be removed.
        r#"[{ "op": "remove", "path": [{ "context": "theme" }] }]"#,
    ] {
        let refused = update_changes(&first, &changes(&first, edits), None).unwrap_err();
        assert!(!refused.is_empty(), "{edits}");
    }
    // `{ context }` is a root only: anywhere else it is not a path segment.
    let text = format!(
        r#"{{ "base": {}, "changes": [{{ "op": "set", "path": ["title", {{ "context": "theme" }}], "value": 1 }}] }}"#,
        first.version()
    );
    let refused = Changes::from_host(&HostValue::from_json(&text).unwrap()).unwrap_err();
    assert_eq!(codes(&refused), ["runtime-invalid-change"]);
}

#[test]
fn render_without_context_is_render_with_none() {
    let templates = templates();
    let texts: Vec<&str> = templates.iter().map(String::as_str).collect();
    let program = Program {
        root: "page",
        templates: &texts,
    };
    // `theme` is required, so a render with no context values is refused at its path.
    let refused = render(&program, MODEL, &snapshot(r#"{ "title": "a" }"#)).unwrap_err();
    assert_eq!(codes(&refused), ["runtime-missing-value"]);
    assert_eq!(
        codes(&with("a", None).unwrap_err()),
        ["runtime-missing-value"]
    );
    // The program itself is valid: the badge's `theme` needs no prop to bind it.
    assert!(mesh_runtime::check_program(&program, MODEL).is_empty());
}
