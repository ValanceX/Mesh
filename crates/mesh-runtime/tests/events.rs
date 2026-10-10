//! Composite events: a composite declares events in the model, its template
//! forwards a primitive's event to one of them by naming it as the handler
//! (`on.tap={select(count)}`, where `select` is an event of the composite and
//! not a command), and the occurrence binds the event to a command of the
//! template it is in (`<card on.select={pick($event)} />`). A forward nothing
//! binds isn't in the render tree, so resolution goes on to the nodes above.

mod common;

use common::{codes, compile_with, snapshot};
use mesh_compiler::check;
use mesh_runtime::{
    declared_events, dispatch, render, update, HostValue, Intent, Node, Program, Render,
    RuntimeDiagnostic, TreeChild,
};
use serde_json::Value;

const MODEL: &str = r#"{
  "version": 1, "types": {},
  "components": {
    "page":   { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "panel":  { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "button": { "props": {}, "events": { "tap": {} }, "commands": {}, "scope": {} },
    "mesh-slot": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "mesh-if": { "props": { "when": { "type": { "kind": "boolean" }, "required": true } }, "events": {}, "commands": {}, "scope": {} },
    "mesh-each": { "props": {
        "items": { "type": { "kind": "list", "element": { "kind": "any" } }, "required": true },
        "as":    { "type": { "kind": "string" }, "required": true },
        "key":   { "type": { "kind": "any" }, "required": true } },
      "events": {}, "commands": {}, "scope": {} },
    "card": { "props": { "count": { "type": { "kind": "number" }, "required": true } },
              "events": { "select": { "payload": { "kind": "number" } }, "close": {} },
              "commands": { "log": { "parameters": [ { "name": "count", "type": { "kind": "number" } } ] } },
              "scope": { "count": { "kind": "number" } } },
    "shell": { "props": { "count": { "type": { "kind": "number" }, "required": true } },
               "events": { "chosen": { "payload": { "kind": "number" } } },
               "commands": {}, "scope": { "count": { "kind": "number" } } },
    "view": { "props": {}, "events": {},
      "commands": { "pick":   { "parameters": [ { "name": "n", "type": { "kind": "number" } } ] },
                    "closed": { "parameters": [] } },
      "scope": {
        "n": { "kind": "number" }, "flag": { "kind": "boolean" },
        "items": { "kind": "list", "element": { "kind": "record", "fields": {
            "id": { "type": { "kind": "number" }, "required": true } } } } } }
  }
}"#;

/// Two buttons: one forwards `select` with the card's count, one `close`.
const CARD: &str = "<panel><button on.tap={select(count)}>pick</button><button on.tap={close()}>close</button></panel>";
/// A shell that passes the card's `select` on as its own `chosen`.
const SHELL: &str = "<card count={count} on.select={chosen($event)} />";

const STATE: &str = r#"{"n":7,"flag":true,"items":[{"id":10},{"id":20}]}"#;

fn compile(component: &str, source: &str) -> String {
    compile_with(MODEL, component, source)
}

fn try_program(view: &str, others: &[(&str, &str)]) -> Result<Render, Vec<RuntimeDiagnostic>> {
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
        &snapshot(STATE),
    )
}

fn program(view: &str, others: &[(&str, &str)]) -> Render {
    try_program(view, others).unwrap_or_else(|d| panic!("renders: {d:#?}"))
}

fn buttons(node: &Node, out: &mut Vec<Node>) {
    if node.component == "button" {
        out.push(node.clone());
    }
    for child in &node.children {
        if let TreeChild::Node(child) = child {
            buttons(child, out);
        }
    }
}

fn button_nodes(render: &Render) -> Vec<Node> {
    let mut out = Vec::new();
    buttons(&render.tree().root, &mut out);
    out
}

fn intent(render: &Render, handler: &str) -> Value {
    let intent: Intent = dispatch(render, handler, None::<&HostValue>)
        .unwrap_or_else(|d| panic!("dispatches: {d:#?}"));
    serde_json::from_str(&intent.to_json()).unwrap()
}

fn command(intent: &Value) -> (&str, &str) {
    (
        intent["command"]["component"].as_str().unwrap(),
        intent["command"]["name"].as_str().unwrap(),
    )
}

