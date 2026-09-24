//! `analyze_expression`: one expression, analyzed by the same walk as a
//! whole template, in value position.

mod common;

use common::at;
use mesh_analysis::{analyze_expression, Analysis, Fact, Target, Ty};
use mesh_semantic::{AttributeValue, Expression};

const MANIFEST: &str = include_str!("../../../examples/components.json");

/// The expression `text`, taken from the value of an attribute, with its
/// spans into `<x a={text} />`, which is returned too.
fn expression(text: &str) -> (String, Expression) {
    let source = format!("<x a={{{text}}} />");
    let ast = mesh_parser::parse(&source).ast.expect("the source parses");
    let ir = mesh_semantic::lower(&ast).ir.expect("the source lowers");
    let AttributeValue::Expression(expression) = ir.attributes[0].value.clone() else {
        panic!("an expression value");
    };
    (source, expression)
}

fn analyzed(text: &str) -> (String, Analysis) {
    let manifest = mesh_manifest::load(MANIFEST).expect("the example manifest loads");
    let template = manifest.template("users-page").expect("declared");
    let (source, expression) = expression(text);
    (source, analyze_expression(&expression, template))
}

#[test]
fn a_reference_resolves_and_types() {
    let (source, analysis) = analyzed("user");
    let span = at(&source, "user");
    assert_eq!(analysis.resolutions().len(), 1);
    assert_eq!(analysis.resolutions()[0].span, span);
    assert_eq!(
        analysis.resolutions()[0].target,
        Target::Scope("user".to_string())
    );
    assert_eq!(analysis.type_at(span), Some(&Ty::Named("User".to_string())));
    assert!(analysis.facts().is_empty());
}

#[test]
fn a_member_access_types_as_in_a_template() {
    let (source, analysis) = analyzed("user.name");
    assert_eq!(
        analysis.type_at(at(&source, "user.name")),
        Some(&Ty::String)
    );

    // The same type `analyze` gives it inside the users page.
    let page = include_str!("../../../examples/users-page.mprx");
    let whole = common::analyze(MANIFEST, "users-page", page);
    assert_eq!(whole.type_at(at(page, "user.name")), Some(&Ty::String));
}

#[test]
fn a_command_is_misplaced_in_value_position() {
    let (_, analysis) = analyzed("selectUser(user)");
    assert!(matches!(
        analysis.facts(),
        [Fact::CommandOutsideHandler { command, .. }] if command == "selectUser"
    ));
    assert!(analysis
        .resolutions()
        .iter()
        .all(|resolution| !matches!(resolution.target, Target::Command(_))));
}

#[test]
fn event_is_outside_a_handler() {
    let (_, analysis) = analyzed("$event");
    assert!(matches!(
        analysis.facts(),
        [Fact::EventValueOutsideHandler { .. }]
    ));
}
