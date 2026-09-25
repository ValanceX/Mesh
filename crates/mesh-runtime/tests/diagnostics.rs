//! The runtime diagnostics document (docs/manual/runtime.md,
//! "Diagnostics"): its codes, each with one location form, and the
//! document they render to.

use mesh_runtime::{to_json, Form, Location, PathSegment, RuntimeCode, RuntimeDiagnostic};
use serde_json::Value;
use std::fs;
use std::path::Path;

fn read(relative: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(relative),
    )
    .unwrap_or_else(|err| panic!("should read {relative}: {err}"))
}

fn validator() -> jsonschema::Validator {
    let schema: Value =
        serde_json::from_str(&read("schemas/runtime-diagnostics-v1.schema.json")).unwrap();
    jsonschema::draft202012::new(&schema).expect("the schema is valid")
}

/// Every assembly and runtime code Pass 0 specified (Decision 27), with
/// the one location form the diagnostics reference gives it.
#[test]
fn the_codes_are_pass_0s_each_with_its_manuals_form() {
    let reference = read("docs/manual/diagnostics.md");
    let mut documented = Vec::new();
    for section in reference.split("\n### `").skip(1) {
        let code = section.split('`').next().unwrap().to_string();
        if !(code.starts_with("assembly-") || code.starts_with("runtime-")) {
            continue;
        }
        let body = section.split("\n### ").next().unwrap();
        let form = body
            .lines()
            .find_map(|line| line.strip_prefix("Location: `")?.split('`').next())
            .unwrap()
            .to_string();
        documented.push((code, form));
    }
    documented.sort();
    let mut ours: Vec<(String, String)> = RuntimeCode::ALL
        .iter()
        .map(|code| (code.as_str().to_string(), code.form().as_str().to_string()))
        .collect();
    ours.sort();
    assert_eq!(ours, documented);
}

fn one_of_each() -> Vec<RuntimeDiagnostic> {
    let span = mesh_template::Span {
        start: mesh_template::Offset { byte: 3, utf16: 3 },
        end: mesh_template::Offset { byte: 7, utf16: 6 },
    };
    vec![
        RuntimeDiagnostic::new(RuntimeCode::MISSING_ROOT, "no root", Location::Program),
        RuntimeDiagnostic::new(
            RuntimeCode::FINGERPRINT_MISMATCH,
            "another model",
            Location::Template {
                index: 1,
                component: Some("card".into()),
            },
        ),
        RuntimeDiagnostic::new(
            RuntimeCode::MALFORMED_TEMPLATE,
            "not JSON",
            Location::Template {
                index: 0,
                component: None,
            },
        ),
        RuntimeDiagnostic::new(
            RuntimeCode::NON_FINITE_OUTPUT,
            "infinity",
            Location::Source {
                component: "card".into(),
                span,
            },
        ),
        RuntimeDiagnostic::new(
            RuntimeCode::VALUE_MISMATCH,
            "wrong",
            Location::Input(vec![
                PathSegment::Name("users".into()),
                PathSegment::Index(2),
            ]),
        ),
        RuntimeDiagnostic::new(
            RuntimeCode::UNKNOWN_HANDLER,
            "no handler",
            Location::Handler,
        ),
    ]
}

#[test]
fn every_location_form_renders_to_a_valid_document() {
    let document: Value = serde_json::from_str(&to_json(&one_of_each(), "")).unwrap();
    let errors: Vec<String> = validator()
        .iter_errors(&document)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{errors:#?}");
    let kinds: Vec<&str> = document["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["location"]["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        ["program", "template", "template", "source", "input", "handler"]
    );
    assert_eq!(
        document["diagnostics"][4]["location"]["path"],
        serde_json::json!(["users", 2])
    );
    assert!(document["diagnostics"][2]["location"]
        .get("component")
        .is_none());
}

/// A manifest diagnostic has the positions `mesh check --format json`
/// gives it: the same `SourceMap`, over the manifest's text.
#[test]
fn a_model_diagnostic_has_the_diagnostics_documents_positions() {
    let manifest = "{\n  \"version\": 1,\n  \"types\": {},\n  \"components\": { \"é😀\": 3 }\n}";
    let diagnostics = mesh_manifest::load(manifest).expect_err("the manifest is broken");
    let ours: Vec<RuntimeDiagnostic> = diagnostics
        .iter()
        .map(RuntimeDiagnostic::from_manifest)
        .collect();
    let document: Value = serde_json::from_str(&to_json(&ours, manifest)).unwrap();
    assert!(validator().is_valid(&document));
    let check: Value = serde_json::from_str(&mesh_compiler::render_json(
        manifest,
        "m.json",
        &diagnostics,
    ))
    .unwrap();
    for (ours, theirs) in document["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .zip(check["diagnostics"].as_array().unwrap())
    {
        assert_eq!(ours["code"], theirs["code"]);
        assert_eq!(ours["location"]["kind"], "model");
        assert_eq!(ours["location"]["span"], theirs["span"]);
    }
}

#[test]
fn every_diagnostic_is_an_error() {
    let document: Value = serde_json::from_str(&to_json(&one_of_each(), "")).unwrap();
    for diagnostic in document["diagnostics"].as_array().unwrap() {
        assert_eq!(diagnostic["severity"], "error");
    }
    let forms: Vec<Form> = one_of_each().iter().map(|d| d.location.form()).collect();
    assert_eq!(forms.len(), 6);
}
