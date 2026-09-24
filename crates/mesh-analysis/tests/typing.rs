//! Expression typing (outline D15), one test per rule. Each expression is
//! checked as the content of a `<box>`, where nothing is expected of it,
//! so only typing's own facts appear.

mod common;

use common::{at, names};
use mesh_analysis::{Analysis, Combination, Expectation, Fact, Operator, Ty};
use mesh_syntax::{BinaryOperator, UnaryOperator};

const MANIFEST: &str = r#"{
  "version": 1,
  "types": {
    "User": { "kind": "record", "fields": {
      "name": { "type": { "kind": "string" }, "required": true },
      "avatar": { "type": { "kind": "string" }, "required": false },
      "nick": { "type": { "kind": "optional", "type": { "kind": "string" } }, "required": true },
      "bio": { "type": { "kind": "optional", "type": { "kind": "string" } }, "required": false },
      "label": { "type": { "kind": "named", "name": "Maybe" }, "required": false }
    } },
    "Maybe": { "kind": "optional", "type": { "kind": "string" } }
  },
  "components": {
    "box": {
      "props": {},
      "events": { "pick": { "payload": { "kind": "named", "name": "User" } } },
      "commands": {}, "scope": {}
    },
    "view": {
      "props": {}, "events": {},
      "commands": {
        "take": { "parameters": [{ "name": "user", "type": { "kind": "named", "name": "User" } }] }
      },
      "scope": {
        "user": { "kind": "named", "name": "User" },
        "maybeUser": { "kind": "optional", "type": { "kind": "named", "name": "User" } },
        "users": { "kind": "list", "element": { "kind": "named", "name": "User" } },
        "name": { "kind": "string" },
        "maybeName": { "kind": "optional", "type": { "kind": "string" } },
        "count": { "kind": "number" },
        "flag": { "kind": "boolean" },
        "maybeFlag": { "kind": "optional", "type": { "kind": "boolean" } },
        "anything": { "kind": "any" },
        "maybeAnything": { "kind": "optional", "type": { "kind": "any" } }
      }
    }
  }
}"#;

fn analyze(source: &str) -> Analysis {
    common::analyze(MANIFEST, "view", source)
}

/// `expression`, as the content of a `<box>`: its type (as displayed), if
/// it has one, and every fact reported.
fn type_of(expression: &str) -> (Option<String>, Vec<Fact>) {
    let source = format!("<box>{{{expression}}}</box>");
    let analysis = analyze(&source);
    let ty = analysis
        .type_at(at(&source, expression))
        .map(|ty| ty.to_string());
    (ty, analysis.facts().to_vec())
}

/// The type of `expression`, which must type without any fact.
fn ty(expression: &str) -> String {
    let (ty, facts) = type_of(expression);
    assert_eq!(facts, [], "{expression}");
    ty.unwrap_or_else(|| panic!("{expression} has no type"))
}

/// The one fact `expression` gets, which leaves it without a type.
fn only_fact(expression: &str) -> Fact {
    let (_, facts) = type_of(expression);
    match <[Fact; 1]>::try_from(facts) {
        Ok([fact]) => fact,
        Err(facts) => panic!("{expression}: expected one fact, got {facts:#?}"),
    }
}

/// Where the first `needle` in `expression` is, in `expression`'s `<box>`
/// source.
fn span(expression: &str, needle: &str) -> mesh_syntax::Span {
    nth_span(expression, needle, 0)
}

/// Where the `n`th (0-based) `needle` in `expression` is, in
/// `expression`'s `<box>` source.
fn nth_span(expression: &str, needle: &str, n: usize) -> mesh_syntax::Span {
    let source = format!("<box>{{{expression}}}</box>");
    let offset = source.find(expression).expect("the expression is there");
    let inner = common::nth(expression, needle, n);
    mesh_syntax::Span {
        start_byte: offset + inner.start_byte,
        end_byte: offset + inner.end_byte,
    }
}

fn mismatch(
    expectation: Expectation,
    expected: Ty,
    actual: Ty,
    absent: bool,
    at: mesh_syntax::Span,
) -> Fact {
    Fact::TypeMismatch {
        expectation,
        expected,
        actual,
        possibly_absent: absent,
        span: at,
    }
}

fn named(name: &str) -> Ty {
    Ty::Named(name.to_string())
}

fn opt(ty: Ty) -> Ty {
    Ty::Optional(Box::new(ty))
}

