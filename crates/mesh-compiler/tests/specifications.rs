//! Keeps v0.5's specified formats honest before any code implements them
//! (v0.5 Pass 0, Decision 1): each new schema is a valid draft 2020-12
//! schema, every example document a manual shows validates against it,
//! and every counter-example the manual shows is rejected.
//!
//! A manual marks its examples by the info string of a fenced block:
//! ```` ```json template-v1 ```` is a valid `template-v1` document, and
//! ```` ```json template-v1-invalid ```` one the schema must reject. As
//! the formats gain their implementations (v0.5 Passes 1–3), these tests
//! move to the crates that own them.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &str) -> String {
    let path = root().join(path);
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("should read {}: {err}", path.display()))
}

fn validator(schema: &str) -> jsonschema::Validator {
    let schema: Value = serde_json::from_str(&read(schema)).expect("the schema is JSON");
    jsonschema::draft202012::new(&schema).expect("the schema is a valid 2020-12 schema")
}

/// A validator for one definition of `schema`: the schema with its
/// top-level constraints replaced by a reference to `$defs/<name>`.
fn definition_validator(schema: &str, name: &str) -> jsonschema::Validator {
    let mut schema: Value = serde_json::from_str(&read(schema)).expect("the schema is JSON");
    let object = schema.as_object_mut().expect("the schema is an object");
    for key in ["type", "properties", "required", "additionalProperties"] {
        object.remove(key);
    }
    object.insert("$ref".into(), Value::String(format!("#/$defs/{name}")));
    jsonschema::draft202012::new(&schema).expect("the definition is a valid 2020-12 schema")
}

/// The text of every fenced block in `doc` whose info string is exactly
/// `json <tag>`.
fn blocks(doc: &str, tag: &str) -> Vec<String> {
    let info = format!("```json {tag}");
    let mut found = Vec::new();
    let mut current: Option<String> = None;
    for line in read(doc).lines() {
        match &mut current {
            None if line.trim_end() == info => current = Some(String::new()),
            None => {}
            Some(_) if line.trim_end() == "```" => found.push(current.take().unwrap()),
            Some(text) => {
                text.push_str(line);
                text.push('\n');
            }
        }
    }
    assert!(current.is_none(), "an unclosed `{info}` block in {doc}");
    found
}

/// Every `tag` block of `doc` is valid against `schema`, and every
/// `tag-invalid` block isn't. Each kind must have at least `minimum`
/// blocks, so an example can't vanish unnoticed.
fn check_examples(doc: &str, schema: &str, tag: &str, minimum: (usize, usize)) {
    check_examples_with(doc, &validator(schema), tag, minimum);
}

fn check_examples_with(
    doc: &str,
    validator: &jsonschema::Validator,
    tag: &str,
    minimum: (usize, usize),
) {
    let valid = blocks(doc, tag);
    let invalid = blocks(doc, &format!("{tag}-invalid"));
    assert!(
        valid.len() >= minimum.0,
        "{doc} should show at least {} `{tag}` examples",
        minimum.0
    );
    assert!(
        invalid.len() >= minimum.1,
        "{doc} should show at least {} `{tag}-invalid` counter-examples",
        minimum.1
    );
    for (index, text) in valid.iter().enumerate() {
        let document: Value = serde_json::from_str(text)
            .unwrap_or_else(|err| panic!("{doc}'s `{tag}` example {index} isn't JSON: {err}"));
        let errors: Vec<String> = validator
            .iter_errors(&document)
            .map(|e| e.to_string())
            .collect();
        assert!(
            errors.is_empty(),
            "{doc}'s `{tag}` example {index} is invalid: {errors:#?}"
        );
    }
    for (index, text) in invalid.iter().enumerate() {
        let document: Value = serde_json::from_str(text).unwrap_or_else(|err| {
            panic!("{doc}'s `{tag}-invalid` example {index} isn't JSON: {err}")
        });
        assert!(
            !validator.is_valid(&document),
            "{doc}'s `{tag}-invalid` example {index} should be rejected"
        );
    }
}

#[test]
fn template_v1_is_a_valid_schema() {
    validator("schemas/template-v1.schema.json");
}

#[test]
fn the_template_manuals_examples_match_template_v1() {
    check_examples(
        "docs/manual/templates.md",
        "schemas/template-v1.schema.json",
        "template-v1",
        (1, 4),
    );
}

#[test]
fn the_template_manuals_models_are_valid_manifests() {
    let validator = validator("schemas/manifest-v1.schema.json");
    let models = blocks("docs/manual/templates.md", "manifest");
    assert!(!models.is_empty());
    for text in models {
        let document: Value = serde_json::from_str(&text).expect("the model is JSON");
        assert!(
            validator.is_valid(&document),
            "a model in the manual is invalid"
        );
        mesh_manifest::load(&text).expect("the model loads");
    }
}

#[test]
fn render_v1_is_a_valid_schema() {
    validator("schemas/render-v1.schema.json");
}

#[test]
fn the_runtime_manuals_trees_match_render_v1() {
    check_examples(
        "docs/manual/runtime.md",
        "schemas/render-v1.schema.json",
        "render-v1",
        (1, 3),
    );
}

#[test]
fn the_runtime_manuals_intents_match_the_intent_definition() {
    check_examples_with(
        "docs/manual/runtime.md",
        &definition_validator("schemas/render-v1.schema.json", "intent"),
        "intent",
        (1, 1),
    );
}