// --- forwarding --------------------------------------------------------------

#[test]
fn a_forwarded_event_reaches_the_command_the_occurrence_binds() {
    let rendered = program(
        "<page><card count={n} on.select={pick($event)} on.close={closed()} /></page>",
        &[("card", CARD)],
    );
    let buttons = button_nodes(&rendered);
    assert_eq!(buttons.len(), 2);
    // The first button forwards `select` with the card's count: the page's `pick(7)`.
    let first = intent(&rendered, &buttons[0].events["tap"]);
    assert_eq!(command(&first), ("view", "pick"));
    assert_eq!(first["arguments"][0]["value"], 7);
    // The second forwards `close`, which has no payload: the page's `closed()`.
    let second = intent(&rendered, &buttons[1].events["tap"]);
    assert_eq!(command(&second), ("view", "closed"));
    assert_eq!(second["arguments"].as_array().unwrap().len(), 0);
}

#[test]
fn the_occurrences_binding_sees_the_callers_scope_and_the_payload() {
    // `pick(n)` reads the page's `n`, not the payload; `$event` is the payload.
    let rendered = program(
        "<page><card count={n} on.select={pick(n)} /></page>",
        &[("card", CARD)],
    );
    let buttons = button_nodes(&rendered);
    let intent = intent(&rendered, &buttons[0].events["tap"]);
    assert_eq!(intent["arguments"][0]["value"], 7);
}

#[test]
fn an_event_nothing_binds_is_not_in_the_tree() {
    let rendered = program(
        "<page><card count={n} on.select={pick($event)} /></page>",
        &[("card", CARD)],
    );
    let buttons = button_nodes(&rendered);
    // `select` is bound; `close` is not, so its button has no `tap` at all.
    assert!(buttons[0].events.contains_key("tap"));
    assert!(buttons[1].events.is_empty(), "{:?}", buttons[1].events);
    // And a card with neither bound has neither.
    let bare = program("<page><card count={n} /></page>", &[("card", CARD)]);
    assert!(button_nodes(&bare).iter().all(|b| b.events.is_empty()));
}

#[test]
fn a_forward_goes_out_through_two_composites() {
    let rendered = program(
        "<page><shell count={n} on.chosen={pick($event)} /></page>",
        &[("shell", SHELL), ("card", CARD)],
    );
    let buttons = button_nodes(&rendered);
    // Only `select` is forwarded all the way: shell binds it, the page binds `chosen`.
    assert!(buttons[0].events.contains_key("tap"));
    assert!(buttons[1].events.is_empty());
    let intent = intent(&rendered, &buttons[0].events["tap"]);
    assert_eq!(command(&intent), ("view", "pick"));
    assert_eq!(intent["arguments"][0]["value"], 7);
    // And without the page binding `chosen`, the chain is dead at the end.
    let dead = program(
        "<page><shell count={n} /></page>",
        &[("shell", SHELL), ("card", CARD)],
    );
    assert!(button_nodes(&dead).iter().all(|b| b.events.is_empty()));
}

#[test]
fn a_composite_may_use_its_own_commands_beside_forwarding() {
    let rendered = program(
        "<page><card count={n} on.select={pick($event)} /></page>",
        &[(
            "card",
            "<panel><button on.tap={select(count)}>a</button><button on.tap={log(count)}>b</button></panel>",
        )],
    );
    let buttons = button_nodes(&rendered);
    let forward = intent(&rendered, &buttons[0].events["tap"]);
    assert_eq!(command(&forward), ("view", "pick"));
    let own = intent(&rendered, &buttons[1].events["tap"]);
    assert_eq!(command(&own), ("card", "log"));
}

#[test]
fn a_composite_in_a_repeat_gives_each_item_its_own_values() {
    let rendered = program(
        "<page><mesh-each items={items} as=\"item\" key={item.id}><card count={item.id} on.select={pick(item.id)} /></mesh-each></page>",
        &[("card", CARD)],
    );
    let selects: Vec<String> = button_nodes(&rendered)
        .iter()
        .filter_map(|b| b.events.get("tap").cloned())
        .collect();
    assert_eq!(selects.len(), 2);
    let ids: Vec<Value> = selects
        .iter()
        .map(|h| intent(&rendered, h)["arguments"][0]["value"].clone())
        .collect();
    assert_eq!(ids, [10, 20]);
}

