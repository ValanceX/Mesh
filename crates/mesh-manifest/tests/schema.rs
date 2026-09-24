//! Keeps the published JSON Schema (`schemas/manifest-v1.schema.json`)
//! and the loader in step. The schema must accept every manifest the
//! loader accepts, and reject every broken-manifest fixture whose mistake
//! JSON Schema can express. The mistakes it can't express (repeated keys,
//! which JSON parsers drop; repeated parameter names; unresolved,
//! recursive or doubly optional named types; a missing component) are the
//! loader's alone, and the schema must accept those fixtures, so the
//! boundary stays where the schema's description says it is.

use std::fs;
use std::path::{Path, PathBuf};

fn repo(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|err| panic!("should read {}: {err}", path.display()))
}

fn validator() -> jsonschema::Validator {
    let schema = serde_json::from_str(&read(&repo("schemas/manifest-v1.schema.json")))
        .expect("the schema is JSON");
    jsonschema::draft202012::new(&schema).expect("the schema is a valid 2020-12 schema")
}

/// Codes for mistakes the schema also rejects.
const SCHEMA_CODES: [&str; 6] = [
    "manifest-unsupported-version",
    "manifest-invalid-value",
    "manifest-missing-property",
    "manifest-unknown-property",
    "manifest-unknown-kind",
    "manifest-invalid-name",
];

/// Codes for mistakes only the loader can find.
const LOADER_CODES: [&str; 6] = [
    "manifest-duplicate-key",
    "manifest-duplicate-parameter",
    "manifest-unknown-type",
    "manifest-recursive-type",
    "manifest-nested-optional",
    "manifest-missing-component",
];

#[test]
fn the_schema_accepts_the_example_manifest() {
    let source = read(&repo("examples/components.json"));
    let instance = serde_json::from_str(&source).expect("the example is JSON");
    let errors: Vec<String> = validator()
        .iter_errors(&instance)
        .map(|error| error.to_string())
        .collect();
    assert!(errors.is_empty(), "{errors:#?}");
    assert!(mesh_manifest::load(&source).is_ok());
}

#[test]
fn the_schema_agrees_with_every_broken_manifest_fixture() {
    let validator = validator();
    let directory = repo("examples/fixtures/manifest/fail");
    let mut checked = 0;
    for entry in fs::read_dir(&directory).expect("should read the fixtures") {
        let path = entry.expect("a directory entry").path();
        if path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        let stderr = read(&path.with_extension("stderr"));
        let code = stderr
            .strip_prefix("error[")
            .and_then(|rest| rest.split(']').next())
            .expect("a fixture's output starts with error[code]");
        let instance = match serde_json::from_str::<serde_json::Value>(&read(&path)) {
            Ok(instance) => instance,
            // `serde_json`'s own recursion limit is the loader's nesting
            // limit, so it can't read a manifest nested too deeply either.
            Err(error) if error.to_string().starts_with("recursion limit exceeded") => {
                assert_eq!(code, "manifest-nesting-too-deep", "{}", path.display());
                continue;
            }
            Err(_) => {
                assert_eq!(code, "manifest-syntax-error", "{}", path.display());
                continue;
            }
        };
        if SCHEMA_CODES.contains(&code) {
            assert!(
                !validator.is_valid(&instance),
                "the schema should reject {} ({code})",
                path.display()
            );
        } else {
            assert!(
                LOADER_CODES.contains(&code),
                "{}: unexpected {code}",
                path.display()
            );
            assert!(
                validator.is_valid(&instance),
                "{} ({code}) is only the loader's to reject, but the schema rejects it",
                path.display()
            );
        }
        checked += 1;
    }
    // Guards against a moved directory making the test pass vacuously.
    assert!(checked >= 12, "only {checked} fixtures were checked");
}

/// The manifest reference's first example is a manifest that both the
/// loader and the schema accept.
#[test]
fn the_manifest_references_example_is_valid() {
    let reference = read(&repo("docs/manual/manifest.md"));
    let example = reference
        .split("```json\n")
        .nth(1)
        .and_then(|block| block.split("```").next())
        .expect("the reference opens with an example manifest");
    mesh_manifest::load(example).expect("the loader accepts the example");
    let example: serde_json::Value = serde_json::from_str(example).expect("the example is JSON");
    assert!(validator().is_valid(&example));
}
