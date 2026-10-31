//! Units (docs/manual/runtime.md, "Units"): state that belongs to one occurrence of a composite and
//! reaches a pure render as an input beside the snapshot. Its lifetime is its node's identity
//! (spec §9.10.6): a reorder keeps it, and a part of the tree that a render keeps is not kept when
//! the state of a unit under it has changed.

mod common;

use common::{codes, compile_with, snapshot};
use mesh_runtime::{
    dispatch, patches_to_json, render, render_units, update_with_units, HostValue, Program, Render,
    Units,
};
use serde_json::{json, Value};

const MODEL: &str = r#"{
  "version": 1, "types": {},
  "components": {
    "box": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "text": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "button": { "props": {}, "events": { "click": {} }, "commands": {}, "scope": {} },
    "mesh-if": { "props": { "when": { "type": { "kind": "boolean" }, "required": true } }, "events": {}, "commands": {}, "scope": {} },
    "mesh-each": { "props": {
        "items": { "type": { "kind": "list", "element": { "kind": "any" } }, "required": true },
        "as":    { "type": { "kind": "string" }, "required": true },
        "key":   { "type": { "kind": "any" }, "required": true } },
      "events": {}, "commands": {}, "scope": {} },
    "disclosure": {
      "props": { "label": { "type": { "kind": "string" }, "required": true } },
      "events": {}, "commands": {}, "scope": { "label": { "kind": "string" } },
      "unit": {
        "state": { "open": { "type": { "kind": "boolean" }, "default": false } },
        "commands": { "toggle": { "parameters": [] } }
      }
    },
    "tabs": {
      "props": { "at": { "type": { "kind": "number" }, "required": false },
                 "count": { "type": { "kind": "number" }, "required": true } },
      "events": { "change": { "payload": { "kind": "number" } } },
      "commands": {},
      "scope": { "count": { "kind": "number" } },
      "unit": {
        "state": { "at": { "type": { "kind": "number" }, "default": 0, "controllable": true } },
        "commands": { "select": { "parameters": [{ "name": "n", "type": { "kind": "number" } }] } }
      }
    },
    "page": { "props": {}, "events": {},
      "commands": { "chose": { "parameters": [{ "name": "n", "type": { "kind": "number" } }] } },
      "scope": {
        "tab": { "kind": "number" },
        "items": { "kind": "list", "element": { "kind": "record", "fields": {
            "id": { "type": { "kind": "number" }, "required": true },
            "label": { "type": { "kind": "string" }, "required": true } } } } } }
  }
}"#;

const DISCLOSURE: &str =
    "<box><button on.click={toggle()}>{label}</button>{#if open}<text>open {label}</text>{/if}</box>";
const TABS: &str = "<box><button on.click={select(at + 1)}>at {at}</button></box>";
const LIST: &str =
    "<box><text>top</text><box>{#each items as item (item.id)}<disclosure label={item.label} />{/each}</box></box>";
const ONE: &str = "<box><text>top</text><box><disclosure label=\"only\" /></box></box>";
const CONTROLLED: &str = "<box><tabs count={3} at={tab} on.change={chose($event)} /></box>";
const UNCONTROLLED: &str = "<box><tabs count={3} on.change={chose($event)} /></box>";
const SNAPSHOT: &str = r#"{ "tab": 2, "items": [
  { "id": 1, "label": "a" }, { "id": 2, "label": "b" }, { "id": 3, "label": "c" } ] }"#;

fn templates(page: &str) -> Vec<String> {
    vec![
        compile_with(MODEL, "page", page),
        compile_with(MODEL, "disclosure", DISCLOSURE),
        compile_with(MODEL, "tabs", TABS),
    ]
}

fn units(text: &str) -> Units {
    Units::from_json(text).expect("the units are JSON")
}

fn made(
    page: &str,
    snapshot_json: &str,
    given: &Units,
) -> Result<(Render, Vec<String>), Vec<mesh_runtime::RuntimeDiagnostic>> {
    let texts = templates(page);
    let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    let render = render_units(
        &Program {
            root: "page",
            templates: &refs,
        },
        MODEL,
        &snapshot(snapshot_json),
        given,
    )?;
    Ok((render, texts))
}

fn ok(page: &str, snapshot_json: &str, given: &Units) -> Render {
    made(page, snapshot_json, given)
        .unwrap_or_else(|d| panic!("renders: {d:#?}"))
        .0
}

fn tree_json(render: &Render) -> Value {
    serde_json::from_str(&render.tree().to_json()).unwrap()
}

