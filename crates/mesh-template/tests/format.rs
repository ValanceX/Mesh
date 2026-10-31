//! Reading and writing `template-v1` and `template-v2` (docs/reference/templates.md, "The
//! format"): what reads, what is refused and why, and that what is
//! written reads back as the same template and matches the schema.

use mesh_template::{from_json, to_json, Child, Expression, Literal, Refusal};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

const EXAMPLE: &str = include_str!("example-template.json");

fn example() -> Value {
    serde_json::from_str(EXAMPLE).expect("the example is JSON")
}

fn validator() -> jsonschema::Validator {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas/template-v1.schema.json");
    let schema: Value = serde_json::from_str(&fs::read_to_string(path).expect("the schema reads"))
        .expect("the schema is JSON");
    jsonschema::draft202012::new(&schema).expect("the schema is valid")
}

fn span() -> Value {
    json!({ "start": { "byte": 0, "utf16": 0 }, "end": { "byte": 1, "utf16": 1 } })
}

fn malformed(document: &Value) -> Vec<String> {
    match from_json(&document.to_string()) {
        Err(Refusal::Malformed(problems)) => problems.iter().map(ToString::to_string).collect(),
        other => panic!("expected Malformed, got {other:?}"),
    }
}

#[test]
fn the_manuals_example_reads() {
    let template = from_json(EXAMPLE).expect("the example reads");
    assert_eq!(template.component, "user-card");
    assert_eq!(template.compiler, "0.5.0");
    assert_eq!(template.root.component, "avatar");
    assert_eq!(template.root.props.len(), 2);
    assert_eq!(template.root.events[0].command, "selectUser");
    assert!(matches!(
        &template.root.events[0].arguments[0],
        Expression::Scope { name, .. } if name == "user"
    ));
}

#[test]
fn what_is_written_reads_back_and_matches_the_schema() {
    let validator = validator();
    let template = from_json(EXAMPLE).expect("the example reads");
    let written = to_json(&template);
    let value: Value = serde_json::from_str(&written).expect("it writes JSON");
    assert!(validator.is_valid(&value), "{written}");
    assert_eq!(from_json(&written).expect("it reads back"), template);
    assert_eq!(value, example(), "the example is already in written form");
    assert!(
        !written.contains('\n') && !written.contains(": "),
        "no whitespace"
    );
}

#[test]
fn every_expression_kind_round_trips() {
    let s = span();
    let scope = json!({ "kind": "scope", "name": "user", "span": s });
    let expression = json!({ "kind": "conditional", "span": s,
        "condition": { "kind": "binary", "operator": "less-equal", "span": s,
            "left": { "kind": "unary", "operator": "negate", "span": s,
                "operand": { "kind": "literal", "value": 1.5, "span": s } },
            "right": { "kind": "member", "object": scope, "field": "age", "span": s } },
        "consequent": { "kind": "list", "span": s, "elements": [
            { "kind": "literal", "value": "a", "span": s },
            { "kind": "literal", "value": true, "span": s },
            { "kind": "literal", "value": null, "span": s } ] },
        "alternate": { "kind": "record", "span": s, "fields": [
            { "name": "n", "value": { "kind": "literal", "value": 0.1, "span": s }, "span": s } ] } });
    let mut document = example();
    document["root"]["props"][0]["value"] = expression;
    document["root"]["events"][0]["arguments"] = json!([{ "kind": "record", "span": s, "fields": [
            { "name": "at", "value": { "kind": "event", "span": s }, "span": s } ] }]);
    document["root"]["children"] = json!([
        { "kind": "text", "value": "Hello, ", "span": s },
        { "kind": "expression", "expression": scope },
        { "kind": "element", "element": { "component": "text", "props": [], "events": [], "children": [], "span": s } }
    ]);
    let text = document.to_string();
    assert!(validator().is_valid(&document));
    let template = from_json(&text).expect("it reads");
    assert_eq!(
        serde_json::from_str::<Value>(&to_json(&template)).unwrap(),
        document
    );
    assert!(matches!(template.root.children[0], Child::Text { .. }));
}

#[test]
fn numbers_keep_their_exact_binary64_value() {
    let s = span();
    for value in [
        0.1,
        1.0,
        1e300,
        5e-324,
        9007199254740992.0,
        0.30000000000000004,
    ] {
        let mut document = example();
        document["root"]["props"][0]["value"] =
            json!({ "kind": "literal", "value": value, "span": s });
        let template = from_json(&document.to_string()).expect("it reads");
        let Expression::Literal {
            value: Literal::Number(read),
            ..
        } = &template.root.props[0].value
        else {
            panic!("a number literal");
        };
        assert_eq!(read.to_bits(), f64::to_bits(value));
        let again = from_json(&to_json(&template)).expect("it reads back");
        assert_eq!(again, template);
    }
    // A number written with more digits than it needs still reads as
    // its nearest binary64 value, correctly rounded.
    let mut document = example();
    document["root"]["props"][0]["value"] = json!({ "kind": "literal", "value": 12345, "span": s });
    let text = document
        .to_string()
        .replace("12345", "0.1000000000000000055511151231257827");
    let template = from_json(&text).expect("it reads");
    let Expression::Literal {
        value: Literal::Number(read),
        ..
    } = &template.root.props[0].value
    else {
        panic!("a number literal");
    };
    assert_eq!(read.to_bits(), 0.1f64.to_bits());
}

