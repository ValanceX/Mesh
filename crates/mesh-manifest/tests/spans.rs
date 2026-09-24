//! Declaration spans: every declaration a template can name, located at
//! its key in the manifest's text.

use mesh_manifest::{load, Declaration, Manifest, Type};
use std::fs;
use std::path::Path;

fn example() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/components.json");
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("should read {}: {err}", path.display()))
}

/// The text `declaration`'s span covers in `source`.
fn text<'s>(manifest: &Manifest, source: &'s str, declaration: Declaration<'_>) -> &'s str {
    let span = manifest
        .span_of(declaration)
        .unwrap_or_else(|| panic!("{declaration:?} has no span"));
    &source[span.start_byte..span.end_byte]
}

fn quoted(name: &str) -> String {
    format!("{name:?}")
}

#[test]
fn every_declaration_in_the_example_has_its_key_span() {
    let source = example();
    let manifest = load(&source).expect("the example loads");
    let mut checked = 0;

    for (name, ty) in manifest.types() {
        assert_eq!(
            text(&manifest, &source, Declaration::NamedType(name)),
            quoted(name)
        );
        checked += 1;
        if let Type::Record(fields) = ty {
            for field in fields.keys() {
                let declaration = Declaration::Field { ty: name, field };
                assert_eq!(text(&manifest, &source, declaration), quoted(field));
                checked += 1;
            }
        }
    }

    for (component, declared) in manifest.components() {
        assert_eq!(
            text(&manifest, &source, Declaration::Component(component)),
            quoted(component)
        );
        for prop in declared.props.keys() {
            let declaration = Declaration::Prop { component, prop };
            assert_eq!(text(&manifest, &source, declaration), quoted(prop));
        }
        for event in declared.events.keys() {
            let declaration = Declaration::Event { component, event };
            assert_eq!(text(&manifest, &source, declaration), quoted(event));
        }
        for (command, declared) in &declared.commands {
            let declaration = Declaration::Command { component, command };
            assert_eq!(text(&manifest, &source, declaration), quoted(command));
            for (index, parameter) in declared.parameters.iter().enumerate() {
                let declaration = Declaration::Parameter {
                    component,
                    command,
                    index,
                };
                // A parameter has no key: its span is its "name" value.
                assert_eq!(
                    text(&manifest, &source, declaration),
                    quoted(&parameter.name)
                );
                checked += 1;
            }
        }
        for name in declared.scope.keys() {
            let declaration = Declaration::Scope { component, name };
            assert_eq!(text(&manifest, &source, declaration), quoted(name));
        }
        checked += 1;
    }
    // Guards against an example that stops exercising the kinds.
    assert!(checked >= 10, "only {checked} declarations were checked");
}

#[test]
fn an_undeclared_name_has_no_span() {
    let manifest = load(&example()).expect("the example loads");
    assert_eq!(manifest.span_of(Declaration::Component("nope")), None);
    assert_eq!(
        manifest.span_of(Declaration::Prop {
            component: "avatar",
            prop: "nope"
        }),
        None
    );
    assert_eq!(
        manifest.span_of(Declaration::Parameter {
            component: "users-page",
            command: "selectUser",
            index: 1
        }),
        None
    );
}

#[test]
fn a_byte_order_mark_shifts_every_span_by_three() {
    let source = example();
    let with_bom = format!("\u{feff}{source}");
    let plain = load(&source).expect("the example loads");
    let marked = load(&with_bom).expect("the example loads with a BOM");
    let declaration = Declaration::Component("avatar");
    let (a, b) = (
        plain.span_of(declaration).unwrap(),
        marked.span_of(declaration).unwrap(),
    );
    assert_eq!(b.start_byte, a.start_byte + 3);
    assert_eq!(b.end_byte, a.end_byte + 3);
}

/// Only a named record type's own fields can be addressed: a record
/// nested inside another type has no name to find it by.
#[test]
fn fields_of_anonymous_records_are_not_addressable() {
    let source = r#"{ "version": 1, "components": {}, "types": {
        "Rows": { "kind": "list", "element": { "kind": "record", "fields": {
            "a": { "type": { "kind": "string" }, "required": true } } } }
    } }"#;
    let manifest = load(source).expect("the manifest loads");
    assert_eq!(
        manifest.span_of(Declaration::Field {
            ty: "Rows",
            field: "a"
        }),
        None
    );
}