#[test]
fn literals() {
    assert_eq!(ty(r#""a""#), "string");
    assert_eq!(ty("1.5"), "number");
    assert_eq!(ty("true"), "boolean");
    assert_eq!(ty("null"), "null");
}

#[test]
fn references_have_their_scope_type() {
    assert_eq!(ty("user"), "User");
    assert_eq!(ty("maybeName"), "string?");
    assert_eq!(ty("users"), "list<User>");
    assert!(matches!(only_fact("usr"), Fact::UnknownReference { .. }));
}

/// D5: requiredness is erased on read. `f: T` reads as `T`; `f?: T`,
/// `f: T?` and `f?: T?` all read as `T?`, and an optional alias isn't
/// wrapped again.
#[test]
fn member_access_on_a_record_reads_the_field_without_its_requiredness() {
    assert_eq!(ty("user.name"), "string");
    assert_eq!(ty("user.avatar"), "string?");
    assert_eq!(ty("user.nick"), "string?");
    assert_eq!(ty("user.bio"), "string?");
    assert_eq!(ty("user.label"), "Maybe");
}

#[test]
fn member_access_on_a_record_needs_the_field() {
    assert_eq!(
        only_fact("user.nmae"),
        Fact::UnknownMember {
            object: named("User"),
            property: "nmae".to_string(),
            span: span("user.nmae", "nmae"),
            candidates: names(&["avatar", "bio", "label", "name", "nick"]),
        }
    );
}

#[test]
fn member_access_on_any_is_any() {
    assert_eq!(ty("anything.whatever.else"), "any");
}

/// MPRX has no optional chaining: reading a member of a value that may
/// be absent is an error at the value, `any?` included.
#[test]
fn member_access_on_a_possibly_absent_value_is_an_error() {
    assert_eq!(
        only_fact("maybeUser.name"),
        Fact::PossiblyAbsentAccess {
            object: opt(named("User")),
            property: "name".to_string(),
            span: span("maybeUser.name", "maybeUser"),
        }
    );
    assert!(matches!(
        only_fact("maybeAnything.x"),
        Fact::PossiblyAbsentAccess { .. }
    ));
    assert!(matches!(
        only_fact("user.avatar.length"),
        Fact::PossiblyAbsentAccess { .. }
    ));
}

#[test]
fn primitives_and_lists_have_no_members() {
    for (expression, object) in [
        ("name.length", Ty::String),
        ("users.length", Ty::List(Box::new(named("User")))),
    ] {
        assert_eq!(
            only_fact(expression),
            Fact::UnknownMember {
                object,
                property: "length".to_string(),
                span: span(expression, "length"),
                candidates: vec![],
            },
            "{expression}"
        );
    }
}

#[test]
fn unary_operators_need_and_give_their_type() {
    assert_eq!(ty("!flag"), "boolean");
    assert_eq!(ty("-count"), "number");
    assert_eq!(ty("!anything"), "boolean");
    let not = Expectation::Operand(Operator::Unary(UnaryOperator::Not));
    assert_eq!(
        type_of("!name"),
        (
            Some("boolean".to_string()),
            vec![mismatch(
                not.clone(),
                Ty::Boolean,
                Ty::String,
                false,
                span("!name", "name")
            )]
        )
    );
    assert_eq!(
        only_fact("!maybeFlag"),
        mismatch(
            not,
            Ty::Boolean,
            opt(Ty::Boolean),
            true,
            span("!maybeFlag", "maybeFlag")
        )
    );
    assert!(matches!(
        only_fact("-maybeAnything"),
        Fact::TypeMismatch {
            possibly_absent: true,
            ..
        }
    ));
    // Absence isn't the reason when the value wouldn't fit even if present.
    assert!(matches!(
        only_fact("-maybeName"),
        Fact::TypeMismatch {
            possibly_absent: false,
            ..
        }
    ));
}

#[test]
fn binary_operators_need_and_give_their_types() {
    assert_eq!(ty("count + 1 * 2 - 3 / 4 % 5"), "number");
    assert_eq!(ty("count < 1"), "boolean");
    assert_eq!(ty("count >= 1"), "boolean");
    assert_eq!(ty("flag && !flag || anything"), "boolean");
    // `+` is numeric only.
    let add = Expectation::Operand(Operator::Binary(BinaryOperator::Add));
    assert_eq!(
        type_of(r#"name + "!""#),
        (
            Some("number".to_string()),
            vec![
                mismatch(
                    add.clone(),
                    Ty::Number,
                    Ty::String,
                    false,
                    span(r#"name + "!""#, "name")
                ),
                mismatch(
                    add,
                    Ty::Number,
                    Ty::String,
                    false,
                    span(r#"name + "!""#, r#""!""#)
                ),
            ]
        )
    );
    assert!(matches!(
        only_fact("maybeFlag && flag"),
        Fact::TypeMismatch { .. }
    ));
}

/// Strict equality: the operands need a common type, with no coercion,
/// and `null` isn't absence.
#[test]
fn equality_needs_a_common_type() {
    assert_eq!(ty("name == maybeName"), "boolean");
    assert_eq!(ty("anything != 1"), "boolean");
    assert_eq!(ty("users == []"), "boolean");
    for (expression, left, right) in [
        ("name == 1", Ty::String, Ty::Number),
        ("maybeName == null", opt(Ty::String), Ty::Null),
    ] {
        assert_eq!(
            type_of(expression),
            (
                Some("boolean".to_string()),
                vec![Fact::NoCommonType {
                    combination: Combination::Equality(BinaryOperator::Eq),
                    left,
                    right,
                    span: span(expression, expression),
                }]
            ),
            "{expression}"
        );
    }
}

#[test]
fn a_conditional_needs_a_boolean_and_joins_its_branches() {
    assert_eq!(ty(r#"flag ? "a" : "b""#), "string");
    assert_eq!(ty("flag ? name : maybeName"), "string?");
    assert_eq!(ty("flag ? [] : users"), "list<User>");
    assert_eq!(
        type_of(r#"maybeFlag ? "a" : "b""#),
        (
            Some("string".to_string()),
            vec![mismatch(
                Expectation::Condition,
                Ty::Boolean,
                opt(Ty::Boolean),
                true,
                span(r#"maybeFlag ? "a" : "b""#, "maybeFlag")
            )]
        )
    );
    assert_eq!(
        only_fact("flag ? name : count"),
        Fact::NoCommonType {
            combination: Combination::Branches,
            left: Ty::String,
            right: Ty::Number,
            span: span("flag ? name : count", "flag ? name : count"),
        }
    );
}

#[test]
fn an_array_is_a_list_of_its_elements_common_type() {
    assert_eq!(ty("[]"), "list<nothing>");
    assert_eq!(ty("[1, count]"), "list<number>");
    assert_eq!(ty("[name, maybeName]"), "list<string?>");
    assert_eq!(ty("[[], [name]]"), "list<list<string>>");
    let expression = r#"[name, maybeName, 1, "a"]"#;
    assert_eq!(
        only_fact(expression),
        Fact::NoCommonType {
            combination: Combination::Elements,
            left: opt(Ty::String),
            right: Ty::Number,
            span: span(expression, "1"),
        }
    );
}

#[test]
fn an_object_is_an_exact_record_of_required_fields() {
    assert_eq!(
        ty(r#"{ a: 1, "b-c": maybeName, d: {} }"#),
        "{ a: number, b-c: string?, d: {} }"
    );
    // As with attributes, the last occurrence of a key counts.
    let expression = "{ x: usr, y: name, x: 1 }";
    assert_eq!(
        type_of(expression),
        (
            Some("{ x: number, y: string }".to_string()),
            vec![
                Fact::DuplicateObjectKey {
                    key: "x".to_string(),
                    span: span(expression, "x"),
                    last: nth_span(expression, "x", 1),
                },
                Fact::UnknownReference {
                    name: "usr".to_string(),
                    span: span(expression, "usr"),
                    candidates: names(&[
                        "anything",
                        "count",
                        "flag",
                        "maybeAnything",
                        "maybeFlag",
                        "maybeName",
                        "maybeUser",
                        "name",
                        "user",
                        "users",
                    ]),
                },
            ]
        )
    );
}

/// A handler's command is `void`; `$event` is the handled event's
/// payload type.
#[test]
fn a_handler_command_is_void_and_event_is_the_payload() {
    let source = "<box on.pick={take($event)} />";
    let analysis = analyze(source);
    assert_eq!(analysis.facts(), []);
    assert_eq!(
        analysis.type_at(at(source, "take($event)")),
        Some(&Ty::Void)
    );
    assert_eq!(analysis.type_at(at(source, "$event")), Some(&named("User")));
}

/// One mistake, one fact: an expression with an error has no type, and
/// nothing that depends on its type is checked. An operator still gives
/// its own result type.
#[test]
fn errors_do_not_cascade() {
    for expression in [
        "usr.name.first",
        "!usr",
        "usr == 1",
        "[usr, 1, count]",
        "flag ? usr : 1",
        "usr ? 1 : 2",
        "{ a: [usr] }",
        "user.nmae + 1",
        "maybeUser.name == 1",
    ] {
        let (_, facts) = type_of(expression);
        assert_eq!(facts.len(), 1, "{expression}: {facts:#?}");
    }
    assert_eq!(type_of("!usr").0.as_deref(), Some("boolean"));
    // An operator's own result still counts: negating a boolean is a
    // second, separate mistake.
    assert!(matches!(only_fact("-(!flag)"), Fact::TypeMismatch { .. }));
    assert_eq!(type_of("[usr, 1]").0, None);
    // Likewise, the other elements of an array are still compared with
    // each other.
    let expression = "[usr, 1, name]";
    let (ty, facts) = type_of(expression);
    assert_eq!(ty, None);
    assert!(matches!(facts[0], Fact::UnknownReference { .. }));
    assert_eq!(
        facts[1..],
        [Fact::NoCommonType {
            combination: Combination::Elements,
            left: Ty::Number,
            right: Ty::String,
            span: span(expression, "name"),
        }]
    );
}

/// Every expression that types gets its type recorded, operands before
/// the expression they're in.
#[test]
fn records_the_type_of_every_expression() {
    let source = "<box>{flag ? user.name : name}</box>";
    let analysis = analyze(source);
    let recorded: Vec<(&str, String)> = analysis
        .types()
        .iter()
        .map(|typed| {
            (
                &source[typed.span.start_byte..typed.span.end_byte],
                typed.ty.to_string(),
            )
        })
        .collect();
    assert_eq!(
        recorded,
        [
            ("flag", "boolean".to_string()),
            ("user", "User".to_string()),
            ("user.name", "string".to_string()),
            ("name", "string".to_string()),
            ("flag ? user.name : name", "string".to_string()),
        ]
    );
}
