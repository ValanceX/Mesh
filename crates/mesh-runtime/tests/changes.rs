//! Changes (docs/manual/runtime.md, "Changes"): `update_changes` applies edits
//! to a render's snapshot instead of taking a whole new one. The law is the
//! update's: the render it makes is the render of the snapshot the edits
//! make. Also: every kind of invalid change fails closed, with nothing done;
//! changes only apply to the render they name; verify mode catches whoever
//! computed them wrongly; and what the edits don't touch is shared.

mod common;

use common::{codes, compile_with, snapshot};
use mesh_runtime::{
    dispatch, evaluations, render, update, update_changes, Change, Changes, HostValue, Location,
    PathSegment, Program, Render, RuntimeDiagnostic, TreeChild,
};
use serde_json::{json, Value};
use std::rc::Rc;

const MODEL: &str = r#"{
  "version": 1, "types": {},
  "components": {
    "page": { "props": { "title": { "type": { "kind": "string" }, "required": true },
                         "count": { "type": { "kind": "number" }, "required": true },
                         "tag":   { "type": { "kind": "optional", "type": { "kind": "string" } }, "required": false } },
              "events": { "tap": {} }, "commands": {}, "scope": {} },
    "note": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "row":  { "props": { "label": { "type": { "kind": "string" }, "required": true },
                         "done":  { "type": { "kind": "boolean" }, "required": true } },
              "events": { "tap": {} }, "commands": {}, "scope": {} },
    "mesh-if": { "props": { "when": { "type": { "kind": "boolean" }, "required": true } }, "events": {}, "commands": {}, "scope": {} },
    "mesh-each": { "props": {
        "items": { "type": { "kind": "list", "element": { "kind": "any" } }, "required": true },
        "as":    { "type": { "kind": "string" }, "required": true },
        "key":   { "type": { "kind": "any" }, "required": true } },
      "events": {}, "commands": {}, "scope": {} },
    "view": { "props": {}, "events": {},
      "commands": { "pick": { "parameters": [ { "name": "id", "type": { "kind": "number" } } ] } },
      "scope": {
        "title": { "kind": "string" },
        "count": { "kind": "number" },
        "flag":  { "kind": "boolean" },
        "tag":   { "kind": "optional", "type": { "kind": "string" } },
        "items": { "kind": "list", "element": { "kind": "record", "fields": {
            "id": { "type": { "kind": "number" }, "required": true },
            "label": { "type": { "kind": "string" }, "required": true },
            "note": { "type": { "kind": "string" }, "required": false },
            "done": { "type": { "kind": "boolean" }, "required": true } } } } } }
  }
}"#;

const VIEW: &str = "<page title={title} count={count + 1} tag={tag}>\
  <note>{title} has {count}</note>\
  <mesh-if when={flag}><note>on</note><note>off</note></mesh-if>\
  <mesh-each items={items} as=\"item\" key={item.id}>\
    <row label={item.label} done={item.done} on.tap={pick(item.id)}>{item.label}</row>\
  </mesh-each>\
</page>";

fn program_render(state: &Value) -> Render {
    let template = compile_with(MODEL, "view", VIEW);
    let templates = [template.as_str()];
    render(
        &Program {
            root: "view",
            templates: &templates,
        },
        MODEL,
        &snapshot(&state.to_string()),
    )
    .unwrap_or_else(|d| panic!("renders: {d:#?}"))
}

fn host(value: &Value) -> HostValue {
    HostValue::from_json(&value.to_string()).expect("JSON")
}

fn name(text: &str) -> PathSegment {
    PathSegment::Name(text.to_string())
}

fn at(index: usize) -> PathSegment {
    PathSegment::Index(index)
}

fn set(path: &[PathSegment], value: Value) -> Change {
    Change::Set {
        path: path.to_vec(),
        value: host(&value),
    }
}

fn insert(path: &[PathSegment], value: Value) -> Change {
    Change::Insert {
        path: path.to_vec(),
        value: host(&value),
    }
}

fn remove(path: &[PathSegment]) -> Change {
    Change::Remove {
        path: path.to_vec(),
    }
}

