//! The model fingerprint (docs/reference/templates.md, "The model
//! fingerprint"): determined by the model's meaning and nothing else.

use mesh_template::{fingerprint, Fingerprint};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

/// The manual's worked example.
const EXAMPLE: &str = include_str!("example-manifest.json");
const EXAMPLE_FINGERPRINT: &str =
    "sha256:6fca78440df9c56ff4a3b7e180af554be971083db4bc2eda137f25c0dddd5caf";

fn of(manifest: &str) -> Fingerprint {
    fingerprint(&mesh_manifest::load(manifest).expect("the manifest loads"))
}

fn of_value(manifest: &Value) -> Fingerprint {
    of(&manifest.to_string())
}

fn example() -> Value {
    serde_json::from_str(EXAMPLE).expect("the example is JSON")
}

#[test]
fn the_manuals_worked_example_reproduces() {
    assert_eq!(of(EXAMPLE).to_string(), EXAMPLE_FINGERPRINT);
}

#[test]
fn a_fingerprint_displays_and_parses_as_sha256_hex() {
    let parsed: Fingerprint = EXAMPLE_FINGERPRINT.parse().expect("it parses");
    assert_eq!(parsed, of(EXAMPLE));
    for bad in [
        "sha256:6FCA78440df9c56ff4a3b7e180af554be971083db4bc2eda137f25c0dddd5caf",
        "sha256:6fca",
        "sha1:6fca78440df9c56ff4a3b7e180af554be971083db4bc2eda137f25c0dddd5caf",
        "6fca78440df9c56ff4a3b7e180af554be971083db4bc2eda137f25c0dddd5caf",
    ] {
        assert!(
            bad.parse::<Fingerprint>().is_err(),
            "{bad} should not parse"
        );
    }
}

/// Reverses the order of every object's keys, at every depth.
fn reversed(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .rev()
                .map(|(key, value)| (key.clone(), reversed(value)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(reversed).collect()),
        other => other.clone(),
    }
}

#[test]
fn what_does_not_count_does_not_change_it() {
    let base = of(EXAMPLE);
    // Formatting and whitespace.
    assert_eq!(of_value(&example()), base, "reformatted");
    // Key order (needs serde_json's preserve_order to be observable; a
    // reordered text is written by hand as well).
    assert_eq!(of_value(&reversed(&example())), base, "keys reversed");
    // `$schema`.
    let mut with_schema = example();
    with_schema["$schema"] = json!("../schemas/manifest-v1.schema.json");
    assert_eq!(of_value(&with_schema), base, "$schema added");
    // A renamed named type.
    let renamed = EXAMPLE.replace("\"User\"", "\"Person\"");
    assert_eq!(of(&renamed), base, "User renamed Person");
    // An unused named type.
    let mut unused = example();
    unused["types"]["Unused"] = json!({ "kind": "number" });
    assert_eq!(of_value(&unused), base, "an unused named type");
    // A renamed parameter.
    let parameter = EXAMPLE.replace("\"name\": \"user\"", "\"name\": \"who\"");
    assert_ne!(parameter, EXAMPLE);
    assert_eq!(of(&parameter), base, "a parameter renamed");
    // A named type inlined where it is used.
    let mut inlined = example();
    let user = inlined["types"]["User"].clone();
    inlined["components"]["user-card"]["props"]["user"]["type"] = user.clone();
    inlined["components"]["user-card"]["scope"]["user"] = user.clone();
    inlined["components"]["user-card"]["commands"]["selectUser"]["parameters"][0]["type"] = user;
    assert_eq!(of_value(&inlined), base, "User inlined");
}

