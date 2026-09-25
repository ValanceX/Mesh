//! Dispatch (D5): the lifecycle, the intent, and every case the
//! Definition of Done's "Dispatch" and "Inputs" lists name for payloads
//! and handler identifiers.

mod common;

use common::{codes, compile, dispatch, handler, snapshot, try_render, view, SNAPSHOT};
use mesh_runtime::{HostKey, HostValue, Location, PathSegment};
use serde_json::{json, Value};

const PAGE: &str = r#"<page>
  <probe on.click={place($event)} on.tap={select(user)} on.pick={take($event)} />
  <probe on.tap={setName(maybeName)} on.pick={take(nothing)} on.maybe={setName($event)} />
</page>"#;

fn intent_json(intent: &mesh_runtime::Intent) -> Value {
    serde_json::from_str(&intent.to_json()).unwrap()
}

fn render() -> mesh_runtime::Render {
    view(PAGE).unwrap()
}

/// The handler identifier of `event` on the `n`th probe.
fn nth(render: &mesh_runtime::Render, n: usize, event: &str) -> String {
    match &render.tree().root.children[n] {
        mesh_runtime::TreeChild::Node(node) => node.events[event].clone(),
        other => panic!("{other:?}"),
    }
}

fn payload(text: &str) -> HostValue {
    HostValue::from_json(text).unwrap()
}

fn input(segments: &[&str]) -> Location {
    Location::Input(
        segments
            .iter()
            .map(|s| PathSegment::Name((*s).to_string()))
            .collect(),
    )
}