fn changes(render: &Render, list: Vec<Change>) -> Changes {
    Changes {
        base: render.version(),
        changes: list,
    }
}

fn state() -> Value {
    json!({
        "title": "T", "count": 1, "flag": true, "tag": "g",
        "items": [
            { "id": 1, "label": "a", "done": false },
            { "id": 2, "label": "b", "done": true, "note": "n" },
            { "id": 3, "label": "c", "done": false }
        ]
    })
}

fn failure(result: Result<mesh_runtime::Update, Vec<RuntimeDiagnostic>>) -> Vec<RuntimeDiagnostic> {
    match result {
        Ok(_) => panic!("expected the changes to be refused"),
        Err(diagnostics) => diagnostics,
    }
}

// --- the law -----------------------------------------------------------------

#[test]
fn a_change_makes_the_render_of_the_snapshot_it_makes() {
    let first = program_render(&state());
    let mut expected = state();
    expected["items"][1]["label"] = json!("B!");
    expected["title"] = json!("T2");
    let updated = update_changes(
        &first,
        &changes(
            &first,
            vec![
                set(&[name("items"), at(1), name("label")], json!("B!")),
                set(&[name("title")], json!("T2")),
            ],
        ),
        None,
    )
    .expect("updates");
    assert_eq!(updated.render.tree(), program_render(&expected).tree());
    assert_ne!(updated.render.version(), first.version());
}

#[test]
fn every_edit_has_its_meaning() {
    let cases: Vec<(&str, Vec<Change>, Value)> = vec![
        ("set a scope name", vec![set(&[name("count")], json!(5))], {
            let mut s = state();
            s["count"] = json!(5);
            s
        }),
        (
            "remove an optional scope name",
            vec![remove(&[name("tag")])],
            {
                let mut s = state();
                s.as_object_mut().unwrap().remove("tag");
                s
            },
        ),
        (
            "set an optional record field, then remove it",
            vec![
                set(&[name("items"), at(0), name("note")], json!("hi")),
                remove(&[name("items"), at(1), name("note")]),
            ],
            {
                let mut s = state();
                s["items"][0]["note"] = json!("hi");
                s["items"][1].as_object_mut().unwrap().remove("note");
                s
            },
        ),
        (
            "append with set at the length",
            vec![set(
                &[name("items"), at(3)],
                json!({ "id": 4, "label": "d", "done": false }),
            )],
            {
                let mut s = state();
                s["items"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({ "id": 4, "label": "d", "done": false }));
                s
            },
        ),
        (
            "insert shifts the rest",
            vec![insert(
                &[name("items"), at(1)],
                json!({ "id": 9, "label": "x", "done": false }),
            )],
            {
                let mut s = state();
                s["items"]
                    .as_array_mut()
                    .unwrap()
                    .insert(1, json!({ "id": 9, "label": "x", "done": false }));
                s
            },
        ),
        (
            "remove shifts the rest",
            vec![remove(&[name("items"), at(0)])],
            {
                let mut s = state();
                s["items"].as_array_mut().unwrap().remove(0);
                s
            },
        ),
        (
            "an edit sees the effect of those before it",
            vec![
                insert(
                    &[name("items"), at(0)],
                    json!({ "id": 7, "label": "first", "done": false }),
                ),
                set(&[name("items"), at(0), name("label")], json!("renamed")),
                remove(&[name("items"), at(1)]),
            ],
            {
                let mut s = state();
                let items = s["items"].as_array_mut().unwrap();
                items.insert(0, json!({ "id": 7, "label": "renamed", "done": false }));
                items.remove(1);
                s
            },
        ),
        (
            "replace a whole list",
            vec![set(&[name("items")], json!([]))],
            {
                let mut s = state();
                s["items"] = json!([]);
                s
            },
        ),
    ];
    for (what, edits, expected) in cases {
        let first = program_render(&state());
        let updated = update_changes(&first, &changes(&first, edits), None)
            .unwrap_or_else(|d| panic!("{what}: {d:#?}"));
        assert_eq!(
            updated.render.tree(),
            program_render(&expected).tree(),
            "{what}"
        );
    }
}

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

