//! Inferred composite contracts: the manifest declares the root and the primitives, and the program's occurrences and handlers state the rest.

use mesh_compiler::infer::infer_components;
use mesh_compiler::{compile_with, CompileOptions};
use serde_json::Value;

const MODEL: &str = r#"{
  "version": 1, "types": {},
  "components": {
    "panel": { "props": {}, "events": { "tap": {} }, "commands": {}, "scope": {} },
    "note":  { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "button": { "props": {}, "events": { "tap": {} }, "commands": {}, "scope": {} },
    "mesh-if": { "props": { "when": { "type": { "kind": "boolean" }, "required": true } }, "events": {}, "commands": {}, "scope": {} },
    "mesh-each": { "props": {
        "items": { "type": { "kind": "list", "element": { "kind": "any" } }, "required": true },
        "as":    { "type": { "kind": "string" }, "required": true },
        "key":   { "type": { "kind": "any" }, "required": true } },
      "events": {}, "commands": {}, "scope": {} },
    "view": { "props": {}, "events": {},
      "commands": { "pick": { "parameters": [ { "name": "id", "type": { "kind": "number" } } ] } },
      "scope": {
        "title": { "kind": "string" }, "flag": { "kind": "boolean" },
        "items": { "kind": "list", "element": { "kind": "record", "fields": {
            "id": { "type": { "kind": "number" }, "required": true },
            "label": { "type": { "kind": "string" }, "required": true } } } } } }
  }
}"#;

const VIEW: &str = "<panel><card heading={title}><note>hi</note></card><mesh-each items={items} as=\"item\" key={item.id}><row id={item.id} label={item.label} on.chosen={pick($event)} /></mesh-each></panel>";
const CARD: &str = "<panel><note>{heading}</note><mesh-slot /></panel>";
const ROW: &str = "<panel><button on.tap={chosen(id)}>{label}</button><mesh-if when={flag}><note>x</note></mesh-if></panel>";

fn sources() -> Vec<(String, String)> {
    vec![
        ("view".into(), VIEW.into()),
        ("card".into(), CARD.into()),
        ("row".into(), ROW.into()),
    ]
}

#[test]
fn props_come_from_occurrences_events_from_handlers_and_the_slot_is_declared() {
    let inferred = infer_components(MODEL, "view", &sources()).expect("something was inferred");
    let document: Value = serde_json::from_str(&inferred).unwrap();
    let components = &document["components"];

    assert_eq!(
        components["card"]["props"]["heading"],
        serde_json::json!({ "type": { "kind": "string" }, "required": true })
    );
    assert_eq!(
        components["card"]["scope"]["heading"],
        serde_json::json!({ "kind": "string" })
    );
    assert_eq!(
        components["row"]["props"]["id"]["type"],
        serde_json::json!({ "kind": "number" })
    );
    assert_eq!(
        components["row"]["events"]["chosen"],
        serde_json::json!({ "payload": { "kind": "number" } })
    );
    assert!(components["mesh-slot"].is_object());
}

#[test]
fn the_inferred_manifest_checks_every_template() {
    let mut sources = sources();
    // `flag` is read by `row` but nobody passes it: that is an ordinary error, reported by the compile, not by inference.
    let inferred = infer_components(MODEL, "view", &sources).unwrap();
    let manifest = mesh_manifest::load(&inferred).unwrap();
    let diagnostics = |name: &str, source: &str| {
        compile_with(
            source,
            &CompileOptions::with_template(manifest.template(name).unwrap()),
        )
        .diagnostics
    };

    assert!(diagnostics("view", VIEW).is_empty());
    assert!(diagnostics("card", CARD).is_empty());
    assert!(diagnostics("row", ROW)
        .iter()
        .any(|d| d.message.contains("flag")));

    sources[2].1 = ROW.replace("<mesh-if when={flag}><note>x</note></mesh-if>", "");
    let fixed = infer_components(MODEL, "view", &sources).unwrap();
    let manifest = mesh_manifest::load(&fixed).unwrap();

    assert!(compile_with(
        &sources[2].1,
        &CompileOptions::with_template(manifest.template("row").unwrap())
    )
    .diagnostics
    .is_empty());
}

#[test]
fn a_declaration_is_never_touched() {
    let declared = MODEL.replace("\"view\":", "\"card\": { \"props\": { \"heading\": { \"type\": { \"kind\": \"any\" }, \"required\": true } }, \"events\": {}, \"commands\": {}, \"scope\": { \"heading\": { \"kind\": \"any\" } } }, \"view\":");
    let inferred = infer_components(&declared, "view", &sources()).unwrap();
    let document: Value = serde_json::from_str(&inferred).unwrap();

    assert_eq!(
        document["components"]["card"]["props"]["heading"]["type"],
        serde_json::json!({ "kind": "any" })
    );
    assert!(document["components"]["row"].is_object());
}

#[test]
fn an_optional_prop_is_one_some_occurrences_leave_out() {
    let sources = vec![
        (
            "view".to_string(),
            "<panel><card heading={title} /><card /></panel>".to_string(),
        ),
        (
            "card".to_string(),
            "<panel><mesh-if when={flag}><note>x</note></mesh-if></panel>".to_string(),
        ),
    ];
    let inferred = infer_components(MODEL, "view", &sources).unwrap();
    let document: Value = serde_json::from_str(&inferred).unwrap();

    assert_eq!(
        document["components"]["card"]["props"]["heading"],
        serde_json::json!({ "type": { "kind": "optional", "type": { "kind": "string" } }, "required": false })
    );
}
