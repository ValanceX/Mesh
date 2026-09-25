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

#[test]
fn runtime_diagnostics_v1_is_a_valid_schema() {
    validator("schemas/runtime-diagnostics-v1.schema.json");
}

#[test]
fn the_runtime_manuals_diagnostics_match_runtime_diagnostics_v1() {
    check_examples(
        "docs/manual/runtime.md",
        "schemas/runtime-diagnostics-v1.schema.json",
        "runtime-diagnostics",
        (5, 1),
    );
}

/// Every code v0.5 adds (Pass 0, Decision 27), by family.
const CHECK_CODES: [&str; 2] = ["content-not-text", "number-literal-out-of-range"];
const RUNTIME_CODES: [&str; 30] = [
    "assembly-malformed-template",
    "assembly-unsupported-format-version",
    "assembly-fingerprint-mismatch",
    "assembly-duplicate-template",
    "assembly-missing-root",
    "assembly-unbound-scope-name",
    "assembly-unsound-binding",
    "assembly-cycle",
    "assembly-composite-event",
    "assembly-composite-children",
    "runtime-missing-value",
    "runtime-value-mismatch",
    "runtime-unknown-field",
    "runtime-absent-element",
    "runtime-number-out-of-range",
    "runtime-non-finite-input",
    "runtime-unpaired-surrogate",
    "runtime-unsupported-value",
    "runtime-unexpected-payload",
    "runtime-handler-other-program",
    "runtime-unknown-handler",
    "runtime-operand-mismatch",
    "runtime-not-a-record",
    "runtime-missing-member",
    "runtime-content-not-text",
    "runtime-prop-mismatch",
    "runtime-argument-mismatch",
    "runtime-non-finite-output",
    "runtime-absent-element-output",
    "runtime-key-collision",
];

/// Until the code that emits them exists, v0.5's runtime and assembly
/// codes are documented in the runtime manual, not in the diagnostics
/// reference, which must list exactly the codes MESH can emit
/// (`crates/mesh-cli/tests/diagnostics_reference.rs`). The check codes
/// moved there in Pass 1; Pass 2 moves the rest.
#[test]
fn every_new_code_is_documented_once() {
    let runtime = read("docs/manual/runtime.md");
    let reference = read("docs/manual/diagnostics.md");
    let spec = read("docs/MPRX-SPEC.md");
    let headings: Vec<&str> = runtime
        .lines()
        .filter_map(|line| line.strip_prefix("### `")?.strip_suffix('`'))
        .collect();
    let mut expected = RUNTIME_CODES.to_vec();
    expected.sort_unstable();
    let mut found = headings.clone();
    found.sort_unstable();
    assert_eq!(
        found, expected,
        "runtime.md should have one heading per runtime and assembly code"
    );
    // The check codes exist since v0.5 Pass 1, so they are in the
    // reference (which `diagnostics_reference.rs` keeps exact), and §9.7
    // names them.
    for code in CHECK_CODES {
        assert!(
            spec.contains(&format!("`{code}`")),
            "§9.7 should name `{code}`"
        );
        assert!(
            reference.contains(&format!("### `{code}`")),
            "`{code}` should be in the diagnostics reference"
        );
    }
    for code in RUNTIME_CODES {
        assert!(
            !reference.contains(&format!("### `{code}`")),
            "`{code}` is in the diagnostics reference before MESH can emit it"
        );
    }
}

/// The six location forms of a runtime diagnostic (Pass 0, Decision 28).
const LOCATION_FORMS: [&str; 6] = ["handler", "input", "model", "program", "source", "template"];

fn sorted(mut items: Vec<String>) -> Vec<String> {
    items.sort_unstable();
    items
}

#[test]
fn the_schema_has_exactly_the_six_location_forms() {
    let schema: Value =
        serde_json::from_str(&read("schemas/runtime-diagnostics-v1.schema.json")).expect("JSON");
    let forms: Vec<String> = schema["$defs"]["location"]["oneOf"]
        .as_array()
        .expect("`location` is a oneOf")
        .iter()
        .map(|form| {
            form["properties"]["kind"]["const"]
                .as_str()
                .expect("a kind")
                .to_string()
        })
        .collect();
    assert_eq!(sorted(forms), LOCATION_FORMS.map(String::from));
}

#[test]
fn the_runtime_manual_defines_exactly_the_six_location_forms() {
    let runtime = read("docs/manual/runtime.md");
    let table = runtime
        .split("### Identity and locations")
        .nth(1)
        .and_then(|rest| rest.split("### Order").next())
        .expect("runtime.md has an \"Identity and locations\" section");
    let forms: Vec<String> = table
        .lines()
        .filter_map(|line| {
            line.strip_prefix("| `")?
                .split('`')
                .next()
                .map(String::from)
        })
        .filter(|form| form != "kind")
        .collect();
    assert_eq!(sorted(forms), LOCATION_FORMS.map(String::from));
}

/// Every code's entry names exactly one location form, and it is one of
/// the six.
#[test]
fn every_runtime_code_has_exactly_one_location_form() {
    let runtime = read("docs/manual/runtime.md");
    let mut sections = runtime.split("\n### `").skip(1);
    let mut seen = 0;
    for section in sections.by_ref() {
        let code = section.split('`').next().unwrap_or_default();
        let body = section.split("\n### ").next().unwrap_or_default();
        let forms: Vec<&str> = body
            .lines()
            .filter_map(|line| line.strip_prefix("Location: `")?.split('`').next())
            .collect();
        assert_eq!(
            forms.len(),
            1,
            "`{code}` should state exactly one location: {forms:?}"
        );
        assert!(
            LOCATION_FORMS.contains(&forms[0]),
            "`{code}` has an unknown location form"
        );
        seen += 1;
    }
    assert_eq!(seen, RUNTIME_CODES.len());
}

#[test]
fn every_location_form_has_an_example() {
    let used: Vec<String> = blocks("docs/manual/runtime.md", "runtime-diagnostics")
        .iter()
        .flat_map(|text| {
            let document: Value = serde_json::from_str(text).expect("JSON");
            document["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .iter()
                .map(|d| d["location"]["kind"].as_str().expect("a kind").to_string())
                .collect::<Vec<_>>()
        })
        .collect();
    let mut used = sorted(used);
    used.dedup();
    assert_eq!(used, LOCATION_FORMS.map(String::from));
}