/// The key of the occurrence whose `label` is `label`, in a render of `LIST`.
fn occurrence_of(render: &Render, label: &str) -> String {
    let keys: Vec<&str> = render
        .occurrences()
        .map(|instance| instance.key.as_str())
        .collect();
    let labels: Vec<String> = serde_json::from_str::<Value>(&render.tree().to_json()).unwrap()
        ["root"]["children"][1]["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| {
            node["children"][0]["children"][0]["text"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
    let at = labels
        .iter()
        .position(|each| each == label)
        .expect("a row with the label");
    keys[at].to_string()
}

fn texts_of(render: &Render) -> Vec<String> {
    fn walk(value: &Value, out: &mut Vec<String>) {
        if value["type"] == "text" {
            out.push(value["text"].as_str().unwrap().to_string());
        }
        for child in value["children"].as_array().into_iter().flatten() {
            walk(child, out);
        }
    }
    let mut out = Vec::new();
    walk(&tree_json(render)["root"], &mut out);
    out
}

#[test]
fn an_occurrence_with_no_entry_has_the_defaults_and_is_listed() {
    let render = ok(LIST, SNAPSHOT, &Units::default());
    let listed: Vec<_> = render.occurrences().collect();

    assert_eq!(listed.len(), 3);
    assert!(listed
        .iter()
        .all(|instance| instance.component == "disclosure"));
    assert!(listed
        .iter()
        .all(|instance| instance.state["open"] == json!(false)));
    assert!(listed.iter().all(|instance| instance.controlled.is_empty()));
    assert_eq!(texts_of(&render), ["top", "a", "b", "c"]);
}

#[test]
fn the_host_s_state_for_an_occurrence_is_what_the_template_sees() {
    let plain = ok(LIST, SNAPSHOT, &Units::default());
    let b = occurrence_of(&plain, "b");
    let render = ok(
        LIST,
        SNAPSHOT,
        &units(&format!(
            r#"[{{ "occurrence": "{b}", "state": {{ "open": true }} }}]"#
        )),
    );

    assert_eq!(texts_of(&render), ["top", "a", "b", "open b", "c"]);
    assert_eq!(
        render.occurrences().find(|i| i.key == b).unwrap().state["open"],
        json!(true)
    );
}

#[test]
fn a_reorder_keeps_the_state_with_the_row_and_an_unlisted_key_is_ignored() {
    let plain = ok(LIST, SNAPSHOT, &Units::default());
    let b = occurrence_of(&plain, "b");
    let given = units(&format!(
        r#"[{{ "occurrence": "{b}", "state": {{ "open": true }} }}, {{ "occurrence": "k-gone", "state": {{ "open": true }} }}]"#
    ));
    let resorted = r#"{ "tab": 2, "items": [
      { "id": 3, "label": "c" }, { "id": 2, "label": "b" }, { "id": 1, "label": "a" } ] }"#;
    let render = ok(LIST, resorted, &given);

    // The same occurrence, by key, is still open, and now second.
    assert_eq!(texts_of(&render), ["top", "c", "b", "open b", "a"]);
    assert_eq!(occurrence_of(&render, "b"), b);
    let mut keys: Vec<_> = render.occurrences().map(|i| i.key.clone()).collect();
    keys.sort();
    let mut before: Vec<_> = plain.occurrences().map(|i| i.key.clone()).collect();
    before.sort();
    assert_eq!(keys, before);
}

#[test]
fn a_part_the_update_keeps_is_not_kept_when_a_unit_under_it_has_changed() {
    let (before, texts) = made(LIST, SNAPSHOT, &Units::default()).unwrap();
    let b = occurrence_of(&before, "b");
    let given = units(&format!(
        r#"[{{ "occurrence": "{b}", "state": {{ "open": true }} }}]"#
    ));
    // The same snapshot, other units: nothing the snapshot says has changed.
    let updated = update_with_units(&before, snapshot(SNAPSHOT), Some(&given)).unwrap();
    let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    let fresh = render_units(
        &Program {
            root: "page",
            templates: &refs,
        },
        MODEL,
        &snapshot(SNAPSHOT),
        &given,
    )
    .unwrap();

    assert_eq!(tree_json(&updated.render), tree_json(&fresh));
    assert_eq!(texts_of(&updated.render), ["top", "a", "b", "open b", "c"]);
    assert!(
        !updated.patches.is_empty(),
        "{}",
        patches_to_json(&updated.patches)
    );
    // The occurrences are listed again, though most of the tree was kept.
    assert_eq!(updated.render.occurrences().count(), 3);

    // And back to the defaults.
    let back =
        update_with_units(&updated.render, snapshot(SNAPSHOT), Some(&Units::default())).unwrap();

    assert_eq!(texts_of(&back.render), ["top", "a", "b", "c"]);
    // With none given, an update keeps the units the render had.
    let same = update_with_units(&updated.render, snapshot(SNAPSHOT), None).unwrap();

    assert_eq!(texts_of(&same.render), ["top", "a", "b", "open b", "c"]);
    assert!(same.patches.is_empty());
}