/// One random edit of `state`, as a change and applied to the JSON `state`.
fn random_edit(rng: &mut Rng, state: &mut Value) -> Change {
    loop {
        let len = state["items"].as_array().unwrap().len();
        let unused = (0..10u64).find(|id| {
            !state["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["id"] == *id)
        });
        match rng.below(11) {
            0 => {
                let value = json!(format!("T{}", rng.below(4)));
                state["title"] = value.clone();
                return set(&[name("title")], value);
            }
            1 => {
                let value = json!(rng.below(5) as f64 - 2.0);
                state["count"] = value.clone();
                return set(&[name("count")], value);
            }
            2 => {
                let value = json!(rng.below(2) == 1);
                state["flag"] = value.clone();
                return set(&[name("flag")], value);
            }
            3 => {
                if state.get("tag").is_some() && rng.below(2) == 0 {
                    state.as_object_mut().unwrap().remove("tag");
                    return remove(&[name("tag")]);
                }
                let value = json!(format!("g{}", rng.below(3)));
                state["tag"] = value.clone();
                return set(&[name("tag")], value);
            }
            4 if len > 0 => {
                let index = rng.below(len as u64) as usize;
                let value = json!(format!("l{}", rng.below(3)));
                state["items"][index]["label"] = value.clone();
                return set(&[name("items"), at(index), name("label")], value);
            }
            5 if len > 0 => {
                let index = rng.below(len as u64) as usize;
                let value = json!(rng.below(2) == 1);
                state["items"][index]["done"] = value.clone();
                return set(&[name("items"), at(index), name("done")], value);
            }
            6 if len > 0 => {
                let index = rng.below(len as u64) as usize;
                if state["items"][index].get("note").is_some() {
                    state["items"][index]
                        .as_object_mut()
                        .unwrap()
                        .remove("note");
                    return remove(&[name("items"), at(index), name("note")]);
                }
                let value = json!("n");
                state["items"][index]["note"] = value.clone();
                return set(&[name("items"), at(index), name("note")], value);
            }
            7 => {
                if let Some(id) = unused {
                    let index = rng.below(len as u64 + 1) as usize;
                    let item = json!({ "id": id, "label": "new", "done": false });
                    state["items"]
                        .as_array_mut()
                        .unwrap()
                        .insert(index, item.clone());
                    return insert(&[name("items"), at(index)], item);
                }
            }
            8 if len > 0 => {
                let index = rng.below(len as u64) as usize;
                state["items"].as_array_mut().unwrap().remove(index);
                return remove(&[name("items"), at(index)]);
            }
            9 => {
                if let Some(id) = unused {
                    let item = json!({ "id": id, "label": "end", "done": true });
                    state["items"].as_array_mut().unwrap().push(item.clone());
                    return set(&[name("items"), at(len)], item);
                }
            }
            10 if len > 1 => {
                // A reorder: a removal and an insertion.
                let from = rng.below(len as u64) as usize;
                let item = state["items"].as_array_mut().unwrap().remove(from);
                let to = rng.below(len as u64) as usize;
                state["items"]
                    .as_array_mut()
                    .unwrap()
                    .insert(to, item.clone());
                // Two changes in one: return the removal; the caller sees both through the state.
                // (The insertion is applied by the next loop turn, so give it as a pair.)
                return Change::Set {
                    path: vec![name("items")],
                    value: host(&state["items"]),
                };
            }
            _ => {}
        }
    }
}

#[test]
fn the_law_holds_over_random_edits() {
    let mut rng = Rng(21);
    let mut checked = 0;
    for _ in 0..40 {
        let mut current_state = state();
        let mut current = program_render(&current_state);
        for _ in 0..25 {
            let mut edits = Vec::new();
            for _ in 0..=rng.below(3) {
                edits.push(random_edit(&mut rng, &mut current_state));
            }
            let updated = update_changes(&current, &changes(&current, edits.clone()), None)
                .unwrap_or_else(|d| panic!("{edits:#?} on {current_state}: {d:#?}"));
            let full = program_render(&current_state);
            assert_eq!(updated.render.tree(), full.tree(), "for {current_state}");
            // The whole-snapshot update agrees too.
            let whole = update(&current, &snapshot(&current_state.to_string())).expect("updates");
            assert_eq!(whole.render.tree(), full.tree());
            // Verify mode, given the whole snapshot, agrees.
            update_changes(
                &current,
                &changes(&current, edits),
                Some(&snapshot(&current_state.to_string())),
            )
            .expect("verifies");
            current = updated.render;
            checked += 1;
        }
    }
    assert_eq!(checked, 1000);
}

// --- sharing and cost --------------------------------------------------------

#[test]
fn what_the_changes_dont_touch_is_the_very_same_node() {
    let items: Vec<Value> = (0..200)
        .map(|id| json!({ "id": id, "label": format!("l{id}"), "done": false }))
        .collect();
    let mut s = state();
    s["items"] = Value::Array(items);
    let first = program_render(&s);
    let updated = update_changes(
        &first,
        &changes(
            &first,
            vec![set(
                &[name("items"), at(100), name("label")],
                json!("changed"),
            )],
        ),
        None,
    )
    .expect("updates");
    let before = &first.tree().root.children;
    let after = &updated.render.tree().root.children;
    assert_eq!(before.len(), after.len());
    let shared = before
        .iter()
        .zip(after)
        .filter(|(a, b)| match (a, b) {
            (TreeChild::Node(a), TreeChild::Node(b)) => Rc::ptr_eq(a, b),
            (TreeChild::Text { text: a, .. }, TreeChild::Text { text: b, .. }) => a == b,
            _ => false,
        })
        .count();
    assert_eq!(shared, before.len() - 1, "all but the changed row");
}

#[test]
fn an_update_by_changes_evaluates_only_what_depends_on_them() {
    let items: Vec<Value> = (0..500)
        .map(|id| json!({ "id": id, "label": format!("l{id}"), "done": false }))
        .collect();
    let mut s = state();
    s["items"] = Value::Array(items);
    let first = program_render(&s);
    let before = evaluations();
    update_changes(
        &first,
        &changes(
            &first,
            vec![set(&[name("items"), at(250), name("label")], json!("x"))],
        ),
        None,
    )
    .expect("updates");
    // `items` once, the changed item's key (2) and its row's label and text (3):
    // nothing for the other 499 rows.
    assert!(
        evaluations() - before < 20,
        "{} evaluations",
        evaluations() - before
    );
}

// --- fail closed -------------------------------------------------------------

#[test]
fn changes_apply_only_to_the_render_they_name() {
    let first = program_render(&state());
    let other = program_render(&state());
    let wrong = Changes {
        base: other.version(),
        changes: vec![set(&[name("count")], json!(2))],
    };
    let diagnostics = failure(update_changes(&first, &wrong, None));
    assert_eq!(codes(&diagnostics), ["runtime-changes-base-mismatch"]);
    // Neither render was touched, and the right base works.
    update_changes(
        &first,
        &changes(&first, vec![set(&[name("count")], json!(2))]),
        None,
    )
    .expect("updates");
    // A render that was updated from is still a base: a second update from it is fine,
    // and the one it made is not.
    let updated = update_changes(&first, &changes(&first, vec![]), None).expect("updates");
    let stale = Changes {
        base: first.version(),
        changes: vec![],
    };
    assert_eq!(
        codes(&failure(update_changes(&updated.render, &stale, None))),
        ["runtime-changes-base-mismatch"]
    );
}

#[test]
fn an_invalid_change_is_a_diagnostic_and_nothing_is_done() {
    let first = program_render(&state());
    let bad: Vec<(&str, Vec<Change>, &str)> = vec![
        (
            "an unknown scope name",
            vec![set(&[name("nope")], json!(1))],
            "runtime-invalid-change",
        ),
        (
            "a field the record type doesn't have",
            vec![set(&[name("items"), at(0), name("extra")], json!(1))],
            "runtime-invalid-change",
        ),
        (
            "an index past the end",
            vec![set(&[name("items"), at(9), name("label")], json!("x"))],
            "runtime-invalid-change",
        ),
        (
            "an append past the length",
            vec![set(
                &[name("items"), at(5)],
                json!({ "id": 4, "label": "d", "done": false }),
            )],
            "runtime-invalid-change",
        ),
        (
            "an insert past the length",
            vec![insert(
                &[name("items"), at(4)],
                json!({ "id": 4, "label": "d", "done": false }),
            )],
            "runtime-invalid-change",
        ),
        (
            "a removal past the end",
            vec![remove(&[name("items"), at(3)])],
            "runtime-invalid-change",
        ),
        (
            "a field name on a list",
            vec![set(&[name("items"), name("label")], json!("x"))],
            "runtime-invalid-change",
        ),
        (
            "an index on a record",
            vec![set(&[name("items"), at(0), at(0)], json!("x"))],
            "runtime-invalid-change",
        ),
        (
            "below a scalar",
            vec![set(&[name("title"), name("x")], json!("x"))],
            "runtime-invalid-change",
        ),
        (
            "removing a required field",
            vec![remove(&[name("items"), at(0), name("label")])],
            "runtime-invalid-change",
        ),
        (
            "removing a required scope name",
            vec![remove(&[name("title")])],
            "runtime-invalid-change",
        ),
        (
            "removing what is already absent",
            vec![remove(&[name("items"), at(0), name("note")])],
            "runtime-invalid-change",
        ),
        (
            "inserting a record field",
            vec![insert(&[name("items"), at(0), name("label")], json!("x"))],
            "runtime-invalid-change",
        ),
        (
            "inserting a scope name",
            vec![insert(&[name("title")], json!("x"))],
            "runtime-invalid-change",
        ),
        (
            "a value of the wrong type",
            vec![set(&[name("count")], json!("three"))],
            "runtime-value-mismatch",
        ),
        (
            "a list element of the wrong type",
            vec![set(&[name("items"), at(0), name("done")], json!(1))],
            "runtime-value-mismatch",
        ),
        (
            "a record missing a required field",
            vec![set(
                &[name("items"), at(0)],
                json!({ "id": 1, "label": "x" }),
            )],
            "runtime-missing-value",
        ),
        (
            "a record with a field it doesn't have",
            vec![set(
                &[name("items"), at(0)],
                json!({ "id": 1, "label": "x", "done": false, "zzz": 1 }),
            )],
            "runtime-unknown-field",
        ),
    ];
    for (what, edits, code) in bad {
        let diagnostics = failure(update_changes(&first, &changes(&first, edits), None));
        assert_eq!(codes(&diagnostics)[0], code, "{what}: {diagnostics:#?}");
        assert!(
            matches!(diagnostics[0].location, Location::Input(_)),
            "{what}"
        );
    }
    // And the render is untouched: it still updates.
    update_changes(
        &first,
        &changes(&first, vec![set(&[name("count")], json!(3))]),
        None,
    )
    .expect("updates");
}

#[test]
fn a_value_that_fails_is_reported_at_its_path_in_the_snapshot() {
    let first = program_render(&state());
    let diagnostics = failure(update_changes(
        &first,
        &changes(
            &first,
            vec![set(&[name("items"), at(1), name("done")], json!("yes"))],
        ),
        None,
    ));
    assert_eq!(
        diagnostics[0].location,
        Location::Input(vec![name("items"), at(1), name("done")])
    );
}

#[test]
fn a_failing_change_leaves_every_change_before_it_undone() {
    let first = program_render(&state());
    let diagnostics = failure(update_changes(
        &first,
        &changes(
            &first,
            vec![
                set(&[name("count")], json!(99)),
                set(&[name("title")], json!(false)),
            ],
        ),
        None,
    ));
    assert_eq!(codes(&diagnostics), ["runtime-value-mismatch"]);
    // `first` still has count 1: updating it with something unrelated shows it.
    let updated = update_changes(
        &first,
        &changes(&first, vec![set(&[name("title")], json!("U"))]),
        None,
    )
    .expect("updates");
    assert_eq!(
        updated.render.tree(),
        {
            let mut s = state();
            s["title"] = json!("U");
            program_render(&s)
        }
        .tree()
    );
}

// --- verify ------------------------------------------------------------------

#[test]
fn verify_mode_catches_changes_that_make_the_wrong_snapshot() {
    let first = program_render(&state());
    // The host believes it set the title; it computed the wrong change.
    let mut believed = state();
    believed["title"] = json!("right");
    let wrong = changes(&first, vec![set(&[name("title")], json!("wrong"))]);
    let diagnostics = failure(update_changes(
        &first,
        &wrong,
        Some(&snapshot(&believed.to_string())),
    ));
    assert_eq!(codes(&diagnostics), ["runtime-changes-disagree"]);
    assert_eq!(
        diagnostics[0].location,
        Location::Input(vec![name("title")])
    );
    // Without verify mode the wrong change is applied: that is what verify is for.
    update_changes(&first, &wrong, None).expect("updates");
    // A difference deep inside, and a change that does less than the host thinks.
    let mut deep = state();
    deep["items"][2]["label"] = json!("D");
    let diagnostics = failure(update_changes(
        &first,
        &changes(&first, vec![]),
        Some(&snapshot(&deep.to_string())),
    ));
    assert_eq!(
        diagnostics[0].location,
        Location::Input(vec![name("items"), at(2), name("label")])
    );
    // And a whole snapshot that is itself invalid is refused with the usual diagnostics.
    let diagnostics = failure(update_changes(
        &first,
        &changes(&first, vec![]),
        Some(&snapshot(r#"{"title":1}"#)),
    ));
    assert!(codes(&diagnostics)
        .iter()
        .all(|c| c.starts_with("runtime-")));
}

#[test]
fn verify_mode_distinguishes_zero_from_negative_zero() {
    let first = program_render(&state());
    let negative = HostValue::Number(-0.0);
    let edit = Change::Set {
        path: vec![name("count")],
        value: negative,
    };
    let positive = snapshot(
        &{
            let mut s = state();
            s["count"] = json!(0);
            s
        }
        .to_string(),
    );
    let diagnostics = failure(update_changes(
        &first,
        &changes(&first, vec![edit]),
        Some(&positive),
    ));
    assert_eq!(codes(&diagnostics), ["runtime-changes-disagree"]);
}

// --- the document ------------------------------------------------------------

fn parse(text: &str) -> Result<Changes, Vec<RuntimeDiagnostic>> {
    Changes::from_host(&HostValue::from_json(text).expect("JSON"))
}

#[test]
fn the_changes_document_is_read_as_described() {
    let read = parse(
        r#"{"base":4,"changes":[
            {"op":"set","path":["items",1,"label"],"value":"x"},
            {"op":"insert","path":["items",0],"value":{"id":1}},
            {"op":"remove","path":["tag"]}]}"#,
    )
    .expect("reads");
    assert_eq!(read.base, 4);
    assert_eq!(read.changes.len(), 3);
    assert!(
        matches!(&read.changes[0], Change::Set { path, .. } if *path == [name("items"), at(1), name("label")])
    );
    assert!(matches!(&read.changes[2], Change::Remove { path } if *path == [name("tag")]));
}

#[test]
fn a_malformed_changes_document_is_refused_with_where() {
    let bad = [
        ("not a record", r#"[]"#),
        ("no base", r#"{"changes":[]}"#),
        ("a negative base", r#"{"base":-1,"changes":[]}"#),
        ("a fractional base", r#"{"base":1.5,"changes":[]}"#),
        ("no changes", r#"{"base":1}"#),
        (
            "an edit that is not a record",
            r#"{"base":1,"changes":[1]}"#,
        ),
        (
            "an unknown op",
            r#"{"base":1,"changes":[{"op":"move","path":["a"]}]}"#,
        ),
        ("no path", r#"{"base":1,"changes":[{"op":"remove"}]}"#),
        (
            "an empty path",
            r#"{"base":1,"changes":[{"op":"remove","path":[]}]}"#,
        ),
        (
            "a path segment that is neither",
            r#"{"base":1,"changes":[{"op":"remove","path":[true]}]}"#,
        ),
        (
            "a negative index",
            r#"{"base":1,"changes":[{"op":"remove","path":["a",-1]}]}"#,
        ),
        (
            "a fractional index",
            r#"{"base":1,"changes":[{"op":"remove","path":["a",0.5]}]}"#,
        ),
        (
            "set without a value",
            r#"{"base":1,"changes":[{"op":"set","path":["a"]}]}"#,
        ),
        (
            "insert without a value",
            r#"{"base":1,"changes":[{"op":"insert","path":["a",0]}]}"#,
        ),
        (
            "remove with a value",
            r#"{"base":1,"changes":[{"op":"remove","path":["a"],"value":1}]}"#,
        ),
    ];
    for (what, text) in bad {
        let diagnostics = parse(text).expect_err(what);
        assert_eq!(codes(&diagnostics), ["runtime-invalid-change"], "{what}");
    }
    // Where: the second edit's op.
    let diagnostics =
        parse(r#"{"base":1,"changes":[{"op":"remove","path":["a"]},{"op":"zap","path":["a"]}]}"#)
            .expect_err("zap");
    assert_eq!(
        diagnostics[0].location,
        Location::Input(vec![name("changes"), at(1), name("op")])
    );
}

// --- what a render made from changes can do ----------------------------------

#[test]
fn a_render_made_from_changes_dispatches_against_its_own_snapshot() {
    let first = program_render(&state());
    let updated = update_changes(
        &first,
        &changes(
            &first,
            vec![set(&[name("items"), at(1), name("id")], json!(22))],
        ),
        None,
    )
    .expect("updates");
    // The row for the item whose id changed: its handler is by key, so the same
    // identifier as before, and its intent has the id the changes gave.
    let handlers: Vec<String> = updated
        .render
        .tree()
        .root
        .children
        .iter()
        .filter_map(|child| match child {
            TreeChild::Node(node) if node.component == "row" => node.events.get("tap").cloned(),
            _ => None,
        })
        .collect();
    assert_eq!(handlers.len(), 3);
    let intent = dispatch(&updated.render, &handlers[1], None::<&HostValue>).expect("dispatches");
    let json: Value = serde_json::from_str(&intent.to_json()).unwrap();
    assert_eq!(json["arguments"][0]["value"], 22);
    // And the previous render still gives the id it had.
    let before = dispatch(
        &first,
        &first
            .tree()
            .root
            .children
            .iter()
            .filter_map(|child| match child {
                TreeChild::Node(node) if node.component == "row" => node.events.get("tap").cloned(),
                _ => None,
            })
            .nth(1)
            .unwrap(),
        None::<&HostValue>,
    )
    .expect("dispatches");
    let json: Value = serde_json::from_str(&before.to_json()).unwrap();
    assert_eq!(json["arguments"][0]["value"], 2);
}

#[test]
fn a_render_made_from_changes_can_be_updated_again_by_either_form() {
    let first = program_render(&state());
    let second = update_changes(
        &first,
        &changes(&first, vec![set(&[name("count")], json!(7))]),
        None,
    )
    .expect("updates");
    let mut expected = state();
    expected["count"] = json!(7);
    let by_whole = update(
        &second.render,
        &snapshot(
            &{
                let mut s = expected.clone();
                s["title"] = json!("W");
                s
            }
            .to_string(),
        ),
    )
    .expect("updates");
    expected["title"] = json!("W");
    assert_eq!(by_whole.render.tree(), program_render(&expected).tree());
    let by_changes = update_changes(
        &second.render,
        &changes(&second.render, vec![set(&[name("title")], json!("C"))]),
        None,
    )
    .expect("updates");
    expected["title"] = json!("C");
    assert_eq!(by_changes.render.tree(), program_render(&expected).tree());
}
