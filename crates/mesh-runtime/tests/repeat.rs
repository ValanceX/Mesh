//! The §9.10 repeated-identity tracer: can one program produce a render
//! tree with one node per item, each identified by the key its author
//! declared, while identity keeps the contract's rules, and can dispatch
//! find the right item again from the identifier alone, without a rendered
//! position or the application's object?
//!
//! PROVISIONAL. The repeat here is
//! `<mesh-each items={…} as="item" key={item.id}>child</mesh-each>`, an
//! ordinary element of a component the model declares, as `mesh-if` is for
//! conditionals. The compiler binds `item` for `key` and the child; the
//! runtime, not the compiler, gives the element meaning, evaluates `key` per
//! item, and never makes it a node. Its spelling is not the language's
//! decision. What these tests pin is the *semantics*: identity is the
//! declared key and not the index, a key must be a string or a finite number
//! and unique in its repeat, and a handler's identity is its node's and its
//! event.

mod common;

use common::{codes, compile_with, snapshot};
use mesh_runtime::{dispatch, Node, Program, Render, RuntimeDiagnostic, TreeChild};
use std::collections::BTreeMap;

const MODEL: &str = r#"{
  "version": 1, "types": {},
  "components": {
    "page": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "note": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "row":  { "props": {}, "events": { "tap": {}, "hold": {} }, "commands": {}, "scope": {} },
    "mesh-each": { "props": {
        "items": { "type": { "kind": "list", "element": { "kind": "any" } }, "required": true },
        "as":    { "type": { "kind": "string" }, "required": true },
        "key":   { "type": { "kind": "any" }, "required": true } },
      "events": {}, "commands": {}, "scope": {} },
    "view": { "props": {}, "events": {},
      "commands": { "pick": { "parameters": [ { "name": "id", "type": { "kind": "any" } } ] },
                    "name": { "parameters": [ { "name": "label", "type": { "kind": "string" } } ] } },
      "scope": {
        "items": { "kind": "list", "element": { "kind": "record", "fields": {
            "id": { "type": { "kind": "any" }, "required": true },
            "label": { "type": { "kind": "string" }, "required": true } } } },
        "others": { "kind": "list", "element": { "kind": "record", "fields": {
            "id": { "type": { "kind": "any" }, "required": true },
            "label": { "type": { "kind": "string" }, "required": true } } } } } },
    "typed": { "props": {}, "events": {}, "commands": {},
      "scope": { "rows": { "kind": "list", "element": { "kind": "record", "fields": {
        "s": { "type": { "kind": "string" }, "required": true },
        "n": { "type": { "kind": "number" }, "required": true },
        "b": { "type": { "kind": "boolean" }, "required": true },
        "z": { "type": { "kind": "null" }, "required": true },
        "r": { "type": { "kind": "record", "fields": { "x": { "type": { "kind": "number" }, "required": true } } }, "required": true },
        "l": { "type": { "kind": "list", "element": { "kind": "number" } }, "required": true },
        "o": { "type": { "kind": "optional", "type": { "kind": "string" } }, "required": false },
        "p": { "type": { "kind": "optional", "type": { "kind": "number" } }, "required": false },
        "a": { "type": { "kind": "any" }, "required": true } } } } } },
    "bad": { "props": {}, "events": {}, "commands": {},
      "scope": { "items": { "kind": "list", "element": { "kind": "any" } } } }
  }
}"#;

/// A repeat between two static siblings; each item is a node with two
/// handlers whose arguments are the item's.
const VIEW: &str = "<page>\
  <note>head</note>\
  <mesh-each items={items} as=\"item\" key={item.id}>\
    <row on.tap={pick(item.id)} on.hold={name(item.label)}>{item.label}</row>\
  </mesh-each>\
  <note>tail</note>\
</page>";

/// Two repeats, so that different sites can be given equal keys.
const TWO: &str = "<page>\
  <mesh-each items={items} as=\"item\" key={item.id}><row on.tap={pick(item.id)}>{item.label}</row></mesh-each>\
  <mesh-each items={others} as=\"other\" key={other.id}><row on.tap={pick(other.id)}>{other.label}</row></mesh-each>\
</page>";