/// A named change to a manifest.
type Mutation = (&'static str, fn(&mut Value));

#[test]
fn what_counts_changes_it() {
    let base = of(EXAMPLE);
    let mutations: Vec<Mutation> = vec![
        (
            "a component added",
            |m| {
                m["components"]["badge"] =
                    json!({ "props": {}, "events": {}, "commands": {}, "scope": {} })
            },
        ),
        ("a component removed", |m| {
            m["components"].as_object_mut().unwrap().remove("avatar");
        }),
        ("a component renamed", |m| {
            let avatar = m["components"]
                .as_object_mut()
                .unwrap()
                .remove("avatar")
                .unwrap();
            m["components"]["picture"] = avatar;
        }),
        ("a prop added", |m| {
            m["components"]["avatar"]["props"]["size"] =
                json!({ "type": { "kind": "number" }, "required": false })
        }),
        ("a prop's requiredness", |m| {
            m["components"]["avatar"]["props"]["alt"]["required"] = json!(false)
        }),
        ("a prop's type", |m| {
            m["components"]["avatar"]["props"]["alt"]["type"] = json!({ "kind": "any" })
        }),
        ("a type deep in a record", |m| {
            m["types"]["User"]["fields"]["name"]["type"] = json!({ "kind": "number" })
        }),
        ("a record field's requiredness", |m| {
            m["types"]["User"]["fields"]["avatar"]["required"] = json!(true)
        }),
        ("an event added", |m| {
            m["components"]["avatar"]["events"]["hover"] = json!({})
        }),
        ("an event's payload", |m| {
            m["components"]["avatar"]["events"]["click"] = json!({ "payload": { "kind": "null" } })
        }),
        ("a command's parameter type", |m| {
            m["components"]["user-card"]["commands"]["selectUser"]["parameters"][0]["type"] =
                json!({ "kind": "string" })
        }),
        ("a command's parameter added", |m| {
            m["components"]["user-card"]["commands"]["selectUser"]["parameters"]
                .as_array_mut()
                .unwrap()
                .push(json!({ "name": "again", "type": { "kind": "boolean" } }))
        }),
        ("a command renamed", |m| {
            let commands = m["components"]["user-card"]["commands"]
                .as_object_mut()
                .unwrap();
            let command = commands.remove("selectUser").unwrap();
            commands.insert("pick".into(), command);
        }),
        ("a scope name's type", |m| {
            m["components"]["user-card"]["scope"]["user"] =
                json!({ "kind": "optional", "type": { "kind": "named", "name": "User" } })
        }),
        ("a scope name added", |m| {
            m["components"]["user-card"]["scope"]["count"] = json!({ "kind": "number" })
        }),
        ("a list's element type", |m| {
            m["components"]["user-card"]["scope"]["user"] =
                json!({ "kind": "list", "element": { "kind": "named", "name": "User" } })
        }),
    ];
    let mut seen = vec![base];
    for (what, mutate) in mutations {
        let mut manifest = example();
        mutate(&mut manifest);
        let changed = of_value(&manifest);
        assert!(
            !seen.contains(&changed),
            "{what} should change the fingerprint"
        );
        seen.push(changed);
    }
}

#[test]
fn parameter_order_counts() {
    let manifest = |first: &str, second: &str| {
        json!({ "version": 1, "types": {}, "components": { "c": {
            "props": {}, "events": {}, "scope": {},
            "commands": { "go": { "parameters": [
                { "name": "a", "type": { "kind": first } },
                { "name": "b", "type": { "kind": second } }
            ] } }
        } } })
    };
    assert_ne!(
        of_value(&manifest("string", "number")),
        of_value(&manifest("number", "string"))
    );
}

/// Sixty-four named types, each a record using the next one twice: a
/// full expansion would have 2^64 leaves, so only a per-type digest can
/// finish.
#[test]
fn named_types_are_digested_once_each() {
    let mut types = serde_json::Map::new();
    for index in 0..64 {
        let next = json!({ "kind": "named", "name": format!("T{}", index + 1) });
        types.insert(
            format!("T{index}"),
            json!({ "kind": "record", "fields": {
                "left": { "type": next, "required": true },
                "right": { "type": next, "required": true }
            } }),
        );
    }
    types.insert("T64".into(), json!({ "kind": "string" }));
    let manifest = json!({ "version": 1, "types": types, "components": { "c": {
        "props": {}, "events": {}, "commands": {},
        "scope": { "t": { "kind": "named", "name": "T0" } }
    } } });
    let start = Instant::now();
    of_value(&manifest);
    assert!(
        start.elapsed() < Duration::from_secs(1),
        "{:?}",
        start.elapsed()
    );
}

fn with_prop(ty: Value, default: Option<Value>) -> Value {
    let mut prop = json!({ "type": ty, "required": false });
    if let Some(default) = default {
        prop["default"] = default;
    }
    json!({ "version": 1, "types": {}, "components": {
        "c": { "props": { "p": prop }, "events": {}, "commands": {}, "scope": {} }
    } })
}

#[test]
fn an_enum_is_a_set_so_its_order_does_not_count() {
    let a = json!({ "kind": "enum", "values": ["a", "b", "c"] });
    let b = json!({ "kind": "enum", "values": ["c", "a", "b"] });
    let other = json!({ "kind": "enum", "values": ["a", "b"] });
    assert_eq!(
        of_value(&with_prop(a.clone(), None)),
        of_value(&with_prop(b, None))
    );
    assert_ne!(
        of_value(&with_prop(a, None)),
        of_value(&with_prop(other, None))
    );
    assert_ne!(
        of_value(&with_prop(json!({ "kind": "enum", "values": ["a"] }), None)),
        of_value(&with_prop(json!({ "kind": "string" }), None))
    );
}

#[test]
fn a_default_changes_the_fingerprint_and_only_a_default_does() {
    let ty = json!({ "kind": "string" });
    let none = of_value(&with_prop(ty.clone(), None));
    let a = of_value(&with_prop(ty.clone(), Some(json!("a"))));
    let b = of_value(&with_prop(ty.clone(), Some(json!("b"))));
    assert_ne!(none, a);
    assert_ne!(a, b);
    assert_eq!(a, of_value(&with_prop(ty, Some(json!("a")))));
    // -0 and 0 are the same default.
    let number = json!({ "kind": "number" });
    assert_eq!(
        of_value(&with_prop(number.clone(), Some(json!(0)))),
        of_value(&with_prop(number, Some(json!(-0.0))))
    );
}

fn with_scope(ty: Value) -> Value {
    json!({ "version": 1, "types": {}, "components": {
        "c": { "props": {}, "events": {}, "commands": {}, "scope": { "x": ty } }
    } })
}

#[test]
fn a_snippet_is_its_ordered_parameters() {
    let snippet = |params: Value| json!({ "kind": "snippet", "params": params });
    let (s, n) = (json!({ "kind": "string" }), json!({ "kind": "number" }));
    let sn = of_value(&with_scope(snippet(json!([s, n]))));
    assert_eq!(sn, of_value(&with_scope(snippet(json!([s, n])))));
    assert_ne!(
        sn,
        of_value(&with_scope(snippet(json!([n, s])))),
        "order counts"
    );
    assert_ne!(
        sn,
        of_value(&with_scope(snippet(json!([s])))),
        "arity counts"
    );
    assert_ne!(
        of_value(&with_scope(snippet(json!([])))),
        of_value(&with_scope(snippet(json!([s])))),
    );
}

#[test]
fn a_commands_effect_and_title_are_not_part_of_a_fingerprint() {
    // They annotate a command for a host that asks a person before running it (decision G13); a template compiled before they were written is still valid after.
    let mut annotated = example();
    let mut commands_seen = 0;

    for component in annotated["components"]
        .as_object_mut()
        .expect("components")
        .values_mut()
    {
        for command in component["commands"]
            .as_object_mut()
            .expect("commands")
            .values_mut()
        {
            command["effect"] = json!("destructive");
            command["title"] = json!("Do it");
            commands_seen += 1;
        }
    }

    assert!(commands_seen > 0, "the example declares commands");
    assert_eq!(of_value(&annotated), of(EXAMPLE));
}
