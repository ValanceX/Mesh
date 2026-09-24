//! Offset queries: what an editor asks when the cursor is at a byte.
//! Analyzed against the real example manifest, as `users-page`.

mod common;

use mesh_analysis::{Analysis, Fact, Target, Ty};
use std::fs;
use std::path::Path;

fn example(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name);
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("should read {}: {err}", path.display()))
}

fn users_page(source: &str) -> Analysis {
    common::analyze(&example("components.json"), "users-page", source)
}

/// The byte `offset` characters into the first occurrence of `needle`.
fn inside(source: &str, needle: &str, offset: usize) -> usize {
    common::at(source, needle).start_byte + offset
}

#[test]
fn a_member_access_is_typed_at_its_property() {
    let source = example("users-page.mprx");
    let analysis = users_page(&source);
    // The `n` of `user.name`: inside the member access, not the reference.
    let offset = inside(&source, "user.name", 5);
    let typed = analysis.typed_at(offset).expect("a typed expression");
    assert_eq!(
        &source[typed.span.start_byte..typed.span.end_byte],
        "user.name"
    );
    assert_eq!(typed.ty, Ty::String);
    assert_eq!(analysis.resolution_at(offset), None);
}

#[test]
fn a_reference_is_resolved_and_typed_at_its_name() {
    let source = example("users-page.mprx");
    let analysis = users_page(&source);
    let offset = inside(&source, "user.name", 0);
    let resolution = analysis.resolution_at(offset).expect("a resolution");
    assert_eq!(resolution.target, Target::Scope("user".to_string()));
    let typed = analysis.typed_at(offset).expect("a typed expression");
    assert_eq!(&source[typed.span.start_byte..typed.span.end_byte], "user");
    assert_eq!(typed.ty, Ty::Named("User".to_string()));
}

#[test]
fn a_prop_is_resolved_at_its_name() {
    let source = example("users-page.mprx");
    let analysis = users_page(&source);
    let resolution = analysis
        .resolution_at(inside(&source, "alt=", 1))
        .expect("a resolution");
    assert_eq!(
        resolution.target,
        Target::Prop {
            component: "avatar".to_string(),
            prop: "alt".to_string()
        }
    );
}

#[test]
fn the_end_of_a_name_still_finds_it() {
    let source = example("users-page.mprx");
    let analysis = users_page(&source);
    let resolution = analysis
        .resolution_at(inside(&source, "selectUser", "selectUser".len()))
        .expect("a resolution");
    assert_eq!(resolution.target, Target::Command("selectUser".to_string()));
}

#[test]
fn nested_expressions_give_the_innermost() {
    let source = example("users-page.mprx");
    let analysis = users_page(&source);
    let typed = analysis
        .typed_at(inside(&source, "user.active", 6))
        .expect("a typed expression");
    assert_eq!(
        &source[typed.span.start_byte..typed.span.end_byte],
        "user.active"
    );
    assert_eq!(typed.ty, Ty::Boolean);
}

#[test]
fn an_offset_in_text_finds_nothing() {
    let source = example("users-page.mprx");
    let analysis = users_page(&source);
    let offset = inside(&source, "Select<", 2);
    assert_eq!(analysis.resolution_at(offset), None);
    assert_eq!(analysis.typed_at(offset), None);
    assert_eq!(analysis.facts_at(offset).count(), 0);
}

#[test]
fn facts_at_returns_every_containing_fact_in_order() {
    // `selectUser` takes one argument: the invocation has an arity
    // mismatch, and `usr` inside it is an unknown reference.
    let source = r#"<page title="x"><button on.click={selectUser(usr, 1)} /></page>"#;
    let analysis = users_page(source);
    let facts: Vec<&Fact> = analysis.facts_at(inside(source, "usr", 1)).collect();
    assert!(
        matches!(
            facts.as_slice(),
            [
                Fact::CommandArityMismatch { .. },
                Fact::UnknownReference { .. }
            ]
        ),
        "{facts:#?}"
    );
    // Outside `usr`, only the arity mismatch contains the offset.
    let facts: Vec<&Fact> = analysis.facts_at(inside(source, ", 1", 2)).collect();
    assert!(
        matches!(facts.as_slice(), [Fact::CommandArityMismatch { .. }]),
        "{facts:#?}"
    );
}
