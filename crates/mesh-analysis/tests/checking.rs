//! Contextual checking (outline D9): prop values, command arguments and
//! `$event` checked with `is_assignable` against the types expected of
//! them, and object literals checked field by field.

mod common;

use common::{at, names, nth};
use mesh_analysis::{Analysis, Expectation, Fact, FieldTy, Ty};
use std::collections::BTreeMap;

const MANIFEST: &str = r#"{
  "version": 1,
  "types": {
    "User": { "kind": "record", "fields": {
      "name": { "type": { "kind": "string" }, "required": true },
      "avatar": { "type": { "kind": "string" }, "required": false },
      "manager": { "type": { "kind": "optional", "type": { "kind": "named", "name": "Person" } }, "required": false }
    } },
    "Person": { "kind": "record", "fields": {
      "name": { "type": { "kind": "string" }, "required": true }
    } }
  },
  "components": {
    "card": {
      "props": {
        "title": { "type": { "kind": "string" }, "required": false },
        "user": { "type": { "kind": "named", "name": "User" }, "required": false },
        "maybeUser": { "type": { "kind": "optional", "type": { "kind": "named", "name": "User" } }, "required": false },
        "anything": { "type": { "kind": "any" }, "required": false },
        "tags": { "type": { "kind": "list", "element": { "kind": "string" } }, "required": false },
        "people": { "type": { "kind": "list", "element": { "kind": "named", "name": "User" } }, "required": false }
      },
      "events": {
        "pick": { "payload": { "kind": "named", "name": "User" } },
        "type": { "payload": { "kind": "string" } }
      },
      "commands": {}, "scope": {}
    },
    "view": {
      "props": {}, "events": {},
      "commands": {
        "select": { "parameters": [{ "name": "user", "type": { "kind": "named", "name": "User" } }] },
        "rename": { "parameters": [
          { "name": "user", "type": { "kind": "named", "name": "User" } },
          { "name": "name", "type": { "kind": "string" } }
        ] }
      },
      "scope": {
        "user": { "kind": "named", "name": "User" },
        "name": { "kind": "string" },
        "count": { "kind": "number" },
        "maybeName": { "kind": "optional", "type": { "kind": "string" } }
      }
    }
  }
}"#;

fn analyze(source: &str) -> Analysis {
    common::analyze(MANIFEST, "view", source)
}

fn named(name: &str) -> Ty {
    Ty::Named(name.to_string())
}

fn prop(prop: &str) -> Expectation {
    Expectation::Prop {
        component: "card".to_string(),
        prop: prop.to_string(),
    }
}

fn mismatch(expectation: Expectation, expected: Ty, actual: Ty, span: mesh_syntax::Span) -> Fact {
    Fact::TypeMismatch {
        expectation,
        expected,
        actual,
        possibly_absent: false,
        span,
    }
}

#[test]
fn prop_values_are_checked_against_their_declared_type() {
    let source = r#"<card title={count}><card title="Hi" user={user} tags={[name]} /></card>"#;
    assert_eq!(
        analyze(source).facts(),
        [mismatch(
            prop("title"),
            Ty::String,
            Ty::Number,
            at(source, "count")
        )]
    );
}