#[test]
fn content_a_composite_places_forwards_the_callers_own_events() {
    // The button is written in the *shell's* template, and placed in a card's
    // slot; the shell's event `chosen` is what it forwards.
    let rendered = program(
        "<page><shell count={n} on.chosen={pick($event)} /></page>",
        &[
            (
                "shell",
                "<card count={count}><button on.tap={chosen(count)}>go</button></card>",
            ),
            ("card", "<panel><mesh-slot /></panel>"),
        ],
    );
    let buttons = button_nodes(&rendered);
    assert_eq!(buttons.len(), 1);
    let intent = intent(&rendered, &buttons[0].events["tap"]);
    assert_eq!(command(&intent), ("view", "pick"));
    assert_eq!(intent["arguments"][0]["value"], 7);
}

// --- what the host is told ---------------------------------------------------

#[test]
fn declared_events_are_commands_the_host_handles_not_forwards() {
    let view = compile(
        "view",
        "<page><card count={n} on.select={pick($event)} /></page>",
    );
    let card = compile("card", CARD);
    let texts = [view.as_str(), card.as_str()];
    let events = declared_events(
        &Program {
            root: "view",
            templates: &texts,
        },
        MODEL,
    )
    .expect("a valid program");
    let mut seen: Vec<(String, String, String)> = events
        .iter()
        .map(|e| (e.component.clone(), e.event.clone(), e.command.clone()))
        .collect();
    seen.sort();
    // The page's binding of `select` and the card's own button, `close()`... which is
    // a forward too, so only the page's `pick` is a command a host handles.
    assert_eq!(
        seen,
        [("view".into(), "select".into(), "pick".into())],
        "{seen:?}"
    );
}

// --- the compiler ------------------------------------------------------------

fn check_card(source: &str) -> Vec<String> {
    let model = check::Model::load(MODEL, "card").expect("the model loads");
    check::template(source, &model)
        .diagnostics
        .iter()
        .map(|d| format!("{:?}", d.code))
        .collect()
}

#[test]
fn the_compiler_checks_a_forward_against_the_events_payload() {
    assert!(check_card("<panel><button on.tap={select(count)} /></panel>").is_empty());
    // An event with a payload needs one argument, of its type; one without, none.
    assert!(!check_card("<panel><button on.tap={select()} /></panel>").is_empty());
    assert!(!check_card("<panel><button on.tap={select(\"x\")} /></panel>").is_empty());
    assert!(!check_card("<panel><button on.tap={close(1)} /></panel>").is_empty());
    assert!(check_card("<panel><button on.tap={close()} /></panel>").is_empty());
    // A name that is neither a command nor an event is still unknown.
    assert!(!check_card("<panel><button on.tap={nope()} /></panel>").is_empty());
}

// --- update ------------------------------------------------------------------

#[test]
fn an_updates_tree_has_the_forwards_a_render_has() {
    let view = "<page><mesh-if when={flag}><card count={n} on.select={pick($event)} /></mesh-if><card count={n} /></page>";
    let first = program(view, &[("card", CARD)]);
    let flipped = STATE.replace("true", "false");
    let updated = update(&first, &snapshot(&flipped)).expect("updates");
    let full = {
        let templates = [compile("view", view), compile("card", CARD)];
        let texts: Vec<&str> = templates.iter().map(String::as_str).collect();
        render(
            &Program {
                root: "view",
                templates: &texts,
            },
            MODEL,
            &snapshot(&flipped),
        )
        .expect("renders")
    };
    assert_eq!(updated.render.tree(), full.tree());
}

#[test]
fn the_other_assembly_rules_still_hold_for_a_composite_with_events() {
    // A composite with events and children but no slot is still refused.
    let diagnostics = try_program(
        "<page><card count={n}><button /></card></page>",
        &[("card", CARD)],
    )
    .unwrap_err();
    assert_eq!(codes(&diagnostics), ["assembly-composite-children"]);
}
