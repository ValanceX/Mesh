//! `mesh-fragment` (docs/manual/templates.md, "Fragments"): content placed without a node of its own. MESH never wraps content in a node: a composite's template, a
//! conditional's alternative or a repeat's item is a node only if the author wrote an element, and text or several nodes only if the author wrote a fragment.

mod common;

use common::{codes, compile_with, snapshot};
use mesh_runtime::{
    dispatch, render, update, HostValue, Node, Program, Render, RuntimeDiagnostic, TreeChild,
};

const MODEL: &str = r#"{
  "version": 1, "types": {},
  "components": {
    "panel": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "note":  { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "button": { "props": {}, "events": { "tap": {} }, "commands": {}, "scope": {} },
    "mesh-fragment": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "mesh-slot": { "props": { "name": { "type": { "kind": "string" }, "required": false } }, "events": {}, "commands": {}, "scope": {} },
    "mesh-if": { "props": { "when": { "type": { "kind": "boolean" }, "required": true } }, "events": {}, "commands": {}, "scope": {} },
    "mesh-each": { "props": { "items": { "type": { "kind": "list", "element": { "kind": "any" } }, "required": true }, "as": { "type": { "kind": "string" }, "required": true }, "key": { "type": { "kind": "any" }, "required": true } }, "events": {}, "commands": {}, "scope": {} },
    "shout": { "props": { "who": { "type": { "kind": "string" }, "required": true } }, "events": {}, "commands": {}, "scope": { "who": { "kind": "string" } } },
    "pair": { "props": { "label": { "type": { "kind": "string" }, "required": true } }, "events": {}, "commands": {}, "scope": { "label": { "kind": "string" } } },
    "wrap": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "row": { "props": { "id": { "type": { "kind": "string" }, "required": true } }, "events": {}, "commands": { "pick": { "parameters": [ { "name": "id", "type": { "kind": "string" } } ] } }, "scope": { "id": { "kind": "string" } } },
    "view": { "props": {}, "events": {},
      "commands": { "pick": { "parameters": [ { "name": "id", "type": { "kind": "string" } } ] } },
      "scope": { "who": { "kind": "string" }, "flag": { "kind": "boolean" },
        "items": { "kind": "list", "element": { "kind": "record", "fields": { "id": { "type": { "kind": "string" }, "required": true }, "label": { "type": { "kind": "string" }, "required": true } } } } } }
  }
}"#;

const SHOUT: &str = "<mesh-fragment>{who}!</mesh-fragment>";
const PAIR: &str = "<mesh-fragment><note>{label}</note><note>·</note></mesh-fragment>";
const WRAP: &str = "<mesh-fragment><note>[</note><mesh-slot /><note>]</note></mesh-fragment>";
const ROW: &str =
    "<mesh-fragment><note>{id}</note><button on.tap={pick(id)}>go</button></mesh-fragment>";

fn try_program(
    view: &str,
    others: &[(&str, &str)],
    state: &str,
) -> Result<Render, Vec<RuntimeDiagnostic>> {
    let mut templates = vec![compile_with(MODEL, "view", view)];
    for (component, source) in others {
        templates.push(compile_with(MODEL, component, source));
    }
    let texts: Vec<&str> = templates.iter().map(String::as_str).collect();

    render(
        &Program {
            root: "view",
            templates: &texts,
        },
        MODEL,
        &snapshot(state),
    )
}

fn program(view: &str, others: &[(&str, &str)], state: &str) -> Render {
    try_program(view, others, state).unwrap_or_else(|d| panic!("renders: {d:#?}"))
}

fn all(others: &[(&'static str, &'static str)]) -> Vec<(&'static str, &'static str)> {
    others.to_vec()
}

const STATE: &str =
    r#"{"who":"Ada","flag":true,"items":[{"id":"a","label":"A"},{"id":"b","label":"B"}]}"#;

/// The children of the root, as a word each: a node's component, or the text.
fn shape(render: &Render) -> Vec<String> {
    render
        .tree()
        .root
        .children
        .iter()
        .map(|child| match child {
            TreeChild::Node(node) => node.component.clone(),
            TreeChild::Text { text, .. } => format!("\"{text}\""),
        })
        .collect()
}