#[test]
fn an_intent_holds_the_command_and_its_evaluated_arguments() {
    let render = render();
    let intent = dispatch(&render, "click", Some(r#"{ "x": 1, "y": -0 }"#)).unwrap();
    assert_eq!(
        intent_json(&intent),
        json!({
            "command": { "component": "view", "name": "place" },
            "arguments": [{ "value": { "x": 1, "y": 0 } }],
        })
    );
    let intent = dispatch(&render, "tap", None).unwrap();
    assert_eq!(intent.command, "select");
    assert_eq!(
        intent.arguments,
        [Some(
            json!({ "name": "Ada", "avatar": "ada.png", "active": true })
        )]
    );
}

#[test]
fn an_absent_argument_is_distinct_from_null() {
    let render = render();
    let absent = mesh_runtime::dispatch(&render, &nth(&render, 1, "tap"), None).unwrap();
    assert_eq!(absent.arguments, [None]);
    assert_eq!(
        intent_json(&absent)["arguments"],
        json!([{ "absent": true }])
    );
    // `pick`'s payload is `any`, which excludes absence: give one.
    let null =
        mesh_runtime::dispatch(&render, &nth(&render, 1, "pick"), Some(&payload("0"))).unwrap();
    assert_eq!(null.arguments, [Some(Value::Null)]);
    assert_eq!(intent_json(&null)["arguments"], json!([{ "value": null }]));
    // An optional payload, absent: an absent argument.
    let maybe = mesh_runtime::dispatch(&render, &nth(&render, 1, "maybe"), None).unwrap();
    assert_eq!(maybe.arguments, [None]);
}

#[test]
fn a_handler_identifier_must_be_this_programs_and_name_a_handler() {
    let render = render();
    let other = view("<probe on.tap={save()} />").unwrap();
    let foreign = handler(&other, "tap");
    let diagnostics = mesh_runtime::dispatch(&render, &foreign, None).unwrap_err();
    assert_eq!(codes(&diagnostics), ["runtime-handler-other-program"]);
    assert_eq!(diagnostics[0].location, Location::Handler);
    for garbage in ["", "h", "not a handler", "k1234567890123456789012"] {
        let diagnostics = mesh_runtime::dispatch(&render, garbage, None).unwrap_err();
        assert_eq!(
            codes(&diagnostics),
            ["runtime-handler-other-program"],
            "{garbage:?}"
        );
    }
    let ours = handler(&render, "tap");
    let unknown = format!("{}.AAAAAAAAAAAAAAAAAAAAAA", &ours[..12]);
    let diagnostics = mesh_runtime::dispatch(&render, &unknown, None).unwrap_err();
    assert_eq!(codes(&diagnostics), ["runtime-unknown-handler"]);
    assert_eq!(diagnostics[0].location, Location::Handler);
}

#[test]
fn an_invalid_handler_is_the_only_diagnostic_even_with_a_bad_payload() {
    let render = render();
    let diagnostics =
        mesh_runtime::dispatch(&render, "nope", Some(&HostValue::Number(f64::NAN))).unwrap_err();
    assert_eq!(codes(&diagnostics), ["runtime-handler-other-program"]);
}

#[test]
fn payloads_are_checked_against_their_events_type() {
    let render = render();
    // Doesn't fit.
    let diagnostics = dispatch(&render, "click", Some(r#"{ "x": "one", "y": 2 }"#)).unwrap_err();
    assert_eq!(codes(&diagnostics), ["runtime-value-mismatch"]);
    assert_eq!(diagnostics[0].location, input(&["$event", "x"]));
    // Given for an event without a payload.
    let diagnostics = dispatch(&render, "tap", Some("1")).unwrap_err();
    assert_eq!(codes(&diagnostics), ["runtime-unexpected-payload"]);
    assert_eq!(diagnostics[0].location, input(&["$event"]));
    // Missing where one is required.
    let diagnostics = dispatch(&render, "click", None).unwrap_err();
    assert_eq!(codes(&diagnostics), ["runtime-missing-value"]);
    assert_eq!(diagnostics[0].location, input(&["$event"]));
    // `any` still refuses what the boundary does.
    let bad = HostValue::List(vec![None]);
    let diagnostics =
        mesh_runtime::dispatch(&render, &handler(&render, "pick"), Some(&bad)).unwrap_err();
    assert_eq!(codes(&diagnostics), ["runtime-absent-element"]);
}

#[test]
fn every_payload_mismatch_is_reported_in_path_order() {
    let render = render();
    let diagnostics = dispatch(&render, "click", Some(r#"{ "y": "two", "z": 3 }"#)).unwrap_err();
    let found: Vec<(&str, Location)> = diagnostics
        .iter()
        .map(|d| (d.code, d.location.clone()))
        .collect();
    assert_eq!(
        found,
        [
            ("runtime-missing-value", input(&["$event", "x"])),
            ("runtime-value-mismatch", input(&["$event", "y"])),
            ("runtime-unknown-field", input(&["$event", "z"])),
        ]
    );
}

#[test]
fn dispatch_uses_the_renders_snapshot() {
    let template = compile("view", PAGE);
    let first = try_render("view", std::slice::from_ref(&template), SNAPSHOT).unwrap();
    let later = SNAPSHOT.replace(
        "\"name\": \"Ada\", \"avatar\"",
        "\"name\": \"Grace\", \"avatar\"",
    );
    let second = try_render("view", std::slice::from_ref(&template), &later).unwrap();
    let from_first = dispatch(&first, "tap", None).unwrap();
    let from_second = dispatch(&second, "tap", None).unwrap();
    assert_eq!(
        from_first.arguments[0].as_ref().unwrap()["name"],
        json!("Ada")
    );
    assert_eq!(
        from_second.arguments[0].as_ref().unwrap()["name"],
        json!("Grace")
    );
}

#[test]
fn changing_the_hosts_snapshot_after_render_changes_nothing() {
    let template = compile("view", PAGE);
    let mut record = snapshot(SNAPSHOT);
    let render = mesh_runtime::render(
        &mesh_runtime::Program {
            root: "view",
            templates: &[&template],
        },
        common::MODEL,
        &record,
    )
    .unwrap();
    for (key, value) in &mut record.0 {
        if *key == HostKey::Text("user".into()) {
            *value = HostValue::Null;
        }
    }
    let intent = dispatch(&render, "tap", None).unwrap();
    assert_eq!(intent.arguments[0].as_ref().unwrap()["name"], json!("Ada"));
}

#[test]
fn arguments_are_checked_and_output_checked() {
    let render = view("<page><probe on.tap={setCount(anything)} /><probe on.pick={take({ a: [1 / 0] })} /></page>").unwrap();
    let diagnostics = mesh_runtime::dispatch(&render, &nth(&render, 0, "tap"), None).unwrap_err();
    assert_eq!(codes(&diagnostics), ["runtime-argument-mismatch"]);
    let diagnostics =
        mesh_runtime::dispatch(&render, &nth(&render, 1, "pick"), Some(&payload("1"))).unwrap_err();
    assert_eq!(codes(&diagnostics), ["runtime-non-finite-output"]);
}

#[test]
fn handlers_are_not_evaluated_at_render() {
    view("<probe on.tap={setCount(anything)} />")
        .expect("a handler's arguments are evaluated at dispatch only");
}

#[test]
fn dispatch_through_a_composite_gives_the_composites_command() {
    let card = compile("card", "<page><probe on.tap={selectUser(user)} /></page>");
    let page = compile("view", "<page><card user={user} /></page>");
    let render = try_render("view", &[page, card], SNAPSHOT).unwrap();
    let intent = dispatch(&render, "tap", None).unwrap();
    assert_eq!(
        intent_json(&intent)["command"],
        json!({ "component": "card", "name": "selectUser" })
    );
    assert_eq!(intent.arguments[0].as_ref().unwrap()["name"], json!("Ada"));
}

#[test]
fn dispatch_through_two_composites_derives_each_scope() {
    let card = compile("card", "<page><probe on.tap={selectUser(user)} /></page>");
    let frame = compile("frame", "<page><card user={user} compact={true} /></page>");
    let page = compile("view", "<page><frame user={user} /></page>");
    let later = SNAPSHOT.replace(
        "\"name\": \"Ada\", \"avatar\"",
        "\"name\": \"Hopper\", \"avatar\"",
    );
    let render = try_render("view", &[page, frame, card], &later).unwrap();
    let intent = dispatch(&render, "tap", None).unwrap();
    assert_eq!(intent.component, "card");
    assert_eq!(
        intent.arguments[0].as_ref().unwrap()["name"],
        json!("Hopper")
    );
}
