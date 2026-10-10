//! Named slots (docs/manual/templates.md, "Named slots"): a composite's template places an occurrence's content in several places with
//! `<mesh-slot name="…" />`, and the occurrence says which content goes where with `<mesh-fill slot="…">`. What is not in a fill goes to the default slot.
//! Like the default slot's, the content is the caller's: its scope, its commands, and paths of its own.

mod common;

use common::{codes, compile_with, snapshot};
use mesh_runtime::{
    dispatch, render, update, HostValue, Node, Program, Render, RuntimeDiagnostic, TreeChild,
};

const MODEL: &str = r#"{
  "version": 1, "types": {},
  "components": {
    "page":  { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "note":  { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "button": { "props": {}, "events": { "tap": {} }, "commands": {}, "scope": {} },
    "panel": { "props": { "heading": { "type": { "kind": "string" }, "required": true } }, "events": {}, "commands": {}, "scope": {} },
    "mesh-slot": { "props": { "name": { "type": { "kind": "string" }, "required": false } }, "events": {}, "commands": {}, "scope": {} },
    "mesh-fill": { "props": { "slot": { "type": { "kind": "string" }, "required": true } }, "events": {}, "commands": {}, "scope": {} },
    "mesh-if": { "props": { "when": { "type": { "kind": "boolean" }, "required": true } }, "events": {}, "commands": {}, "scope": {} },
    "card": { "props": { "heading": { "type": { "kind": "string" }, "required": true } }, "events": {}, "commands": {}, "scope": { "heading": { "kind": "string" } } },
    "view": { "props": {}, "events": {},
      "commands": { "greet": { "parameters": [ { "name": "who", "type": { "kind": "string" } } ] } },
      "scope": { "title": { "kind": "string" }, "who": { "kind": "string" }, "flag": { "kind": "boolean" } } }
  }
}"#;

/// A card with three places: a header, the default, and a footer, in that order after its own first note.
const CARD: &str = r#"<panel heading={heading}><note>top</note><mesh-slot name="header" /><mesh-slot /><mesh-slot name="footer" /></panel>"#;

const STATE: &str = r#"{"title":"T","who":"Ada","flag":true}"#;

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

fn handler(node: &Node, event: &str) -> Option<String> {
    node.events.get(event).cloned().or_else(|| {
        node.children.iter().find_map(|child| match child {
            TreeChild::Node(node) => handler(node, event),
            TreeChild::Text { .. } => None,
        })
    })
}

#[test]
fn each_fill_goes_to_its_slot_whatever_the_order_it_is_written_in() {
    let rendered = program(
        r#"<page><card heading={title}><mesh-fill slot="footer"><note>F {who}</note></mesh-fill><note>body {who}</note><mesh-fill slot="header"><note>H</note></mesh-fill></card></page>"#,
        &[("card", CARD)],
        STATE,
    );

    assert_eq!(shown(&rendered), ["top", "H", "body Ada", "F Ada"]);
}

#[test]
fn a_slot_nobody_fills_is_empty_and_the_default_may_be_empty_too() {
    let only_header = program(
        r#"<page><card heading={title}><mesh-fill slot="header"><note>H</note></mesh-fill></card></page>"#,
        &[("card", CARD)],
        STATE,
    );
    let nothing = program(
        "<page><card heading={title} /></page>",
        &[("card", CARD)],
        STATE,
    );

    assert_eq!(shown(&only_header), ["top", "H"]);
    assert_eq!(shown(&nothing), ["top"]);
}

#[test]
fn a_fill_is_the_callers_content_so_it_reads_the_callers_scope_and_calls_the_callers_commands() {
    let rendered = program(
        r#"<page><card heading="H"><mesh-fill slot="header"><button on.tap={greet(who)}>{title}</button></mesh-fill></card></page>"#,
        &[("card", CARD)],
        STATE,
    );
    let tap = handler(&rendered.tree().root, "tap").expect("the button has a handler");
    let intent = dispatch(&rendered, &tap, None::<&HostValue>).expect("dispatches");

    assert_eq!(shown(&rendered), ["top", "T"]);
    assert_eq!(intent.command, "greet");
    assert_eq!(intent.component, "view");
}

#[test]
fn a_fill_may_hold_conditionals_and_text_and_an_update_agrees_with_a_fresh_render() {
    let view = r#"<page><card heading={title}><mesh-fill slot="header">hello <mesh-if when={flag}><note>on</note></mesh-if></mesh-fill>body</card></page>"#;
    let mut current = program(view, &[("card", CARD)], STATE);

    for state in [
        r#"{"title":"T","who":"Ada","flag":false}"#,
        r#"{"title":"U","who":"Bob","flag":true}"#,
        STATE,
    ] {
        let updated = update(&current, &snapshot(state)).expect("updates");
        let full = program(view, &[("card", CARD)], state);

        assert_eq!(updated.render.tree(), full.tree(), "{state}");
        current = updated.render;
    }
}

#[test]
fn a_slot_forwarded_through_another_composite_still_finds_its_fill() {
    // `shell` places its own header into the card's header, and its default into the card's default.
    let shell = r#"<card heading={heading}><mesh-fill slot="header"><mesh-slot name="title" /></mesh-fill><mesh-slot /></card>"#;
    let model = MODEL.replace(
        r#""view": {"#,
        r#""shell": { "props": { "heading": { "type": { "kind": "string" }, "required": true } }, "events": {}, "commands": {}, "scope": { "heading": { "kind": "string" } } },
    "view": {"#,
    );
    let view = r#"<page><shell heading={title}><mesh-fill slot="title"><note>T!</note></mesh-fill><note>body</note></shell></page>"#;
    let templates = [
        compile_with(&model, "view", view),
        compile_with(&model, "shell", shell),
        compile_with(&model, "card", CARD),
    ];
    let texts: Vec<&str> = templates.iter().map(String::as_str).collect();
    let rendered = render(
        &Program {
            root: "view",
            templates: &texts,
        },
        &model,
        &snapshot(STATE),
    )
    .expect("renders");

    assert_eq!(shown(&rendered), ["top", "T!", "body"]);
}

#[test]
fn the_same_content_in_two_slots_never_shares_a_key() {
    let both = r#"<panel heading={heading}><mesh-slot name="a" /><mesh-slot name="b" /></panel>"#;
    let rendered = program(
        r#"<page><card heading={title}><mesh-fill slot="a"><note>x</note></mesh-fill><mesh-fill slot="b"><note>x</note></mesh-fill></card></page>"#,
        &[("card", both)],
        STATE,
    );
    let mut keys = Vec::new();
    fn collect(node: &Node, keys: &mut Vec<String>) {
        keys.push(node.key.clone());
        for child in &node.children {
            if let TreeChild::Node(node) = child {
                collect(node, keys);
            }
        }
    }
    collect(&rendered.tree().root, &mut keys);
    let unique: std::collections::BTreeSet<_> = keys.iter().collect();

    assert_eq!(unique.len(), keys.len(), "{keys:?}");
}

#[test]
fn the_assembly_rules_for_fills() {
    let card = [("card", CARD)];
    let refused = |view: &str, others: &[(&str, &str)]| {
        codes(&try_program(view, others, STATE).expect_err("is refused"))
            .into_iter()
            .collect::<Vec<_>>()
    };

    // A fill for a slot the composite doesn't have.
    assert_eq!(
        refused(
            r#"<page><card heading={title}><mesh-fill slot="side"><note>x</note></mesh-fill></card></page>"#,
            &card
        ),
        ["assembly-composite-children"]
    );
    // The same slot filled twice.
    assert_eq!(
        refused(
            r#"<page><card heading={title}><mesh-fill slot="header"><note>x</note></mesh-fill><mesh-fill slot="header"><note>y</note></mesh-fill></card></page>"#,
            &card
        ),
        ["assembly-composite-children"]
    );
    // Children outside a fill, for a composite with no default slot.
    let named_only = r#"<panel heading={heading}><mesh-slot name="header" /></panel>"#;
    assert_eq!(
        refused(
            r#"<page><card heading={title}><note>loose</note></card></page>"#,
            &[("card", named_only)]
        ),
        ["assembly-composite-children"]
    );
    // A fill that is not directly inside a composite occurrence.
    assert_eq!(
        refused(
            r#"<page><panel heading={title}><mesh-fill slot="header"><note>x</note></mesh-fill></panel></page>"#,
            &card
        ),
        ["assembly-composite-children"]
    );
    // Two slots of one name in a template.
    let twice = r#"<panel heading={heading}><mesh-slot name="a" /><mesh-slot name="a" /></panel>"#;
    assert_eq!(
        refused(
            r#"<page><card heading={title} /></page>"#,
            &[("card", twice)]
        ),
        ["assembly-malformed-template"]
    );
    // Two unnamed slots, as before.
    let unnamed = r#"<panel heading={heading}><mesh-slot /><mesh-slot /></panel>"#;
    assert_eq!(
        refused(
            r#"<page><card heading={title} /></page>"#,
            &[("card", unnamed)]
        ),
        ["assembly-malformed-template"]
    );
}
