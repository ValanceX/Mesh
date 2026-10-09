//! The shape and place of a `mesh-switch`: `invalid-switch`, one code, for each way a switch can be wrong.

use mesh_compiler::{compile_with, CompileOptions};

const MODEL: &str = r#"{
  "version": 1, "types": {},
  "components": {
    "panel": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "note":  { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "mesh-if": { "props": { "when": { "type": { "kind": "boolean" }, "required": true } }, "events": {}, "commands": {}, "scope": {} },
    "mesh-slot": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "mesh-switch":  { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "mesh-default": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "mesh-case": { "props": { "when": { "type": { "kind": "boolean" }, "required": true } }, "events": {}, "commands": {}, "scope": {} },
    "view": { "props": {}, "events": {}, "commands": {}, "scope": { "flag": { "kind": "boolean" } } }
  }
}"#;

fn codes(source: &str) -> Vec<String> {
    let manifest = mesh_manifest::load(MODEL).unwrap();
    let result = compile_with(
        source,
        &CompileOptions::with_template(manifest.template("view").unwrap()),
    );

    result
        .diagnostics
        .iter()
        .map(|d| d.code.as_str().to_string())
        .collect()
}

fn invalid(source: &str) -> usize {
    codes(source)
        .iter()
        .filter(|code| *code == "invalid-switch")
        .count()
}

#[test]
fn a_well_formed_switch_is_clean() {
    assert!(codes("<panel><mesh-switch><mesh-case when={flag}><note>a</note></mesh-case><mesh-case when={!flag}><note>b</note></mesh-case><mesh-default><note>c</note></mesh-default></mesh-switch></panel>").is_empty());
    assert!(codes("<panel><mesh-switch><mesh-case when={flag}><note>a</note></mesh-case></mesh-switch></panel>").is_empty());
}

#[test]
fn each_way_a_switch_can_be_wrong_is_invalid_switch() {
    let wrong = [
        ("no cases", "<panel><mesh-switch></mesh-switch></panel>"),
        ("a stray child", "<panel><mesh-switch><note>x</note><mesh-case when={flag}><note>a</note></mesh-case></mesh-switch></panel>"),
        ("text", "<panel><mesh-switch>hello<mesh-case when={flag}><note>a</note></mesh-case></mesh-switch></panel>"),
        ("a case after the default", "<panel><mesh-switch><mesh-case when={flag}><note>a</note></mesh-case><mesh-default><note>b</note></mesh-default><mesh-case when={flag}><note>c</note></mesh-case></mesh-switch></panel>"),
        ("two defaults", "<panel><mesh-switch><mesh-case when={flag}><note>a</note></mesh-case><mesh-default><note>b</note></mesh-default><mesh-default><note>c</note></mesh-default></mesh-switch></panel>"),
        ("a case with two bodies", "<panel><mesh-switch><mesh-case when={flag}><note>a</note><note>b</note></mesh-case></mesh-switch></panel>"),
        ("a case with text", "<panel><mesh-switch><mesh-case when={flag}>a</mesh-case></mesh-switch></panel>"),
        ("a conditional body", "<panel><mesh-switch><mesh-case when={flag}><mesh-if when={flag}><note>a</note></mesh-if></mesh-case></mesh-switch></panel>"),
        ("a slot body", "<panel><mesh-switch><mesh-case when={flag}><mesh-slot /></mesh-case></mesh-switch></panel>"),
        ("a case outside a switch", "<panel><mesh-case when={flag}><note>a</note></mesh-case></panel>"),
        ("a default outside a switch", "<panel><mesh-default><note>a</note></mesh-default></panel>"),
        ("the root", "<mesh-switch><mesh-case when={flag}><note>a</note></mesh-case></mesh-switch>"),
        ("the body of a conditional", "<panel><mesh-if when={flag}><mesh-switch><mesh-case when={flag}><note>a</note></mesh-case></mesh-switch></mesh-if></panel>"),
    ];

    for (what, source) in wrong {
        assert!(invalid(source) >= 1, "{what}: {:?}", codes(source));
    }
}

#[test]
fn a_case_without_a_boolean_when_is_the_ordinary_prop_error() {
    assert!(codes(
        "<panel><mesh-switch><mesh-case><note>a</note></mesh-case></mesh-switch></panel>"
    )
    .contains(&"missing-required-prop".to_string()));
    assert!(codes("<panel><mesh-switch><mesh-case when=\"x\"><note>a</note></mesh-case></mesh-switch></panel>").contains(&"type-mismatch".to_string()));
}
