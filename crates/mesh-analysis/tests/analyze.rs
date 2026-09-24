//! Analysis of real MPRX source against a small manifest: the source is
//! parsed and lowered, then analyzed as the template of `view`.

mod common;

use common::{at, names, nth};
use mesh_analysis::{Analysis, Fact, Resolution, Target};
use mesh_syntax::Span;

const MANIFEST: &str = r#"{
  "version": 1,
  "types": {
    "User": { "kind": "record", "fields": {
      "name": { "type": { "kind": "string" }, "required": true }
    } }
  },
  "components": {
    "page": {
      "props": {
        "title": { "type": { "kind": "string" }, "required": true },
        "subtitle": { "type": { "kind": "string" }, "required": false }
      },
      "events": {}, "commands": {}, "scope": {}
    },
    "card": {
      "props": {
        "user": { "type": { "kind": "named", "name": "User" }, "required": true },
        "note": { "type": { "kind": "optional", "type": { "kind": "string" } }, "required": true }
      },
      "events": {
        "select": { "payload": { "kind": "named", "name": "User" } },
        "close": {}
      },
      "commands": {}, "scope": {}
    },
    "view": {
      "props": {}, "events": {},
      "commands": {
        "log": { "parameters": [{ "name": "value", "type": { "kind": "any" } }] },
        "save": { "parameters": [] },
        "select": { "parameters": [{ "name": "user", "type": { "kind": "named", "name": "User" } }] }
      },
      "scope": {
        "user": { "kind": "named", "name": "User" },
        "count": { "kind": "number" }
      }
    }
  }
}"#;

fn analyze(source: &str) -> Analysis {
    common::analyze(MANIFEST, "view", source)
}

#[test]
fn a_template_that_matches_its_manifest_has_no_facts() {
    let source = r#"<page title="Users">
  <card user={user} note={user.name} on.select={select($event)} on.close={save()} />
</page>"#;
    assert_eq!(analyze(source).facts(), []);
}

#[test]
fn resolves_every_name_against_the_manifest() {
    let source =
        r#"<card user={user} note={count == 1 ? "one" : "many"} on.select={select($event)} />"#;
    let target = |span: Span| {
        analyze(source)
            .resolutions()
            .iter()
            .find(|resolution| resolution.span == span)
            .map(|resolution| resolution.target.clone())
    };
    assert_eq!(
        target(at(source, "card")),
        Some(Target::Component("card".to_string()))
    );
    assert_eq!(
        target(at(source, "user")),
        Some(Target::Prop {
            component: "card".to_string(),
            prop: "user".to_string(),
        })
    );
    assert_eq!(
        target(nth(source, "user", 1)),
        Some(Target::Scope("user".to_string()))
    );
    assert_eq!(
        target(at(source, "count")),
        Some(Target::Scope("count".to_string()))
    );
    assert_eq!(
        target(nth(source, "select", 0)),
        Some(Target::Event {
            component: "card".to_string(),
            event: "select".to_string(),
        })
    );
    assert_eq!(
        target(nth(source, "select", 1)),
        Some(Target::Command("select".to_string()))
    );
    assert_eq!(
        target(at(source, "note")),
        Some(Target::Prop {
            component: "card".to_string(),
            prop: "note".to_string(),
        })
    );
    assert_eq!(analyze(source).resolutions().len(), 7);
}

#[test]
fn reports_an_unknown_component_once_and_still_checks_its_expressions() {
    let source = r#"<crad user={usr} on.select={select($event)} />"#;
    assert_eq!(
        analyze(source).facts(),
        [
            Fact::UnknownComponent {
                name: "crad".to_string(),
                span: at(source, "crad"),
                candidates: names(&["card", "page", "view"]),
            },
            Fact::UnknownReference {
                name: "usr".to_string(),
                span: at(source, "usr"),
                candidates: names(&["count", "user"]),
            },
        ]
    );
}

#[test]
fn reports_unknown_props_and_events_with_the_components_names() {
    let source = r#"<card user={user} note={user.name} colour="red" on.selct={save()} />"#;
    assert_eq!(
        analyze(source).facts(),
        [
            Fact::UnknownProp {
                component: "card".to_string(),
                prop: "colour".to_string(),
                span: at(source, "colour"),
                candidates: names(&["note", "user"]),
            },
            Fact::UnknownEvent {
                component: "card".to_string(),
                event: "selct".to_string(),
                span: at(source, "selct"),
                candidates: names(&["close", "select"]),
            },
        ]
    );
}

/// D5: requiredness belongs to the declaration. `note: string?` must be
/// supplied even though its value may be absent; `subtitle?: string`
/// may be omitted.
#[test]
fn reports_each_missing_required_prop_at_the_tag() {
    let source = r#"<page><card /></page>"#;
    assert_eq!(
        analyze(source).facts(),
        [
            Fact::MissingRequiredProp {
                component: "page".to_string(),
                prop: "title".to_string(),
                span: at(source, "page"),
            },
            Fact::MissingRequiredProp {
                component: "card".to_string(),
                prop: "note".to_string(),
                span: at(source, "card"),
            },
            Fact::MissingRequiredProp {
                component: "card".to_string(),
                prop: "user".to_string(),
                span: at(source, "card"),
            },
        ]
    );
}