#[test]
fn a_command_of_the_unit_names_the_occurrence_it_is_for() {
    for (page, snapshot_json) in [(ONE, SNAPSHOT), (LIST, SNAPSHOT)] {
        let render = ok(page, snapshot_json, &Units::default());
        let intent = dispatch(&render, &common::handler(&render, "click"), None).unwrap();
        let first = render.occurrences().next().unwrap().key.clone();

        assert_eq!(intent.component, "disclosure");
        assert_eq!(intent.command, "toggle");
        assert_eq!(intent.occurrence(), Some(first.as_str()));
        let json: Value = serde_json::from_str(&intent.to_json()).unwrap();
        assert_eq!(json["occurrence"], json!(first));
        assert!(json.get("controlled").is_none());
    }
}

#[test]
fn a_handler_s_arguments_are_evaluated_against_the_state_the_person_saw() {
    let plain = ok(CONTROLLED_FREE, SNAPSHOT, &Units::default());
    let tabs = plain.occurrences().next().unwrap().key.clone();
    let given = units(&format!(
        r#"[{{ "occurrence": "{tabs}", "state": {{ "at": 4 }} }}]"#
    ));
    let render = ok(CONTROLLED_FREE, SNAPSHOT, &given);
    let intent = dispatch(&render, &common::handler(&render, "click"), None).unwrap();

    assert_eq!(intent.command, "select");
    assert_eq!(intent.arguments, [Some(serde_json::json!(5))]);
    assert_eq!(texts_of(&render), ["at 4"]);
}

const CONTROLLED_FREE: &str = "<box><tabs count={3} /></box>";

#[test]
fn a_controllable_field_takes_the_callers_prop_and_the_intent_says_so() {
    let free = ok(UNCONTROLLED, SNAPSHOT, &Units::default());
    let owned = ok(CONTROLLED, SNAPSHOT, &Units::default());
    let key = owned.occurrences().next().unwrap().key.clone();

    // The prop is the value; the host's own state for the occurrence is not used.
    assert_eq!(texts_of(&free), ["at 0"]);
    assert_eq!(texts_of(&owned), ["at 2"]);
    assert_eq!(owned.occurrences().next().unwrap().controlled, ["at"]);
    assert!(free.occurrences().next().unwrap().controlled.is_empty());

    let given = units(&format!(
        r#"[{{ "occurrence": "{key}", "state": {{ "at": 7 }} }}]"#
    ));
    let still = ok(CONTROLLED, SNAPSHOT, &given);

    assert_eq!(texts_of(&still), ["at 2"]);

    let intent = dispatch(&still, &common::handler(&still, "click"), None).unwrap();

    assert_eq!(intent.occurrence(), Some(key.as_str()));
    assert_eq!(intent.controlled, ["at"]);
    let json: Value = serde_json::from_str(&intent.to_json()).unwrap();
    assert_eq!(json["controlled"], json!(["at"]));
}

