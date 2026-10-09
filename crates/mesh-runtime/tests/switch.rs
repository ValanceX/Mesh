//! `mesh-switch` (docs/manual/templates.md, "Switch"): the compiler writes it as the conditionals it stands for, so the runtime has nothing new to do and these tests
//! pin what a program written with a switch means: the first case whose `when` holds is rendered, else the default, else nothing; each case is its own site;
//! and `update` between cases gives the tree a fresh render gives.

mod common;

use common::{compile_with, snapshot};
use mesh_runtime::{render, update, Node, Program, Render, TreeChild};

const MODEL: &str = r#"{
  "version": 1, "types": {},
  "components": {
    "panel": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "note":  { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "mesh-if": { "props": { "when": { "type": { "kind": "boolean" }, "required": true } }, "events": {}, "commands": {}, "scope": {} },
    "mesh-switch":  { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "mesh-default": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "mesh-case": { "props": { "when": { "type": { "kind": "boolean" }, "required": true } }, "events": {}, "commands": {}, "scope": {} },
    "view": { "props": {}, "events": {}, "commands": {},
      "scope": { "kind": { "kind": "string" }, "count": { "kind": "number" } } }
  }
}"#;

const WITH_DEFAULT: &str = r#"<panel>
  <note>before</note>
  <mesh-switch>
    <mesh-case when={kind == "a"}><note>A</note></mesh-case>
    <mesh-case when={count > 1}><note>many</note></mesh-case>
    <mesh-case when={count > 0}><note>one</note></mesh-case>
    <mesh-default><note>none</note></mesh-default>
  </mesh-switch>
  <note>after</note>
</panel>"#;

const WITHOUT_DEFAULT: &str = r#"<panel><mesh-switch><mesh-case when={kind == "a"}><note>A</note></mesh-case></mesh-switch></panel>"#;

fn drawn(source: &str, state: &str) -> Render {
    let template = compile_with(MODEL, "view", source);
    render(
        &Program {
            root: "view",
            templates: &[template.as_str()],
        },
        MODEL,
        &snapshot(state),
    )
    .expect("renders")
}

fn words(node: &Node, out: &mut Vec<String>) {
    for child in &node.children {
        match child {
            TreeChild::Text { text, .. } => out.push(text.clone()),
            TreeChild::Node(node) => words(node, out),
        }
    }
}

fn shown(render: &Render) -> Vec<String> {
    let mut out = Vec::new();
    words(&render.tree().root, &mut out);
    out
}

#[test]
fn the_first_case_that_holds_is_rendered_else_the_default_else_nothing() {
    let state = |kind: &str, count: u32| format!(r#"{{"kind":"{kind}","count":{count}}}"#);

    assert_eq!(
        shown(&drawn(WITH_DEFAULT, &state("a", 5))),
        ["before", "A", "after"],
        "the first case wins over a later one that also holds"
    );
    assert_eq!(
        shown(&drawn(WITH_DEFAULT, &state("b", 5))),
        ["before", "many", "after"]
    );
    assert_eq!(
        shown(&drawn(WITH_DEFAULT, &state("b", 1))),
        ["before", "one", "after"]
    );
    assert_eq!(
        shown(&drawn(WITH_DEFAULT, &state("b", 0))),
        ["before", "none", "after"],
        "the default"
    );
    assert_eq!(
        shown(&drawn(WITHOUT_DEFAULT, &state("b", 0))),
        Vec::<String>::new(),
        "nothing, without a default"
    );
    assert_eq!(shown(&drawn(WITHOUT_DEFAULT, &state("a", 0))), ["A"]);
}

#[test]
fn an_update_between_cases_is_the_tree_a_fresh_render_gives() {
    let states = [
        ("a", 5),
        ("b", 5),
        ("b", 1),
        ("b", 0),
        ("a", 0),
        ("b", 2),
        ("b", 2),
    ];
    let state = |(kind, count): (&str, u32)| format!(r#"{{"kind":"{kind}","count":{count}}}"#);
    let mut current = drawn(WITH_DEFAULT, &state(states[0]));

    for next in &states[1..] {
        let updated = update(&current, &snapshot(&state(*next))).expect("updates");
        let full = drawn(WITH_DEFAULT, &state(*next));

        assert_eq!(updated.render.tree(), full.tree(), "{next:?}");
        current = updated.render;
    }
}

#[test]
fn each_case_is_its_own_site_so_two_cases_never_share_a_key() {
    // The same note in two cases: switching between them is a different part of the tree each time, never a kept one.
    let source = r#"<panel><mesh-switch><mesh-case when={kind == "a"}><note>same</note></mesh-case><mesh-default><note>same</note></mesh-default></mesh-switch></panel>"#;
    let a = drawn(source, r#"{"kind":"a","count":0}"#);
    let b = drawn(source, r#"{"kind":"b","count":0}"#);
    let key = |render: &Render| match &render.tree().root.children[0] {
        TreeChild::Node(node) => node.key.clone(),
        TreeChild::Text { .. } => unreachable!(),
    };

    assert_ne!(key(&a), key(&b));
}