#[test]
fn a_string_attribute_value_is_a_string() {
    let source = r#"<card user="Ada" />"#;
    assert_eq!(
        analyze(source).facts(),
        [mismatch(
            prop("user"),
            named("User"),
            Ty::String,
            at(source, r#""Ada""#)
        )]
    );
}

/// D5: an optional declaration doesn't make the value optional.
#[test]
fn a_possibly_absent_value_needs_a_possibly_absent_prop() {
    let source = r#"<card title={maybeName} />"#;
    assert_eq!(
        analyze(source).facts(),
        [Fact::TypeMismatch {
            expectation: prop("title"),
            expected: Ty::String,
            actual: Ty::Optional(Box::new(Ty::String)),
            possibly_absent: true,
            span: at(source, "maybeName"),
        }]
    );
}

/// An unknown prop's value, and an expression with an error, are
/// reported once and not compared with anything.
#[test]
fn errors_are_not_also_mismatches() {
    for source in [
        r#"<card titel={count} />"#,
        r#"<card title={usr} />"#,
        r#"<card title={save()} />"#,
        r#"<card title={user.nmae} />"#,
        r#"<crad title={count} />"#,
    ] {
        assert_eq!(analyze(source).facts().len(), 1, "{source}");
    }
}

#[test]
fn command_arguments_are_checked_against_their_parameters() {
    let source = r#"<card on.pick={rename($event, count)} on.type={select($event)} />"#;
    let argument = |parameter: &str, command: &str| Expectation::Argument {
        command: command.to_string(),
        parameter: parameter.to_string(),
    };
    assert_eq!(
        analyze(source).facts(),
        [
            mismatch(
                argument("name", "rename"),
                Ty::String,
                Ty::Number,
                at(source, "count")
            ),
            mismatch(
                argument("user", "select"),
                named("User"),
                Ty::String,
                nth(source, "$event", 1)
            ),
        ]
    );
}

/// With the wrong number of arguments, arity is the one mistake: the
/// arguments are typed, not compared with the parameters.
#[test]
fn arguments_of_the_wrong_arity_are_only_typed() {
    let source = r#"<card on.pick={rename(count)} />"#;
    assert!(matches!(
        analyze(source).facts(),
        [Fact::CommandArityMismatch { .. }]
    ));
}

#[test]
fn an_object_literal_is_checked_field_by_field() {
    let source = r#"<card user={{ nmae: "Ada", avatar: 1 }} />"#;
    assert_eq!(
        analyze(source).facts(),
        // In source order: the literal starts before its first key.
        [
            Fact::MissingRequiredField {
                record: named("User"),
                field: "name".to_string(),
                span: at(source, r#"{ nmae: "Ada", avatar: 1 }"#),
            },
            Fact::UnknownField {
                record: named("User"),
                field: "nmae".to_string(),
                span: at(source, "nmae"),
                candidates: names(&["avatar", "manager", "name"]),
            },
            mismatch(
                Expectation::Field {
                    field: "avatar".to_string()
                },
                Ty::String,
                Ty::Number,
                at(source, "1")
            ),
        ]
    );
}

/// A literal is present, so where an optional record is expected, its
/// fields are checked against that record. Nested literals are checked
/// the same way, through optional fields and named types.
#[test]
fn nested_and_optional_records_are_checked_field_by_field() {
    let source = r#"<card maybeUser={{ name: name, manager: { name: 1 } }} />"#;
    assert_eq!(
        analyze(source).facts(),
        [mismatch(
            Expectation::Field {
                field: "name".to_string()
            },
            Ty::String,
            Ty::Number,
            at(source, "1")
        )]
    );
    assert_eq!(
        analyze(r#"<card maybeUser={{ name: name, manager: { name: name } }} />"#).facts(),
        []
    );
}

/// Only an object literal meeting a record type is checked field by field.
/// Anywhere else, the whole value is compared.
#[test]
fn other_objects_are_compared_whole() {
    let source = r#"<card title={{ name: name }} anything={{ extra: 1 }} />"#;
    let object = Ty::Record(BTreeMap::from([(
        "name".to_string(),
        FieldTy {
            ty: Ty::String,
            required: true,
        },
    )]));
    assert_eq!(
        analyze(source).facts(),
        [mismatch(
            prop("title"),
            Ty::String,
            object,
            at(source, "{ name: name }")
        )]
    );
}

/// A repeated key is reported once, whichever way the literal is checked.
/// As with attributes, the last occurrence counts: it is the one checked
/// against the field, and the shadowed `1` isn't.
#[test]
fn a_repeated_key_is_reported_once() {
    let source = r#"<card user={{ name: 1, name: name }} />"#;
    assert_eq!(
        analyze(source).facts(),
        [Fact::DuplicateObjectKey {
            key: "name".to_string(),
            span: nth(source, "name", 0),
            last: nth(source, "name", 1),
        }]
    );
}

#[test]
fn a_checked_object_literal_records_its_own_type() {
    let source = r#"<card user={{ name: name }} />"#;
    assert_eq!(
        analyze(source)
            .type_at(at(source, "{ name: name }"))
            .map(|ty| ty.to_string()),
        Some("{ name: string }".to_string())
    );
}

/// An array literal where a list is expected is checked element by
/// element, so its elements need no common type.
#[test]
fn an_array_literal_is_checked_element_by_element() {
    let people = r#"[{ name: name }, { name: name, avatar: name }]"#;
    let source = format!("<card people={{{people}}} tags={{[name, count]}} />");
    let analysis = analyze(&source);
    assert_eq!(
        analysis.facts(),
        [mismatch(
            Expectation::Element,
            Ty::String,
            Ty::Number,
            at(&source, "count")
        )]
    );
    assert_eq!(
        analysis
            .type_at(at(&source, people))
            .map(|ty| ty.to_string()),
        Some("list<User>".to_string())
    );
}

/// A conditional where a type is expected is checked branch by branch,
/// so its branches need no common type. Where `any` is expected, every
/// branch fits.
#[test]
fn each_branch_is_checked_against_the_expected_type() {
    let user = r#"count > 1 ? { name: name } : { name: name, avatar: name }"#;
    let source = format!(
        "<card user={{{user}}} title={{count > 1 ? name : count}} anything={{count > 1 ? name : count}} />"
    );
    let analysis = analyze(&source);
    assert_eq!(
        analysis.facts(),
        [mismatch(
            prop("title"),
            Ty::String,
            Ty::Number,
            nth(&source, "count", 2)
        )]
    );
    assert_eq!(
        analysis.type_at(at(&source, user)).map(|ty| ty.to_string()),
        Some("User".to_string())
    );
}
