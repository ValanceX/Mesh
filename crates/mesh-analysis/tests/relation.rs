//! `is_assignable` and `join`, one test per rule of outline D9, plus the
//! outline's own `any` / `any?` cases.

use mesh_analysis::{is_assignable, join, FieldTy, Ty};
use mesh_manifest::Manifest;
use std::collections::BTreeMap;

/// `Name` is an alias of `string`, `MaybeName` of `string?`, and `User` a
/// record type.
fn manifest() -> Manifest {
    mesh_manifest::load(
        r#"{ "version": 1, "components": {}, "types": {
            "Name": { "kind": "string" },
            "MaybeName": { "kind": "optional", "type": { "kind": "string" } },
            "User": { "kind": "record", "fields": {
                "name": { "type": { "kind": "named", "name": "Name" }, "required": true }
            } }
        } }"#,
    )
    .expect("the test manifest loads")
}

fn opt(ty: Ty) -> Ty {
    Ty::Optional(Box::new(ty))
}

fn list(ty: Ty) -> Ty {
    Ty::List(Box::new(ty))
}

fn named(name: &str) -> Ty {
    Ty::Named(name.to_string())
}

/// A record from `(name, type, required)` triples.
fn record(fields: &[(&str, Ty, bool)]) -> Ty {
    Ty::Record(
        fields
            .iter()
            .map(|(name, ty, required)| {
                (
                    name.to_string(),
                    FieldTy {
                        ty: ty.clone(),
                        required: *required,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>(),
    )
}

fn assignable(actual: &Ty, expected: &Ty) -> bool {
    is_assignable(&manifest(), actual, expected)
}

fn common(a: &Ty, b: &Ty) -> Option<Ty> {
    join(&manifest(), a, b)
}

#[test]
fn rule_1_void_is_compatible_with_nothing() {
    for other in [Ty::Any, opt(Ty::Any), Ty::Void, Ty::Nothing, Ty::String] {
        assert!(!assignable(&Ty::Void, &other), "void -> {other}");
        assert!(!assignable(&other, &Ty::Void), "{other} -> void");
    }
}

#[test]
fn rule_2_optionality_is_checked_first() {
    // A present value satisfies "may be absent".
    assert!(assignable(&Ty::String, &opt(Ty::String)));
    assert!(assignable(&opt(Ty::String), &opt(Ty::String)));
    // A possibly absent value satisfies only that.
    assert!(!assignable(&opt(Ty::String), &Ty::String));
    // `null` is a value, not absence.
    assert!(!assignable(&Ty::Null, &opt(Ty::String)));
    // Through aliases, too.
    assert!(!assignable(&named("MaybeName"), &named("Name")));
    assert!(assignable(&named("Name"), &named("MaybeName")));
}

/// The outline's `any` conformance cases: absence is never unchecked.
#[test]
fn rule_2_applies_to_any_too() {
    assert!(assignable(&Ty::Any, &Ty::String));
    assert!(!assignable(&opt(Ty::Any), &Ty::String));
    assert!(assignable(&opt(Ty::Any), &opt(Ty::String)));
    assert!(!assignable(&opt(Ty::String), &Ty::Any));
    assert!(assignable(&opt(Ty::String), &opt(Ty::Any)));
}

#[test]
fn rule_3_anything_present_fits_any() {
    for actual in [
        Ty::String,
        Ty::Null,
        Ty::Nothing,
        list(Ty::Number),
        named("User"),
    ] {
        assert!(assignable(&actual, &Ty::Any), "{actual} -> any");
    }
}

#[test]
fn rule_4_any_fits_anything_present() {
    for expected in [Ty::String, Ty::Null, list(Ty::Number), named("User")] {
        assert!(assignable(&Ty::Any, &expected), "any -> {expected}");
    }
}

#[test]
fn rule_5_nothing_fits_everything() {
    assert!(assignable(&Ty::Nothing, &Ty::String));
    assert!(assignable(&Ty::Nothing, &named("User")));
    assert!(assignable(&list(Ty::Nothing), &list(named("User"))));
}

#[test]
fn rule_6_primitives_match_exactly() {
    let primitives = [Ty::String, Ty::Number, Ty::Boolean, Ty::Null];
    for actual in &primitives {
        for expected in &primitives {
            assert_eq!(
                assignable(actual, expected),
                actual == expected,
                "{actual} -> {expected}"
            );
        }
    }
    assert!(assignable(&named("Name"), &Ty::String));
}

#[test]
fn rule_7_lists_are_covariant() {
    assert!(assignable(&list(Ty::String), &list(opt(Ty::String))));
    assert!(!assignable(&list(opt(Ty::String)), &list(Ty::String)));
    assert!(!assignable(&list(Ty::String), &list(Ty::Number)));
}

#[test]
fn rule_8_records_compare_declarations_exactly() {
    let t = || Ty::String;
    // A required field must be declared required, with a fitting type.
    let expected = record(&[("f", t(), true)]);
    assert!(assignable(&record(&[("f", t(), true)]), &expected));
    assert!(!assignable(&record(&[("f", t(), false)]), &expected));
    assert!(!assignable(&record(&[]), &expected));
    assert!(!assignable(&record(&[("f", Ty::Number, true)]), &expected));
    // An optional field may be left out, or declared either way.
    let expected = record(&[("f", t(), false)]);
    assert!(assignable(&record(&[]), &expected));
    assert!(assignable(&record(&[("f", t(), true)]), &expected));
    assert!(assignable(&record(&[("f", t(), false)]), &expected));
    // Exact: no field the expected record lacks.
    assert!(!assignable(
        &record(&[("f", t(), true), ("g", t(), true)]),
        &record(&[("f", t(), true)])
    ));
    // `f?: T` and `f: T?` aren't interchangeable, either way.
    assert!(!assignable(
        &record(&[("f", t(), false)]),
        &record(&[("f", opt(t()), true)])
    ));
    assert!(!assignable(
        &record(&[("f", opt(t()), true)]),
        &record(&[("f", t(), false)])
    ));
    // Named types are expanded, fields included.
    assert!(assignable(
        &record(&[("name", Ty::String, true)]),
        &named("User")
    ));
}

#[test]
fn rule_9_anything_else_fails() {
    assert!(!assignable(&list(Ty::String), &named("User")));
    assert!(!assignable(&Ty::String, &list(Ty::String)));
    assert!(!assignable(&named("User"), &Ty::String));
}

#[test]
fn join_rule_1_void_has_no_common_type() {
    for other in [Ty::Void, Ty::Any, Ty::Nothing, Ty::String] {
        assert_eq!(common(&Ty::Void, &other), None, "void, {other}");
        assert_eq!(common(&other, &Ty::Void), None, "{other}, void");
    }
}

#[test]
fn join_rule_2_optionality_is_kept() {
    assert_eq!(common(&opt(Ty::String), &Ty::String), Some(opt(Ty::String)));
    assert_eq!(common(&Ty::String, &opt(Ty::String)), Some(opt(Ty::String)));
    assert_eq!(common(&Ty::Any, &opt(Ty::String)), Some(opt(Ty::Any)));
    // `null` is not absence.
    assert_eq!(common(&opt(Ty::String), &Ty::Null), None);
}

#[test]
fn join_rule_3_nothing_adds_nothing() {
    assert_eq!(common(&Ty::Nothing, &named("User")), Some(named("User")));
    assert_eq!(common(&Ty::String, &Ty::Nothing), Some(Ty::String));
}

#[test]
fn join_rule_4_any_absorbs_present_types() {
    assert_eq!(common(&Ty::Any, &named("User")), Some(Ty::Any));
    assert_eq!(common(&Ty::Number, &Ty::Any), Some(Ty::Any));
}

#[test]
fn join_rule_5_identical_types_join_to_themselves() {
    assert_eq!(common(&Ty::String, &Ty::String), Some(Ty::String));
    assert_eq!(common(&named("Name"), &Ty::String), Some(named("Name")));
    let user = record(&[("name", Ty::String, true)]);
    assert_eq!(common(&user, &named("User")), Some(user.clone()));
    // `{f?: T}` and `{f: T?}` aren't identical.
    assert_eq!(
        common(
            &record(&[("f", Ty::String, false)]),
            &record(&[("f", opt(Ty::String), true)])
        ),
        None
    );
}

#[test]
fn join_rule_6_lists_join_element_wise() {
    assert_eq!(
        common(&list(Ty::Nothing), &list(Ty::String)),
        Some(list(Ty::String))
    );
    assert_eq!(
        common(&list(Ty::String), &list(opt(Ty::String))),
        Some(list(opt(Ty::String)))
    );
    assert_eq!(common(&list(Ty::String), &list(Ty::Number)), None);
}

#[test]
fn join_rule_7_anything_else_has_no_common_type() {
    assert_eq!(common(&Ty::String, &Ty::Number), None);
    assert_eq!(common(&Ty::Null, &Ty::String), None);
    assert_eq!(common(&list(Ty::String), &Ty::String), None);
}

/// `join` never invents compatibility that `is_assignable` would deny:
/// both inputs are assignable to their join, whenever there is one.
#[test]
fn both_sides_are_assignable_to_their_join() {
    let types = [
        Ty::String,
        Ty::Number,
        Ty::Null,
        Ty::Any,
        Ty::Nothing,
        opt(Ty::String),
        opt(Ty::Any),
        named("Name"),
        named("MaybeName"),
        named("User"),
        list(Ty::Nothing),
        list(Ty::String),
        list(opt(Ty::String)),
        record(&[("f", Ty::String, false)]),
    ];
    for a in &types {
        for b in &types {
            if let Some(joined) = common(a, b) {
                assert!(assignable(a, &joined), "{a} -> join({a}, {b}) = {joined}");
                assert!(assignable(b, &joined), "{b} -> join({a}, {b}) = {joined}");
            }
        }
    }
}

#[test]
fn types_display_in_the_docs_notation() {
    assert_eq!(opt(list(named("User"))).to_string(), "list<User>?");
    assert_eq!(
        record(&[
            ("name", Ty::String, true),
            ("avatar", opt(Ty::String), false)
        ])
        .to_string(),
        "{ avatar?: string?, name: string }"
    );
    assert_eq!(record(&[]).to_string(), "{}");
    assert_eq!(list(Ty::Nothing).to_string(), "list<nothing>");
    assert_eq!(Ty::Void.to_string(), "void");
}
