//! Composite children and the slot (docs/manual/templates.md, "Children and
//! the slot"): a composite's template places the children of an occurrence
//! with `mesh-slot`, and they are the *caller's*: evaluated in the scope they
//! were written in, calling the caller's commands, and named by paths of their
//! own. Also what an update does with them: the law (an update's tree is a
//! render's) over random states, and that nothing stale is kept.

mod common;

use common::{codes, compile_with, snapshot};
use mesh_runtime::{
    dispatch, patches_to_json, render, update, HostValue, Intent, Node, Program, Render,
    RuntimeDiagnostic, TreeChild,
};
use serde_json::Value;

const MODEL: &str = r#"{
  "version": 1, "types": {},
  "components": {
    "page":  { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "note":  { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "button": { "props": {}, "events": { "tap": {} }, "commands": {}, "scope": {} },
    "panel": { "props": { "heading": { "type": { "kind": "string" }, "required": true } },
               "events": { "tap": {} }, "commands": {}, "scope": {} },
    "mesh-slot": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "mesh-if": { "props": { "when": { "type": { "kind": "boolean" }, "required": true } }, "events": {}, "commands": {}, "scope": {} },
    "mesh-each": { "props": {
        "items": { "type": { "kind": "list", "element": { "kind": "any" } }, "required": true },
        "as":    { "type": { "kind": "string" }, "required": true },
        "key":   { "type": { "kind": "any" }, "required": true } },
      "events": {}, "commands": {}, "scope": {} },
    "card": { "props": { "heading": { "type": { "kind": "string" }, "required": true } },
              "events": {}, "commands": { "cardTap": { "parameters": [ { "name": "heading", "type": { "kind": "string" } } ] } },
              "scope": { "heading": { "kind": "string" } } },
    "shell": { "props": { "label": { "type": { "kind": "string" }, "required": true } },
               "events": {}, "commands": {}, "scope": { "label": { "kind": "string" } } },
    "view": { "props": {}, "events": {},
      "commands": { "greet": { "parameters": [ { "name": "who", "type": { "kind": "string" } } ] },
                    "pick":  { "parameters": [ { "name": "id", "type": { "kind": "number" } } ] } },
      "scope": {
        "title": { "kind": "string" }, "who": { "kind": "string" }, "flag": { "kind": "boolean" },
        "items": { "kind": "list", "element": { "kind": "record", "fields": {
            "id": { "type": { "kind": "number" }, "required": true },
            "label": { "type": { "kind": "string" }, "required": true } } } } } }
  }
}"#;

/// A card with a place for its caller's content, between two notes of its own.
const CARD: &str =
    "<panel heading={heading} on.tap={cardTap(heading)}><note>top</note><mesh-slot /><note>end</note></panel>";

/// A shell that hands its own content on to a card.
const SHELL: &str = "<card heading={label}><mesh-slot /></card>";

fn compile(component: &str, source: &str) -> String {
    compile_with(MODEL, component, source)
}

fn try_program(
    view: &str,
    others: &[(&str, &str)],
    state: &str,
) -> Result<Render, Vec<RuntimeDiagnostic>> {
    let mut templates = vec![compile("view", view)];
    for (component, source) in others {
        templates.push(compile(component, source));
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

const STATE: &str =
    r#"{"title":"T","who":"Ada","flag":true,"items":[{"id":1,"label":"a"},{"id":2,"label":"b"}]}"#;

fn texts(node: &Node, out: &mut Vec<String>) {
    for child in &node.children {
        match child {
            TreeChild::Text { text, .. } => out.push(text.clone()),
            TreeChild::Node(node) => texts(node, out),
        }
    }
}

fn components(node: &Node) -> Vec<String> {
    node.children
        .iter()
        .filter_map(|child| match child {
            TreeChild::Node(node) => Some(node.component.clone()),
            TreeChild::Text { .. } => None,
        })
        .collect()
}

fn keys(node: &Node, out: &mut Vec<String>) {
    out.push(node.key.clone());
    for child in &node.children {
        match child {
            TreeChild::Text { key, .. } => out.push(key.clone()),
            TreeChild::Node(node) => keys(node, out),
        }
    }
}

fn find_handler(node: &Node, component: &str, event: &str) -> Option<String> {
    if node.component == component {
        if let Some(handler) = node.events.get(event) {
            return Some(handler.clone());
        }
    }
    node.children.iter().find_map(|child| match child {
        TreeChild::Node(node) => find_handler(node, component, event),
        TreeChild::Text { .. } => None,
    })
}

fn intent(render: &Render, handler: &str) -> Intent {
    dispatch(render, handler, None::<&HostValue>).unwrap_or_else(|d| panic!("dispatches: {d:#?}"))
}

// --- what a slot places ------------------------------------------------------

#[test]
fn the_occurrences_children_are_placed_where_the_slot_is() {
    let rendered = program(
        "<page><card heading={title}><note>hello {who}</note><button on.tap={greet(who)}>go</button></card></page>",
        &[("card", CARD)],
        STATE,
    );
    let panel = match &rendered.tree().root.children[0] {
        TreeChild::Node(panel) => panel.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(panel.component, "panel");
    // The card's own notes, with the caller's content between them, in order.
    assert_eq!(components(&panel), ["note", "note", "button", "note"]);
    let mut all = Vec::new();
    texts(&panel, &mut all);
    assert_eq!(all, ["top", "hello Ada", "go", "end"]);
}

#[test]
fn the_content_is_evaluated_in_the_callers_scope_not_the_composites() {
    // `heading` is the card's scope name and `title` the caller's: the content
    // reads the caller's, and the card's own scope doesn't reach it.
    let rendered = program(
        "<page><card heading=\"H\"><note>{title}</note></card></page>",
        &[("card", CARD)],
        STATE,
    );
    let mut all = Vec::new();
    texts(&rendered.tree().root, &mut all);
    assert_eq!(all, ["top", "T", "end"]);
}

#[test]
fn an_occurrence_with_no_children_leaves_the_slot_empty() {
    let rendered = program(
        "<page><card heading={title} /></page>",
        &[("card", CARD)],
        STATE,
    );
    let mut all = Vec::new();
    texts(&rendered.tree().root, &mut all);
    assert_eq!(all, ["top", "end"]);
}

#[test]
fn text_beside_a_slot_is_one_run_as_a_render_tree_has_it() {
    // A text run before the slot and the content's text after it are adjacent:
    // a render tree's text runs are maximal, so they are one.
    let rendered = program(
        "<page><card heading={title}>middle</card></page>",
        &[(
            "card",
            "<panel heading={heading}>before <mesh-slot /> after</panel>",
        )],
        STATE,
    );
    let panel = match &rendered.tree().root.children[0] {
        TreeChild::Node(panel) => panel.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(panel.children.len(), 1, "one run");
    let mut all = Vec::new();
    texts(&panel, &mut all);
    assert_eq!(all, ["before middle after"]);
}

#[test]
fn a_conditional_and_a_repeat_among_the_children_work_in_the_callers_scope() {
    let rendered = program(
        "<page><card heading={title}><mesh-if when={flag}><note>on</note></mesh-if><mesh-each items={items} as=\"item\" key={item.id}><note>{item.label}</note></mesh-each></card></page>",
        &[("card", CARD)],
        STATE,
    );
    let mut all = Vec::new();
    texts(&rendered.tree().root, &mut all);
    assert_eq!(all, ["top", "on", "a", "b", "end"]);
}

#[test]
fn a_slot_forwards_the_callers_content_through_another_composite() {
    let rendered = program(
        "<page><shell label={title}><note>from the page: {who}</note></shell></page>",
        &[("shell", SHELL), ("card", CARD)],
        STATE,
    );
    let mut all = Vec::new();
    texts(&rendered.tree().root, &mut all);
    assert_eq!(all, ["top", "from the page: Ada", "end"]);
}

// --- identity ----------------------------------------------------------------

#[test]
fn the_same_content_in_two_occurrences_has_different_keys_and_every_key_is_unique() {
    let rendered = program(
        "<page><card heading=\"one\"><note>same</note></card><card heading=\"two\"><note>same</note></card></page>",
        &[("card", CARD)],
        STATE,
    );
    let mut all = Vec::new();
    keys(&rendered.tree().root, &mut all);
    let unique: std::collections::BTreeSet<&String> = all.iter().collect();
    assert_eq!(unique.len(), all.len(), "{all:?}");
    // And the same program renders to the same keys again.
    let again = program(
        "<page><card heading=\"one\"><note>same</note></card><card heading=\"two\"><note>same</note></card></page>",
        &[("card", CARD)],
        STATE,
    );
    let mut second = Vec::new();
    keys(&again.tree().root, &mut second);
    assert_eq!(all, second);
}

#[test]
fn content_placed_by_a_slot_is_named_apart_from_the_hosts_own_children() {
    // The host's own note and the caller's note are both children at position
    // 0 of their templates: the slot's step is what keeps their keys apart.
    let rendered = program(
        "<page><card heading=\"h\"><note>caller</note></card></page>",
        &[(
            "card",
            "<panel heading={heading}><mesh-slot /><note>own</note></panel>",
        )],
        STATE,
    );
    let mut all = Vec::new();
    keys(&rendered.tree().root, &mut all);
    let unique: std::collections::BTreeSet<&String> = all.iter().collect();
    assert_eq!(unique.len(), all.len());
}

// --- handlers ----------------------------------------------------------------

#[test]
fn a_handler_in_the_content_calls_the_callers_command_with_the_callers_values() {
    let rendered = program(
        "<page><card heading={title}><button on.tap={greet(who)}>go</button></card></page>",
        &[("card", CARD)],
        STATE,
    );
    let handler = find_handler(&rendered.tree().root, "button", "tap").expect("a button");
    let intent = intent(&rendered, &handler);
    assert_eq!(intent.component, "view");
    assert_eq!(intent.command, "greet");
    assert_eq!(
        serde_json::from_str::<Value>(&intent.to_json()).unwrap()["arguments"][0]["value"],
        "Ada"
    );
    // And the card's own handler is still the card's, with the card's scope.
    let own = find_handler(&rendered.tree().root, "panel", "tap").expect("a panel");
    let intent = self::intent(&rendered, &own);
    assert_eq!(intent.component, "card");
    assert_eq!(intent.command, "cardTap");
    assert_eq!(
        serde_json::from_str::<Value>(&intent.to_json()).unwrap()["arguments"][0]["value"],
        "T"
    );
}

#[test]
fn a_handler_in_content_forwarded_through_two_composites_reaches_the_callers_command() {
    let rendered = program(
        "<page><shell label={title}><button on.tap={greet(who)}>go</button></shell></page>",
        &[("shell", SHELL), ("card", CARD)],
        STATE,
    );
    let handler = find_handler(&rendered.tree().root, "button", "tap").expect("a button");
    let intent = intent(&rendered, &handler);
    assert_eq!(
        (intent.component.as_str(), intent.command.as_str()),
        ("view", "greet")
    );
}

#[test]
fn a_handler_in_a_repeated_content_item_has_the_items_values() {
    let rendered = program(
        "<page><card heading={title}><mesh-each items={items} as=\"item\" key={item.id}><button on.tap={pick(item.id)}>{item.label}</button></mesh-each></card></page>",
        &[("card", CARD)],
        STATE,
    );
    let mut buttons = Vec::new();
    fn collect(node: &Node, out: &mut Vec<String>) {
        if let Some(handler) = node
            .events
            .get("tap")
            .filter(|_| node.component == "button")
        {
            out.push(handler.clone());
        }
        for child in &node.children {
            if let TreeChild::Node(child) = child {
                collect(child, out);
            }
        }
    }
    collect(&rendered.tree().root, &mut buttons);
    assert_eq!(buttons.len(), 2);
    let ids: Vec<Value> = buttons
        .iter()
        .map(|handler| {
            serde_json::from_str::<Value>(&intent(&rendered, handler).to_json()).unwrap()
                ["arguments"][0]["value"]
                .clone()
        })
        .collect();
    assert_eq!(ids, [1, 2]);
}

// --- assembly ----------------------------------------------------------------

#[test]
fn children_need_a_slot_in_the_composites_template() {
    let diagnostics = try_program(
        "<page><card heading={title}><note>x</note></card></page>",
        &[("card", "<panel heading={heading} />")],
        STATE,
    )
    .unwrap_err();
    assert_eq!(codes(&diagnostics), ["assembly-composite-children"]);
}

#[test]
fn whitespace_between_the_tags_is_still_not_a_child() {
    program(
        "<page><card heading={title}>  </card></page>",
        &[("card", "<panel heading={heading} />")],
        STATE,
    );
}

#[test]
fn a_slot_must_be_well_formed() {
    let bad: [(&str, &str); 4] = [
        (
            "two slots",
            "<panel heading={heading}><mesh-slot /><mesh-slot /></panel>",
        ),
        (
            "a slot with children",
            "<panel heading={heading}><mesh-slot><note>x</note></mesh-slot></panel>",
        ),
        ("a slot as the template's root", "<mesh-slot />"),
        (
            "a slot as a conditional's alternative",
            "<panel heading={heading}><mesh-if when={true}><mesh-slot /></mesh-if></panel>",
        ),
    ];
    for (what, source) in bad {
        let template = std::panic::catch_unwind(|| compile("card", source));
        // Some of these the compiler itself refuses; the rest the runtime must.
        if let Ok(template) = template {
            let view = compile("view", "<page><card heading={title} /></page>");
            let texts = [view.as_str(), template.as_str()];
            let result = render(
                &Program {
                    root: "view",
                    templates: &texts,
                },
                MODEL,
                &snapshot(STATE),
            );
            let diagnostics = result.expect_err(what);
            assert_eq!(
                codes(&diagnostics),
                ["assembly-malformed-template"],
                "{what}"
            );
        }
    }
}

// --- update ------------------------------------------------------------------

/// A small deterministic generator.
struct Rng(u64);
impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) % n
    }
}

fn random_state(rng: &mut Rng, base: &Value) -> Value {
    let mut out = base.clone();
    for _ in 0..=rng.below(3) {
        match rng.below(6) {
            0 => out["title"] = format!("T{}", rng.below(3)).into(),
            1 => out["who"] = format!("W{}", rng.below(3)).into(),
            2 => out["flag"] = (rng.below(2) == 1).into(),
            3 => {
                let mut ids: Vec<u64> = (0..6).collect();
                out["items"] = (0..rng.below(5))
                    .map(|_| {
                        let id = ids.remove(rng.below(ids.len() as u64) as usize);
                        serde_json::json!({ "id": id, "label": format!("l{}", rng.below(3)) })
                    })
                    .collect::<Vec<_>>()
                    .into();
            }
            4 => {
                if let Some(items) = out["items"].as_array_mut() {
                    if items.len() > 1 {
                        let (a, b) = (
                            rng.below(items.len() as u64) as usize,
                            rng.below(items.len() as u64) as usize,
                        );
                        items.swap(a, b);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

#[test]
fn an_updates_tree_is_a_renders_with_slots_in_play() {
    // Content with a conditional and a repeat, placed by one card, forwarded by
    // a shell into another, and a card of the page's own outside any content.
    let view = "<page><card heading={title}><note>{who}</note><mesh-if when={flag}><note>on</note></mesh-if><mesh-each items={items} as=\"item\" key={item.id}><button on.tap={pick(item.id)}>{item.label}</button></mesh-each></card><shell label={who}><note>{title}</note></shell><note>{title}</note></page>";
    let others = [("card", CARD), ("shell", SHELL)];
    let mut rng = Rng(11);
    let mut checked = 0;
    for _ in 0..40 {
        let mut state: Value = serde_json::from_str(STATE).unwrap();
        let mut current = program(view, &others, &state.to_string());
        for _ in 0..25 {
            state = random_state(&mut rng, &state);
            let updated = update(&current, &snapshot(&state.to_string())).expect("updates");
            let full = program(view, &others, &state.to_string());
            assert_eq!(updated.render.tree(), full.tree(), "tree for {state}");
            // The patches name only keys the previous or new tree has, and are valid JSON.
            let patches: Value = serde_json::from_str(&patches_to_json(&updated.patches)).unwrap();
            assert_eq!(patches["format"], "mesh-render-patch");
            current = updated.render;
            checked += 1;
        }
    }
    assert_eq!(checked, 1000);
}

#[test]
fn a_change_the_content_reads_reaches_it_even_when_nothing_the_composite_reads_changed() {
    // The card's props read `title`; the content reads `who`. Changing only
    // `who` must change the content: a node that hosts a slot is never kept as it was.
    let view = "<page><card heading={title}><note>{who}</note></card></page>";
    let first = program(view, &[("card", CARD)], STATE);
    let changed = STATE.replace("Ada", "Grace");
    let updated = update(&first, &snapshot(&changed)).expect("updates");
    let mut all = Vec::new();
    texts(&updated.render.tree().root, &mut all);
    assert_eq!(all, ["top", "Grace", "end"]);
    let doc: Value = serde_json::from_str(&patches_to_json(&updated.patches)).unwrap();
    let ops: Vec<&str> = doc["patches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["op"].as_str().unwrap())
        .collect();
    assert_eq!(ops, ["setText"], "{doc}");
}