#[test]
fn unknown_properties_are_ignored() {
    let mut document = example();
    document["later"] = json!({ "anything": [1, 2] });
    document["root"]["hint"] = json!("a property a later v1 might add");
    document["root"]["props"][0]["value"]["note"] = json!(true);
    let template = from_json(&document.to_string()).expect("it reads");
    assert_eq!(template, from_json(EXAMPLE).unwrap());
}

#[test]
fn another_version_is_unsupported_not_malformed() {
    for version in [0, 3, 17] {
        let mut document = example();
        document["version"] = json!(version);
        assert_eq!(
            from_json(&document.to_string()),
            Err(Refusal::UnsupportedVersion(version))
        );
    }
}

#[test]
fn what_is_not_a_template_is_malformed() {
    let s = span();
    assert_eq!(malformed_text("{ not json").len(), 1);
    assert_eq!(malformed_text("[]").len(), 1);

    let mut other_format = example();
    other_format["format"] = json!("mesh-render");
    assert_eq!(malformed(&other_format).len(), 1);

    let mut string_version = example();
    string_version["version"] = json!("1");
    assert_eq!(malformed(&string_version).len(), 1);

    let mut fractional_version = example();
    fractional_version["version"] = json!(1.5);
    assert_eq!(malformed(&fractional_version).len(), 1);

    let mut no_fingerprint = example();
    no_fingerprint
        .as_object_mut()
        .unwrap()
        .remove("fingerprint");
    assert_eq!(malformed(&no_fingerprint).len(), 1);

    let mut bad_fingerprint = example();
    bad_fingerprint["fingerprint"] = json!("sha256:1234");
    assert_eq!(malformed(&bad_fingerprint).len(), 1);

    let mut unknown_kind = example();
    unknown_kind["root"]["props"][0]["value"] = json!({ "kind": "command", "span": s });
    assert_eq!(malformed(&unknown_kind).len(), 1);

    let mut empty_text = example();
    empty_text["root"]["children"] = json!([{ "kind": "text", "value": "", "span": s }]);
    assert_eq!(malformed(&empty_text).len(), 1);

    let mut bad_name = example();
    bad_name["root"]["props"][0]["prop"] = json!("not-an-identifier");
    assert_eq!(malformed(&bad_name).len(), 1);

    let mut bad_component = example();
    bad_component["root"]["component"] = json!("has space");
    assert_eq!(malformed(&bad_component).len(), 1);
}

fn malformed_text(text: &str) -> Vec<String> {
    match from_json(text) {
        Err(Refusal::Malformed(problems)) => problems.iter().map(ToString::to_string).collect(),
        other => panic!("expected Malformed, got {other:?}"),
    }
}

/// `$event` may appear only inside a handler's arguments, at any depth
/// (§9.5). Every misplaced one is reported, each with where it is.
#[test]
fn event_outside_a_handler_is_malformed_everywhere_it_is() {
    let s = span();
    let event = json!({ "kind": "event", "span": s });
    let mut document = example();
    document["root"]["props"][0]["value"] = event.clone();
    document["root"]["props"][1]["value"] =
        json!({ "kind": "list", "span": s, "elements": [event.clone()] });
    document["root"]["children"] = json!([{ "kind": "expression", "expression": event }]);
    let problems = malformed(&document);
    assert_eq!(problems.len(), 3, "{problems:#?}");
    assert!(
        problems[0].contains("/root/props/0/value"),
        "{}",
        problems[0]
    );
    assert!(
        problems[1].contains("/root/props/1/value/elements/0"),
        "{}",
        problems[1]
    );
    assert!(
        problems[2].contains("/root/children/0/expression"),
        "{}",
        problems[2]
    );
}

#[test]
fn duplicate_names_are_malformed() {
    let mut props = example();
    let first = props["root"]["props"][0].clone();
    props["root"]["props"][1] = first;
    assert_eq!(malformed(&props).len(), 1, "two props named src");
    let mut events = example();
    let first = events["root"]["events"][0].clone();
    events["root"]["events"].as_array_mut().unwrap().push(first);
    assert_eq!(malformed(&events).len(), 1, "two click bindings");
}

#[test]
fn a_duplicate_record_field_is_malformed() {
    let s = span();
    let one = json!({ "kind": "literal", "value": 1, "span": s });
    let mut document = example();
    document["root"]["props"][0]["value"] = json!({ "kind": "record", "span": s, "fields": [
        { "name": "a", "value": one, "span": s }, { "name": "a", "value": one, "span": s } ] });
    assert_eq!(malformed(&document).len(), 1);
}