#[test]
fn reports_unknown_references_and_commands() {
    let source =
        r#"<page title={titel}><card user={user} note={user.name} on.close={sav()} /></page>"#;
    assert_eq!(
        analyze(source).facts(),
        [
            Fact::UnknownReference {
                name: "titel".to_string(),
                span: at(source, "titel"),
                candidates: names(&["count", "user"]),
            },
            Fact::UnknownCommand {
                command: "sav".to_string(),
                span: at(source, "sav"),
                candidates: names(&["log", "save", "select"]),
            },
        ]
    );
}

#[test]
fn checks_command_arity_and_still_checks_the_arguments() {
    let source = r#"<card user={user} note={user.name} on.select={select($event, usr)} on.close={save(1)} />"#;
    assert_eq!(
        analyze(source).facts(),
        [
            Fact::CommandArityMismatch {
                command: "select".to_string(),
                expected: 1,
                found: 2,
                span: at(source, "select($event, usr)"),
            },
            Fact::UnknownReference {
                name: "usr".to_string(),
                span: at(source, "usr"),
                candidates: names(&["count", "user"]),
            },
            Fact::CommandArityMismatch {
                command: "save".to_string(),
                expected: 0,
                found: 1,
                span: at(source, "save(1)"),
            },
        ]
    );
}

/// D10: a command is valid only as the complete handler expression. A
/// misplaced command is reported once, and nothing inside it is checked.
#[test]
fn a_command_is_valid_only_as_a_whole_handler() {
    for (source, command) in [
        (r#"<page title={save()} />"#, "save()"),
        (r#"<page title="t">{save()}</page>"#, "save()"),
        (r#"<page title="t">{!save()}</page>"#, "save()"),
        (r#"<page title={[nope(usr)]} />"#, "nope(usr)"),
        (
            r#"<card user={user} note={user.name} on.select={select(save())} />"#,
            "save()",
        ),
    ] {
        let name = command.split('(').next().expect("has a name");
        assert_eq!(
            analyze(source).facts(),
            [Fact::CommandOutsideHandler {
                command: name.to_string(),
                span: at(source, command),
            }],
            "{source}"
        );
    }
}

#[test]
fn a_handler_must_be_a_command_and_is_not_looked_inside() {
    let source = r#"<card user={user} note={user.name} on.select={usr.name} on.close={$event} />"#;
    assert_eq!(
        analyze(source).facts(),
        [
            Fact::HandlerNotCommand {
                event: "select".to_string(),
                span: at(source, "usr.name"),
            },
            Fact::HandlerNotCommand {
                event: "close".to_string(),
                span: at(source, "$event"),
            },
        ]
    );
}

/// D10: `$event` exists only inside the arguments of an `on.` handler's
/// command, at any depth.
#[test]
fn event_value_is_valid_only_in_handler_arguments() {
    let valid = r#"<card user={user} note={user.name} on.select={log({ users: [$event] })} />"#;
    assert_eq!(analyze(valid).facts(), []);

    for source in [
        r#"<page title={$event} />"#,
        r#"<page title="t">{$event}</page>"#,
    ] {
        assert_eq!(
            analyze(source).facts(),
            [Fact::EventValueOutsideHandler {
                span: at(source, "$event"),
            }],
            "{source}"
        );
    }
}

#[test]
fn event_value_needs_a_payload() {
    let source = r#"<card user={user} note={user.name} on.close={select($event)} />"#;
    assert_eq!(
        analyze(source).facts(),
        [Fact::EventHasNoPayload {
            component: "card".to_string(),
            event: "close".to_string(),
            span: at(source, "$event"),
        }]
    );
}

/// An unknown event or component is reported once; `$event` in its
/// handler isn't reported again.
#[test]
fn event_value_under_an_unknown_event_is_not_reported_again() {
    let source = r#"<crad on.select={select($event)} />"#;
    assert_eq!(analyze(source).facts().len(), 1);
    let source = r#"<card user={user} note={user.name} on.pick={select($event)} />"#;
    assert_eq!(analyze(source).facts().len(), 1);
}

#[test]
fn only_event_is_a_special_value() {
    let source = r#"<card user={user} note={$evnt} on.select={select($target)} />"#;
    assert_eq!(
        analyze(source).facts(),
        [
            Fact::UnknownSpecialValue {
                name: "evnt".to_string(),
                span: at(source, "$evnt"),
                candidates: names(&["event"]),
            },
            Fact::UnknownSpecialValue {
                name: "target".to_string(),
                span: at(source, "$target"),
                candidates: names(&["event"]),
            },
        ]
    );
}

/// Facts come out in source order even though the walk visits an
/// element's attributes before its event bindings.
#[test]
fn facts_are_in_source_order() {
    let source = r#"<card on.pick={sav()} usr={a} note={b} />"#;
    let starts: Vec<usize> = analyze(source)
        .facts()
        .iter()
        .map(|fact| fact.span().start_byte)
        .collect();
    let mut sorted = starts.clone();
    sorted.sort();
    assert_eq!(starts, sorted);
    assert_eq!(starts.len(), 6, "{:#?}", analyze(source).facts());
}

#[test]
fn resolutions_record_only_names_that_resolved() {
    let source = r#"<crad title={usr} />"#;
    assert_eq!(analyze(source).resolutions(), [] as [Resolution; 0]);
}