fn texts(node: &Node, out: &mut Vec<String>) {
    for child in &node.children {
        match child {
            TreeChild::Text { text, .. } => out.push(text.clone()),
            TreeChild::Node(node) => texts(node, out),
        }
    }
}

#[test]
fn a_composite_that_starts_with_a_fragment_makes_no_node_and_its_text_joins_the_text_beside_it() {
    let rendered = program(
        "<panel>hello <shout who={who} /> bye</panel>",
        &[("shout", SHOUT)],
        STATE,
    );

    assert_eq!(
        shape(&rendered),
        ["\"hello Ada! bye\""],
        "one run, and no node for the composite"
    );
}

#[test]
fn a_fragment_may_be_several_nodes() {
    let rendered = program(
        "<panel><pair label={who} /><note>after</note></panel>",
        &[("pair", PAIR)],
        STATE,
    );

    assert_eq!(shape(&rendered), ["note", "note", "note"]);
}

#[test]
fn a_conditional_alternative_may_be_a_fragment() {
    let view = "<panel><mesh-if when={flag}><mesh-fragment><note>x</note><note>y</note></mesh-fragment><mesh-fragment>off</mesh-fragment></mesh-if></panel>";

    assert_eq!(shape(&program(view, &[], STATE)), ["note", "note"]);
    assert_eq!(
        shape(&program(view, &[], &STATE.replace("true", "false"))),
        ["\"off\""]
    );
}

#[test]
fn a_fragment_that_is_written_among_children_places_them_in_place() {
    let rendered = program(
        "<panel><note>a</note><mesh-fragment>b<note>c</note></mesh-fragment>d</panel>",
        &[],
        STATE,
    );

    assert_eq!(shape(&rendered), ["note", "\"b\"", "note", "\"d\""]);
}

#[test]
fn a_repeat_item_may_be_a_fragment_and_each_item_keeps_its_own_keys() {
    let view = "<panel><mesh-each items={items} as=\"item\" key={item.id}><mesh-fragment><note>{item.label}</note><note>·</note></mesh-fragment></mesh-each></panel>";
    let rendered = program(view, &[], STATE);
    let mut keys = Vec::new();

    fn collect(node: &Node, keys: &mut Vec<String>) {
        keys.push(node.key.clone());
        for child in &node.children {
            match child {
                TreeChild::Node(node) => collect(node, keys),
                TreeChild::Text { key, .. } => keys.push(key.clone()),
            }
        }
    }
    collect(&rendered.tree().root, &mut keys);

    assert_eq!(shape(&rendered), ["note", "note", "note", "note"]);
    assert_eq!(
        keys.iter().collect::<std::collections::BTreeSet<_>>().len(),
        keys.len(),
        "no two parts share a key"
    );
}

#[test]
fn a_fragment_composite_places_a_slot_in_the_middle() {
    let rendered = program(
        "<panel><wrap><note>in</note></wrap></panel>",
        &[("wrap", WRAP)],
        STATE,
    );

    assert_eq!(shape(&rendered), ["note", "note", "note"]);
    let mut all = Vec::new();
    texts(&rendered.tree().root, &mut all);
    assert_eq!(all, ["[", "in", "]"]);
}

#[test]
fn an_event_inside_a_fragment_dispatches_with_the_scope_it_had_in_the_repeat() {
    let view = "<panel><mesh-each items={items} as=\"item\" key={item.id}><row id={item.id} /></mesh-each></panel>";
    let rendered = program(view, &[("row", ROW)], STATE);
    let handlers: Vec<String> = rendered
        .tree()
        .root
        .children
        .iter()
        .filter_map(|child| match child {
            TreeChild::Node(node) => node.events.get("tap").cloned(),
            TreeChild::Text { .. } => None,
        })
        .collect();

    assert_eq!(handlers.len(), 2);
    let intent = dispatch(&rendered, &handlers[1], None::<&HostValue>).expect("dispatches");

    assert_eq!(intent.command, "pick");
    assert_eq!(intent.arguments, [Some(serde_json::json!("b"))]);
}