#[test]
fn a_negative_zero_literal_is_malformed() {
    let mut document = example();
    document["root"]["props"][0]["value"] =
        json!({ "kind": "literal", "value": 12345, "span": span() });
    let text = document.to_string().replace("12345", "-0.0");
    assert_eq!(malformed_text(&text).len(), 1);
}

/// An object literal's key may be quoted, so any string (§5): a record
/// field's name isn't restricted to an identifier, unlike a member
/// access's field.
#[test]
fn a_record_field_name_is_any_string() {
    let s = span();
    let one = json!({ "kind": "literal", "value": 1, "span": s });
    let mut document = example();
    document["root"]["props"][0]["value"] = json!({ "kind": "record", "span": s, "fields": [
        { "name": "display-name", "value": one, "span": s },
        { "name": "", "value": one, "span": s },
        { "name": "with space", "value": one, "span": s } ] });
    assert!(validator().is_valid(&document));
    from_json(&document.to_string()).expect("any string is a field name");
    let mut member = example();
    member["root"]["props"][0]["value"]["field"] = json!("display-name");
    assert_eq!(
        malformed(&member).len(),
        1,
        "a member access's field is an identifier"
    );
}

/// A template nested past `MAX_JSON_DEPTH` is refused as malformed, in
/// bounded time and stack, not crashed on, and one nested far deeper than
/// serde_json's own limit of 128 levels (about 42 elements) reads.
#[test]
fn json_depth_is_limited_by_the_format_not_the_parser() {
    let deep = |levels: usize| {
        let mut document = example();
        document["root"]["children"] = Value::Null;
        let text = document.to_string().replace(
            "\"children\":null",
            &format!("\"children\":{}{}", "[".repeat(levels), "]".repeat(levels)),
        );
        text
    };
    let refused = malformed_text(&deep(100_000));
    assert_eq!(refused.len(), 1);
    assert!(refused[0].contains("levels of JSON deep"), "{}", refused[0]);
    // Past serde_json's limit, within ours: refused for what it is (a
    // children list of lists), not for its depth.
    let inside = malformed_text(&deep(500));
    assert!(!inside[0].contains("levels of JSON deep"), "{}", inside[0]);
    // Brackets inside strings don't count.
    let mut document = example();
    document["compiler"] = json!("[".repeat(5_000));
    assert!(from_json(&document.to_string()).is_ok());
}

fn validator_v2() -> jsonschema::Validator {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas/template-v2.schema.json");
    let schema: Value = serde_json::from_str(&fs::read_to_string(path).expect("the schema reads"))
        .expect("the schema is JSON");
    jsonschema::draft202012::new(&schema).expect("the schema is valid")
}

/// The example with `has(user)` as a prop's value, and `a ?? b` as another's.
fn with_v2_constructs() -> Value {
    let s = span();
    let scope = json!({ "kind": "scope", "name": "user", "span": s });
    let mut document = example();
    document["root"]["props"][0]["value"] = json!({ "kind": "has", "operand": scope, "span": s });
    document["root"]["props"][1]["value"] = json!({ "kind": "binary", "operator": "coalesce", "span": s,
        "left": scope, "right": { "kind": "literal", "value": "x", "span": s } });
    document
}

#[test]
fn has_and_coalesce_are_template_v2() {
    let mut document = with_v2_constructs();
    document["version"] = json!(2);
    let template = from_json(&document.to_string()).expect("version 2 reads them");
    let written: Value = serde_json::from_str(&to_json(&template)).unwrap();
    assert_eq!(
        written["version"], 2,
        "a template that uses them is written as version 2"
    );
    assert_eq!(written, document);
    assert!(validator_v2().is_valid(&written));
    assert!(
        !validator().is_valid(&written),
        "version 1 has neither the kind nor the operator"
    );
}

#[test]
fn a_template_that_uses_nothing_of_version_2_is_written_as_version_1() {
    let template = from_json(EXAMPLE).expect("the example reads");
    let written: Value = serde_json::from_str(&to_json(&template)).unwrap();
    assert_eq!(written["version"], 1);
    assert_eq!(written, example(), "byte for byte as before");
}

#[test]
fn version_2_reads_a_template_that_uses_nothing_of_it() {
    let mut document = example();
    document["version"] = json!(2);
    let template = from_json(&document.to_string()).expect("version 2 is a superset");
    assert_eq!(
        serde_json::from_str::<Value>(&to_json(&template)).unwrap()["version"],
        1,
        "and is written as version 1"
    );
}

#[test]
fn a_version_1_document_with_a_version_2_construct_is_malformed() {
    let document = with_v2_constructs();
    let problems = malformed(&document);
    assert_eq!(problems.len(), 2, "{problems:?}");
    assert!(
        problems[0].contains("`has` is a template-v2"),
        "{problems:?}"
    );
    assert!(
        problems[1].contains("`coalesce` is a template-v2"),
        "{problems:?}"
    );
}