fn items(list: &[(&str, &str)]) -> String {
    let rows: Vec<String> = list
        .iter()
        .map(|(id, label)| format!(r#"{{"id":{id},"label":"{label}"}}"#))
        .collect();
    format!("[{}]", rows.join(","))
}

fn try_render(root: &str, source: &str, values: &str) -> Result<Render, Vec<RuntimeDiagnostic>> {
    let template = compile_with(MODEL, root, source);
    let templates = [template.as_str()];
    mesh_runtime::render(
        &Program {
            root,
            templates: &templates,
        },
        MODEL,
        &snapshot(values),
    )
}

/// `VIEW` rendered with `list` as `items`: `(id literal, label)` pairs.
fn view(list: &[(&str, &str)]) -> Render {
    let values = format!(r#"{{"items":{},"others":[]}}"#, items(list));
    try_render("view", VIEW, &values).unwrap_or_else(|d| panic!("renders: {d:#?}"))
}

fn two(list: &[(&str, &str)], others: &[(&str, &str)]) -> Render {
    let values = format!(r#"{{"items":{},"others":{}}}"#, items(list), items(others));
    try_render("view", TWO, &values).unwrap_or_else(|d| panic!("renders: {d:#?}"))
}

#[derive(Debug, Clone)]
struct Part {
    key: String,
    component: String,
    text: String,
    rendered_index: usize,
    events: BTreeMap<String, String>,
}

fn parts(render: &Render) -> Vec<Part> {
    fn go(node: &Node, index: usize, out: &mut Vec<Part>) {
        let text = node
            .children
            .iter()
            .filter_map(|child| match child {
                TreeChild::Text { text, .. } => Some(text.as_str()),
                TreeChild::Node(_) => None,
            })
            .collect::<String>();
        out.push(Part {
            key: node.key.clone(),
            component: node.component.clone(),
            text,
            rendered_index: index,
            events: node.events.clone(),
        });
        for (i, child) in node.children.iter().enumerate() {
            if let TreeChild::Node(child) = child {
                go(child, i, out);
            }
        }
    }
    let mut out = Vec::new();
    go(&render.tree().root, 0, &mut out);
    out
}

/// The row showing `label`, if the render has one.
fn row(render: &Render, label: &str) -> Option<Part> {
    parts(render)
        .into_iter()
        .find(|part| part.component == "row" && part.text == label)
}

fn must(render: &Render, label: &str) -> Part {
    row(render, label).unwrap_or_else(|| panic!("no row `{label}`"))
}

fn note(render: &Render, text: &str) -> Part {
    parts(render)
        .into_iter()
        .find(|part| part.component == "note" && part.text == text)
        .unwrap_or_else(|| panic!("no note `{text}`"))
}

fn keys(render: &Render) -> Vec<String> {
    parts(render).into_iter().map(|part| part.key).collect()
}

fn refused(list: &[(&str, &str)]) -> Vec<&'static str> {
    let values = format!(r#"{{"items":{},"others":[]}}"#, items(list));
    match try_render("view", VIEW, &values) {
        Ok(_) => panic!("{list:?} should be refused"),
        Err(diagnostics) => codes(&diagnostics),
    }
}

fn intent(render: &Render, part: &Part, event: &str) -> String {
    let intent = dispatch(render, &part.events[event], None).expect("dispatches");
    format!("{}({:?})", intent.command, intent.arguments)
}

const ABC: [(&str, &str); 3] = [("\"a\"", "A"), ("\"b\"", "B"), ("\"c\"", "C")];

// --- R1: the same input, the same identity ------------------------------------

#[test]
fn r1_the_same_snapshot_gives_the_same_identities() {
    let (one, two) = (view(&ABC), view(&ABC));
    assert_eq!(keys(&one), keys(&two));
    for label in ["A", "B", "C"] {
        assert_eq!(must(&one, label).events, must(&two, label).events);
    }
    // One node per item, and every key distinct.
    let all = keys(&one);
    let mut sorted = all.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(all.len(), sorted.len());
    assert_eq!(
        parts(&one).iter().filter(|p| p.component == "row").count(),
        3
    );
}

// --- R2: reordering moves nodes, not identities -------------------------------

#[test]
fn r2_reordering_changes_the_index_and_not_the_identity() {
    let before = view(&ABC);
    let after = view(&[("\"c\"", "C"), ("\"a\"", "A"), ("\"b\"", "B")]);
    for label in ["A", "B", "C"] {
        let (b, a) = (must(&before, label), must(&after, label));
        assert_eq!(b.key, a.key, "{label}: same item, same key");
        assert_eq!(b.events, a.events, "{label}: same handlers");
    }
    // The rendered indices did change: that is what is not identity.
    assert_ne!(
        must(&before, "C").rendered_index,
        must(&after, "C").rendered_index
    );
    // The statics around the repeat are untouched.
    assert_eq!(note(&before, "head").key, note(&after, "head").key);
    assert_eq!(note(&before, "tail").key, note(&after, "tail").key);
}

// --- R3: an insertion disturbs nobody -----------------------------------------

#[test]
fn r3_an_insertion_leaves_every_other_identity_alone() {
    let before = view(&[("\"a\"", "A"), ("\"c\"", "C")]);
    let after = view(&ABC);
    for label in ["A", "C"] {
        assert_eq!(must(&before, label).key, must(&after, label).key);
        assert_eq!(must(&before, label).events, must(&after, label).events);
    }
    assert!(row(&before, "B").is_none());
    assert!(row(&after, "B").is_some());
    // C's index moved (1 → 2 in the rows, 2 → 3 among the children); its key didn't.
    assert_ne!(
        must(&before, "C").rendered_index,
        must(&after, "C").rendered_index
    );
    assert_eq!(note(&before, "tail").key, note(&after, "tail").key);
}

// --- R4: a removal ------------------------------------------------------------

#[test]
fn r4_a_removed_item_is_gone_and_the_rest_keep_their_identity() {
    let before = view(&ABC);
    let after = view(&[("\"a\"", "A"), ("\"c\"", "C")]);
    assert!(row(&after, "B").is_none());
    for label in ["A", "C"] {
        assert_eq!(must(&before, label).key, must(&after, label).key);
    }
    // And its identifiers are not found against the render it is not in.
    let gone = must(&before, "B");
    let found = dispatch(&after, &gone.events["tap"], None);
    assert_eq!(codes(&found.unwrap_err()), ["runtime-unknown-handler"]);
}

// --- R5: duplicate keys fail closed -------------------------------------------

#[test]
fn r5_a_duplicate_key_is_refused_and_nothing_is_rendered() {
    assert_eq!(
        refused(&[("\"a\"", "A"), ("\"b\"", "B"), ("\"a\"", "again")]),
        ["runtime-duplicate-key"]
    );
    // The same duplicate in numbers.
    assert_eq!(
        refused(&[("1", "A"), ("2", "B"), ("1", "again")]),
        ["runtime-duplicate-key"]
    );
}

// --- R6: invalid or missing keys fail closed ----------------------------------

#[test]
fn r6_a_key_that_is_not_a_string_or_a_finite_number_is_refused() {
    for id in ["null", "true", "false", "[1]", "{}", "{\"x\":1}"] {
        assert_eq!(refused(&[(id, "A")]), ["runtime-invalid-key"], "key {id}");
    }
    // An absent key can't reach the runtime from a program that compiles: the
    // checker refuses a key that may be absent (D1), and a missing field of
    // an `any` is `runtime-missing-member`. The runtime's check of it stays
    // as the backstop.
    // An invalid key among valid ones refuses the whole render.
    assert_eq!(
        refused(&[("\"a\"", "A"), ("null", "B")]),
        ["runtime-invalid-key"]
    );
}

// --- R7: a string and a number are different keys -----------------------------

#[test]
fn r7_the_string_one_and_the_number_one_are_different_keys() {
    let render = view(&[("\"1\"", "S"), ("1", "N")]);
    assert_ne!(must(&render, "S").key, must(&render, "N").key);
    // And each is the key it is, independent of the other being there.
    let alone_string = view(&[("\"1\"", "S")]);
    let alone_number = view(&[("1", "N")]);
    assert_eq!(must(&render, "S").key, must(&alone_string, "S").key);
    assert_eq!(must(&render, "N").key, must(&alone_number, "N").key);
    // Numbers are equal when their values are: `1` and `1.0` are one key.
    assert_eq!(
        refused(&[("1", "A"), ("1.0", "B")]),
        ["runtime-duplicate-key"]
    );
    // A string that looks like the encoding of a number is still a string.
    let tricky = view(&[("\"n:1\"", "T"), ("1", "N")]);
    assert_ne!(must(&tricky, "T").key, must(&tricky, "N").key);
}

// --- R8: zero and negative zero are one key -----------------------------------

#[test]
fn r8_zero_and_negative_zero_are_the_same_key() {
    assert_eq!(
        refused(&[("0", "A"), ("-0.0", "B")]),
        ["runtime-duplicate-key"]
    );
    let zero = view(&[("0", "Z")]);
    let negative = view(&[("-0.0", "Z")]);
    assert_eq!(must(&zero, "Z").key, must(&negative, "Z").key);
}

// --- R9: equal keys at different sites don't collide --------------------------

#[test]
fn r9_the_same_key_at_two_repeated_sites_is_two_identities() {
    let render = two(&[("\"x\"", "first")], &[("\"x\"", "second")]);
    let (first, second) = (must(&render, "first"), must(&render, "second"));
    assert_ne!(first.key, second.key);
    assert_ne!(first.events["tap"], second.events["tap"]);
    // Neither depends on the other site's items.
    let alone = two(&[("\"x\"", "first")], &[]);
    assert_eq!(must(&alone, "first").key, first.key);
    // Each routes to its own item.
    assert_eq!(
        intent(&render, &first, "tap"),
        r#"pick([Some(String("x"))])"#
    );
    assert_eq!(
        intent(&render, &second, "tap"),
        intent(&render, &first, "tap")
    );
}

// --- R10: reappearance --------------------------------------------------------

#[test]
fn r10_an_item_that_comes_back_has_the_identity_it_had() {
    let both = view(&[("\"a\"", "A"), ("\"b\"", "B")]);
    let without = view(&[("\"a\"", "A")]);
    let again = view(&[("\"a\"", "A"), ("\"b\"", "B")]);
    assert!(row(&without, "B").is_none());
    assert_eq!(must(&both, "B").key, must(&again, "B").key);
    assert_eq!(must(&both, "B").events, must(&again, "B").events);
    // The absence is a render of its own: nothing carries B through it.
    assert!(!keys(&without).contains(&must(&both, "B").key));
}

// --- R11: a handler is its node's identity and its event ----------------------

#[test]
fn r11_handlers_are_the_items_identity_and_the_event() {
    let render = view(&ABC);
    let (a, b) = (must(&render, "A"), must(&render, "B"));
    assert_ne!(a.events["tap"], b.events["tap"]);
    assert_ne!(a.events["tap"], a.events["hold"]);
    // The same item has the same handlers in a reordered render.
    let moved = view(&[("\"b\"", "B"), ("\"c\"", "C"), ("\"a\"", "A")]);
    assert_eq!(must(&moved, "B").events, b.events);
}

// --- Dispatch: the item again, without a position or an object ----------------

#[test]
fn dispatch_finds_the_item_from_the_identifier_and_the_snapshot() {
    let render = view(&ABC);
    let (a, b, c) = (must(&render, "A"), must(&render, "B"), must(&render, "C"));
    assert_eq!(intent(&render, &a, "tap"), intent(&render, &a, "tap"));
    let tap = |part: &Part| intent(&render, part, "tap");
    assert_ne!(tap(&a), tap(&b));
    assert_ne!(tap(&b), tap(&c));
    // The arguments are the item's own: its id for `tap`, its label for `hold`.
    let hold = dispatch(&render, &b.events["hold"], None).unwrap();
    assert_eq!(hold.command, "name");
    assert_eq!(format!("{:?}", hold.arguments), r#"[Some(String("B"))]"#);
    let pick = dispatch(&render, &b.events["tap"], None).unwrap();
    assert_eq!(pick.command, "pick");
    assert_eq!(format!("{:?}", pick.arguments), r#"[Some(String("b"))]"#);
}

#[test]
fn dispatch_follows_the_item_when_the_list_has_been_reordered() {
    // An identifier from the render before, against the snapshot after: the
    // item is found by its identity, wherever it now is.
    let before = view(&ABC);
    let c_handler = must(&before, "C").events["tap"].clone();
    let after = view(&[("\"c\"", "C"), ("\"a\"", "A"), ("\"b\"", "B")]);
    let by_old = dispatch(&after, &c_handler, None).unwrap();
    assert_eq!(format!("{:?}", by_old.arguments), r#"[Some(String("c"))]"#);
    // And a handler the snapshot no longer has is not guessed at by position.
    let b_handler = must(&before, "B").events["tap"].clone();
    let without_b = view(&[("\"a\"", "A"), ("\"c\"", "C")]);
    assert_eq!(
        codes(&dispatch(&without_b, &b_handler, None).unwrap_err()),
        ["runtime-unknown-handler"]
    );
}

#[test]
fn dispatch_evaluates_an_item_with_its_numeric_key_in_its_own_scope() {
    let render = view(&[("7", "seven"), ("8", "eight")]);
    let eight = must(&render, "eight");
    let pick = dispatch(&render, &eight.events["tap"], None).unwrap();
    assert_eq!(format!("{:?}", pick.arguments), "[Some(Number(8))]");
}

#[test]
fn dispatch_against_a_snapshot_with_a_bad_key_fails_closed() {
    let good = view(&ABC);
    let handler = must(&good, "A").events["tap"].clone();
    let values = format!(
        r#"{{"items":{},"others":[]}}"#,
        items(&[("\"a\"", "A"), ("\"a\"", "dup")])
    );
    let template = compile_with(MODEL, "view", VIEW);
    let templates = [template.as_str()];
    let program = Program {
        root: "view",
        templates: &templates,
    };
    let found = mesh_runtime::dispatch_from(&program, MODEL, &snapshot(&values), &handler, None);
    assert_eq!(codes(&found.unwrap_err()), ["runtime-duplicate-key"]);
}

/// D1: the checker requires a repeat's key to be a string or a number, where
/// types can say so, whatever the model declares for the `key` prop. `any` is
/// accepted as it is everywhere (the runtime checks the value); the runtime's
/// own check stays, and the tests above exercise it.
#[test]
fn the_checker_requires_a_key_that_is_a_string_or_a_number() {
    let model = mesh_compiler::check::Model::load(MODEL, "typed").unwrap();
    let key = |expression: &str| {
        let source = format!(
            "<page><mesh-each items={{rows}} as=\"r\" key={{{expression}}}><note>x</note></mesh-each></page>"
        );
        mesh_compiler::check::template(&source, &model)
    };
    for accepted in ["r.s", "r.n", "r.a", "\"literal\"", "7", "r.n + 1"] {
        let compiled = key(accepted);
        assert!(
            compiled.template.is_some(),
            "{accepted}: {:#?}",
            compiled.diagnostics
        );
    }
    for (rejected, absent) in [
        ("r.b", false),
        ("r.z", false),
        ("r.r", false),
        ("r.l", false),
        ("true", false),
        ("null", false),
        ("r.n > 1", false),
        ("r.o", true),
        ("r.p", true),
    ] {
        let compiled = key(rejected);
        assert!(compiled.template.is_none(), "{rejected} should be refused");
        let message = &compiled.diagnostics[0].message;
        assert!(
            message.contains("a repeat's key must be a string or a number"),
            "{message}"
        );
        assert_eq!(
            message.contains("may be absent"),
            absent,
            "{rejected}: {message}"
        );
        assert_eq!(
            compiled.diagnostics.len(),
            1,
            "{rejected}: one mistake, one diagnostic"
        );
    }
}

// --- The compiler's side -------------------------------------------------------

#[test]
fn the_item_is_in_scope_only_inside_the_repeat() {
    let model = mesh_compiler::check::Model::load(MODEL, "view").unwrap();
    for source in [
        // outside it
        "<page><note>{item.label}</note></page>",
        // an `as` that isn't the name used
        "<page><mesh-each items={items} as=\"x\" key={item.id}><note>a</note></mesh-each></page>",
    ] {
        let compiled = mesh_compiler::check::template(source, &model);
        assert!(compiled.template.is_none(), "{source}");
    }
    // Its fields are typed: an unknown one is the ordinary unknown-member error.
    let compiled = mesh_compiler::check::template(
        "<page><mesh-each items={items} as=\"item\" key={item.id}><note>{item.nope}</note></mesh-each></page>",
        &model,
    );
    assert!(compiled.template.is_none());
}

#[test]
fn a_malformed_repeat_is_refused() {
    let refuse = |source: &str| {
        let template = compile_with(MODEL, "bad", source);
        let templates = [template.as_str()];
        mesh_runtime::check_program(
            &Program {
                root: "bad",
                templates: &templates,
            },
            MODEL,
        )
    };
    for source in [
        // two children
        "<page><mesh-each items={items} as=\"i\" key={i}><note>a</note><note>b</note></mesh-each></page>",
        // none
        "<page><mesh-each items={items} as=\"i\" key={i}></mesh-each></page>",
        // text beside the child
        "<page><mesh-each items={items} as=\"i\" key={i}><note>a</note>text</mesh-each></page>",
        // a repeat as the repeated child
        "<page><mesh-each items={items} as=\"i\" key={i}><mesh-each items={items} as=\"j\" key={j}><note>a</note></mesh-each></mesh-each></page>",
        // an empty name
        "<page><mesh-each items={items} as=\"\" key=\"k\"><note>a</note></mesh-each></page>",
        // the repeat as the template's root
        "<mesh-each items={items} as=\"i\" key={i}><note>a</note></mesh-each>",
    ] {
        let diagnostics = refuse(source);
        assert_eq!(codes(&diagnostics), ["assembly-malformed-template"], "{source}");
    }
}

// --- The conformance vectors, against the implementation -----------------------

/// `examples/conformance/identity/repeat/` is render-v1-level and knows no
/// syntax; this renders the declared keys it gives through the tracer's repeat
/// and checks that the runtime classifies kept, created and removed, and the
/// handlers, as the vectors say, and refuses what they say is refused.
mod vectors {
    use super::*;
    use serde_json::Value;
    use std::collections::BTreeSet;

    fn document() -> Vec<Value> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/conformance/identity/repeat/cases.json");
        let text = std::fs::read_to_string(path).expect("the vectors");
        serde_json::from_str::<Value>(&text).unwrap()["vectors"]
            .as_array()
            .unwrap()
            .clone()
    }

    /// The program's snapshot for declared keys `site → keys`: the site's
    /// label is `site/<key as the identity names write it>`.
    fn snapshot_of(keys: &Value) -> String {
        let site = |name: &str| -> String {
            let rows: Vec<String> = keys
                .get(name)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|key| {
                    let written = match key {
                        Value::Number(n) if n.as_f64() == Some(0.0) => "0".to_string(),
                        other => other.to_string(),
                    };
                    format!(
                        r#"{{"id":{key},"label":{}}}"#,
                        Value::from(format!("{name}/{written}"))
                    )
                })
                .collect();
            format!("[{}]", rows.join(","))
        };
        let (first, second) = if keys.get("list").is_some() {
            ("list", "none")
        } else {
            ("first", "second")
        };
        format!(r#"{{"items":{},"others":{}}}"#, site(first), site(second))
    }

    fn render_keys(keys: &Value) -> Result<Render, Vec<&'static str>> {
        try_render("view", TWO, &snapshot_of(keys)).map_err(|d| codes(&d))
    }

    /// `page/<site>/item[<key>]` → the label the program shows for it.
    fn label(identity: &str) -> String {
        let rest = identity.strip_prefix("page/").unwrap();
        let (site, item) = rest.split_once("/item[").unwrap();
        format!("{site}/{}", item.trim_end_matches(']'))
    }

    fn labels(render: &Render) -> BTreeSet<String> {
        parts(render)
            .into_iter()
            .filter(|p| p.component == "row")
            .map(|p| p.text)
            .collect()
    }

    fn named(list: &Value) -> BTreeSet<String> {
        list.as_array()
            .unwrap()
            .iter()
            .filter(|i| i.as_str().unwrap().contains("/item["))
            .filter(|i| !i.as_str().unwrap().ends_with("/#text"))
            .map(|i| label(i.as_str().unwrap()))
            .collect()
    }

    #[test]
    fn the_runtime_classifies_as_the_vectors_say() {
        let mut checked = 0;
        for vector in document() {
            let Some(previous) = vector.get("previous") else {
                continue;
            };
            let next = &vector["next"];
            let (before, after) = (
                render_keys(&previous["keys"]).expect("renders"),
                render_keys(&next["keys"]).expect("renders"),
            );
            let name = vector["name"].as_str().unwrap();
            let expect = &vector["expect"];
            let (was, now) = (labels(&before), labels(&after));
            let kept: BTreeSet<String> = was.intersection(&now).cloned().collect();
            let created: BTreeSet<String> = now.difference(&was).cloned().collect();
            let removed: BTreeSet<String> = was.difference(&now).cloned().collect();
            assert_eq!(kept, named(&expect["kept"]), "{name}: kept");
            assert_eq!(created, named(&expect["created"]), "{name}: created");
            assert_eq!(removed, named(&expect["removed"]), "{name}: removed");
            // A kept item has the key and the handlers it had; none is shared.
            for kept in &kept {
                let (b, a) = (must(&before, kept), must(&after, kept));
                assert_eq!((&b.key, &b.events), (&a.key, &a.events), "{name}: {kept}");
            }
            let handlers: Vec<String> = parts(&after)
                .into_iter()
                .flat_map(|p| p.events.into_values())
                .collect();
            assert_eq!(
                handlers.len(),
                handlers.iter().collect::<BTreeSet<_>>().len(),
                "{name}: handlers are distinct"
            );
            // A returning identity inherits nothing from the render before it.
            if let Some(earlier) = vector.get("earlier") {
                let earlier = render_keys(&earlier["keys"]).expect("renders");
                for identity in expect["recurring"].as_array().unwrap() {
                    let item = label(identity.as_str().unwrap());
                    assert!(
                        !was.contains(&item),
                        "{name}: absent from the render before"
                    );
                    assert_eq!(must(&earlier, &item).key, must(&after, &item).key);
                }
            }
            checked += 1;
        }
        assert_eq!(checked, 10);
    }

    #[test]
    fn the_runtime_refuses_what_the_vectors_refuse() {
        for vector in document() {
            let name = vector["name"].as_str().unwrap();
            if name.starts_with("C16") {
                assert_eq!(
                    render_keys(&vector["declaredKeys"]).err(),
                    Some(vec!["runtime-duplicate-key"])
                );
            }
            if name.starts_with("C17") {
                for case in vector["invalid"].as_array().unwrap() {
                    // An absent key can't be built from a snapshot of a program that
                    // compiles (the checker refuses it); the rest are values.
                    if case["category"] == "absent" {
                        continue;
                    }
                    assert_eq!(
                        render_keys(&case["declaredKeys"]).err(),
                        Some(vec!["runtime-invalid-key"]),
                        "{}",
                        case["category"]
                    );
                }
            }
        }
    }
}