#[test]
fn an_update_agrees_with_a_fresh_render_over_changes_of_every_kind() {
    let view = "<panel><shout who={who} /><mesh-if when={flag}><mesh-fragment><note>on</note><pair label={who} /></mesh-fragment></mesh-if><mesh-each items={items} as=\"item\" key={item.id}><row id={item.id} /></mesh-each><wrap><note>{who}</note></wrap></panel>";
    let others = all(&[
        ("shout", SHOUT),
        ("pair", PAIR),
        ("row", ROW),
        ("wrap", WRAP),
    ]);
    let item = |id: &str| format!(r#"{{"id":"{id}","label":"{}"}}"#, id.to_uppercase());
    let state = |who: &str, flag: bool, ids: &[&str]| {
        format!(
            r#"{{"who":"{who}","flag":{flag},"items":[{}]}}"#,
            ids.iter().map(|id| item(id)).collect::<Vec<_>>().join(",")
        )
    };
    let states = [
        state("Ada", true, &["a", "b"]),
        state("Ada", true, &["b", "a"]),
        state("Bob", true, &["b", "a", "c"]),
        state("Bob", false, &["c"]),
        state("Bob", false, &[]),
        state("Cy", true, &["a", "b", "c", "d"]),
        state("Cy", true, &["a", "b", "c", "d"]),
        state("Cy", true, &["d", "c"]),
    ];
    let mut current = program(view, &others, &states[0]);

    for next in &states[1..] {
        let updated = update(&current, &snapshot(next)).expect("updates");
        let full = program(view, &others, next);

        assert_eq!(updated.render.tree(), full.tree(), "{next}");
        current = updated.render;
    }
}

#[test]
fn the_root_renders_one_node_so_it_cannot_start_with_a_fragment() {
    let refused = |view: &str, others: &[(&str, &str)]| {
        codes(&try_program(view, others, STATE).expect_err("is refused"))
    };

    assert_eq!(
        refused("<mesh-fragment><note>x</note></mesh-fragment>", &[]),
        ["assembly-root-fragment"]
    );
    assert_eq!(
        refused("<pair label={who} />", &[("pair", PAIR)]),
        ["assembly-root-fragment"]
    );
}

#[test]
fn a_fragment_has_no_props_and_no_events() {
    // The manifest declares neither, so the compiler already refuses them; a template made by hand that has a prop is malformed for the runtime too.
    let template = compile_with(
        MODEL,
        "view",
        "<panel><mesh-fragment><note>x</note></mesh-fragment></panel>",
    );
    let span = r#"{"start":{"byte":0,"utf16":0},"end":{"byte":0,"utf16":0}}"#;
    let prop = format!(
        r#"{{"prop":"x","value":{{"kind":"literal","value":"y","span":{span}}},"span":{span}}}"#
    );
    let injected = template.replacen(
        r#""component":"mesh-fragment","props":[]"#,
        &format!(r#""component":"mesh-fragment","props":[{prop}]"#),
        1,
    );

    assert_ne!(injected, template, "the fragment's props were found");
    let result = render(
        &Program {
            root: "view",
            templates: &[injected.as_str()],
        },
        MODEL,
        &snapshot(STATE),
    );

    assert_eq!(
        codes(&result.expect_err("is refused")),
        ["assembly-malformed-template"]
    );
}

#[test]
fn text_runs_are_maximal_whatever_made_them_adjacent() {
    // A conditional that chooses nothing leaves the text on either side of it adjacent; a render tree's runs are maximal, so they are one.
    let view = "<panel>a<mesh-if when={flag}><note>x</note></mesh-if>b</panel>";
    let off = program(view, &[], &STATE.replace("true", "false"));
    let on = program(view, &[], STATE);

    assert_eq!(shape(&off), ["\"ab\""]);
    assert_eq!(shape(&on), ["\"a\"", "note", "\"b\""]);

    // And an update between the two agrees with a fresh render.
    let updated = update(&on, &snapshot(&STATE.replace("true", "false"))).expect("updates");

    assert_eq!(updated.render.tree(), off.tree());
}