#[test]
fn a_host_state_the_manifest_forbids_is_refused_at_its_path() {
    let plain = ok(ONE, SNAPSHOT, &Units::default());
    let key = plain.occurrences().next().unwrap().key.clone();
    let texts = templates(ONE);
    let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    let program = Program {
        root: "page",
        templates: &refs,
    };

    for (state, code) in [
        (r#"{ "open": "yes" }"#, "runtime-value-mismatch"),
        (
            r#"{ "open": true, "shut": false }"#,
            "runtime-unknown-field",
        ),
    ] {
        let given = units(&format!(
            r#"[{{ "occurrence": "{key}", "state": {state} }}]"#
        ));
        let refused = render_units(&program, MODEL, &snapshot(SNAPSHOT), &given).unwrap_err();

        assert_eq!(codes(&refused), [code], "{state}");
    }

    // A render without the units is not affected by them.
    assert!(render(&program, MODEL, &snapshot(SNAPSHOT)).is_ok());
    assert!(matches!(
        HostValue::from_json("1").unwrap(),
        HostValue::Number(_)
    ));
}

#[test]
fn units_are_read_from_json_or_refused() {
    assert!(Units::from_json("[]").unwrap().0.is_empty());
    for bad in [
        "{}",
        "[1]",
        r#"[{ "occurrence": 1, "state": {} }]"#,
        r#"[{ "occurrence": "a" }]"#,
        r#"[{ "occurrence": "a", "state": {} }, { "occurrence": "a", "state": {} }]"#,
    ] {
        assert!(Units::from_json(bad).is_err(), "{bad}");
    }
}

#[test]
fn a_unit_raises_an_event_to_the_handler_its_caller_bound() {
    let bound = ok(UNCONTROLLED, SNAPSHOT, &Units::default());
    let key = bound.occurrences().next().unwrap().key.clone();
    let five = HostValue::from_json("5").unwrap();
    let intent = mesh_runtime::raise(&bound, &key, "change", Some(&five))
        .unwrap()
        .expect("the caller bound the event");

    assert_eq!(
        (intent.component.as_str(), intent.command.as_str()),
        ("page", "chose")
    );
    assert_eq!(intent.arguments, [Some(json!(5))]);
    // A command of the page, not of the unit: no occurrence is named.
    assert_eq!(intent.occurrence(), None);

    // The caller bound nothing: nothing to tell.
    let quiet = ok("<box><tabs count={3} /></box>", SNAPSHOT, &Units::default());
    let key = quiet.occurrences().next().unwrap().key.clone();

    assert_eq!(
        mesh_runtime::raise(&quiet, &key, "change", Some(&five)).unwrap(),
        None
    );
}

#[test]
fn raising_what_does_not_exist_is_refused() {
    let render = ok(UNCONTROLLED, SNAPSHOT, &Units::default());
    let key = render.occurrences().next().unwrap().key.clone();
    let five = HostValue::from_json("5").unwrap();
    let text = HostValue::from_json("\"five\"").unwrap();

    assert_eq!(
        codes(&mesh_runtime::raise(&render, "k-nowhere", "change", Some(&five)).unwrap_err()),
        ["runtime-unknown-occurrence"]
    );
    assert_eq!(
        codes(&mesh_runtime::raise(&render, &key, "shout", Some(&five)).unwrap_err()),
        ["runtime-unknown-event"]
    );
    assert_eq!(
        codes(&mesh_runtime::raise(&render, &key, "change", Some(&text)).unwrap_err()),
        ["runtime-value-mismatch"]
    );
    assert_eq!(
        codes(&mesh_runtime::raise(&render, &key, "change", None).unwrap_err()),
        ["runtime-missing-value"]
    );
}

// The old names of the unit occurrence, deprecated in 0.18.0 for one release (refinements C-4).
#[test]
#[allow(deprecated)]
fn the_old_names_still_work_and_say_the_same() {
    let plain = ok(UNCONTROLLED, SNAPSHOT, &Units::default());
    let key = plain.occurrences().next().unwrap().key.clone();

    // The old input name reads as the new one; both at once are refused.
    let old = units(&format!(
        r#"[{{ "instance": "{key}", "state": {{ "at": 4 }} }}]"#
    ));
    let new = units(&format!(
        r#"[{{ "occurrence": "{key}", "state": {{ "at": 4 }} }}]"#
    ));
    assert_eq!(old, new);
    assert!(Units::from_json(&format!(
        r#"[{{ "instance": "{key}", "occurrence": "{key}", "state": {{}} }}]"#
    ))
    .is_err());

    // The old accessors give what the new ones do, and the old field mirrors the new method.
    assert_eq!(plain.instances().count(), plain.occurrences().count());
    assert_eq!(plain.instances_to_json(), plain.occurrences_to_json());
    let _: Option<&mesh_runtime::Instance> = plain.instances().next();
    let five = HostValue::from_json("5").unwrap();
    let intent = mesh_runtime::raise(&plain, &key, "change", Some(&five)).unwrap();
    if let Some(intent) = intent {
        assert_eq!(intent.instance.as_deref(), intent.occurrence());
    }

    // The old diagnostic code is retired: the catalogue keeps it, nothing reports it.
    assert_eq!(
        mesh_runtime::RuntimeCode::UNKNOWN_INSTANCE.as_str(),
        "runtime-unknown-instance"
    );
    let refused = mesh_runtime::raise(&plain, "k-nowhere", "change", Some(&five)).unwrap_err();
    assert_eq!(codes(&refused), ["runtime-unknown-occurrence"]);
}
