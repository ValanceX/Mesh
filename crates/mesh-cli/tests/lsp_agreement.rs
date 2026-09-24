//! The language server publishes exactly what `mesh check` reports
//! (outline invariant I3, and the Definition of Done's "Canonical
//! diagnostics agree with the CLI").
//!
//! For every run the fixture corpus makes, the same file is opened in an
//! in-memory `mesh-lsp`, configured with the same model and, explicitly,
//! the component the CLI checked it as. Its publication must match
//! `mesh check --format json` diagnostic for diagnostic: severity, code,
//! message, range and suggestions. It runs once per position encoding, and
//! `fixtures/check/fail/unknown-reference-after-emoji.mprx` makes UTF-8,
//! UTF-16 and character columns all differ.
//!
//! One difference is by design (outline D9): when the manifest is broken,
//! `mesh check` reports only the manifest's errors, while the server
//! publishes them on the manifest and checks the document without a
//! model, exactly as `mesh check` without `--model` does. Both halves are
//! compared.

use assert_cmd::Command;
use mesh_compiler::{ColumnUnit, LineColumn, SourceMap};
use mesh_lsp::testing::{file_uri, Client, Publication};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .canonicalize()
        .expect("examples/ exists")
}

/// Every `.<extension>` file directly in `examples/<dir>`, relative to
/// `examples/`, sorted.
fn files(dir: &str, extension: &str) -> Vec<String> {
    let mut found: Vec<String> = fs::read_dir(examples_dir().join(dir))
        .expect("the directory exists")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == extension))
        .map(|path| {
            let name = path
                .file_name()
                .expect("a name")
                .to_string_lossy()
                .into_owned();
            if dir.is_empty() {
                name
            } else {
                format!("{dir}/{name}")
            }
        })
        .collect();
    found.sort();
    assert!(
        !found.is_empty(),
        "examples/{dir} has no .{extension} files"
    );
    found
}

/// One corpus run: a file, and the model and component `mesh check`
/// checks it against.
struct Run {
    file: String,
    model: Option<String>,
    component: Option<String>,
}

/// The component `mesh check` uses when `--component` isn't given: the
/// file's stem. The CLI derives it; the server is told it explicitly.
fn stem(file: &str) -> String {
    Path::new(file)
        .file_stem()
        .expect("a stem")
        .to_string_lossy()
        .into_owned()
}

/// Every run `fixtures.rs` makes, with each component made explicit.
fn runs() -> Vec<Run> {
    let mut runs = Vec::new();
    for dir in ["", "fixtures/pass", "fixtures/fail"] {
        for file in files(dir, "mprx") {
            runs.push(Run {
                file,
                model: None,
                component: None,
            });
        }
    }
    for dir in ["fixtures/check/pass", "fixtures/check/fail"] {
        for file in files(dir, "mprx") {
            runs.push(Run {
                file,
                model: Some("fixtures/check/components.json".into()),
                component: Some("template".into()),
            });
        }
    }
    for file in files("", "mprx") {
        let component = match file.as_str() {
            "user-card.mprx" => "user-card-example".to_string(),
            other => stem(other),
        };
        runs.push(Run {
            file,
            model: Some("components.json".into()),
            component: Some(component),
        });
    }
    for manifest in files("fixtures/manifest/fail", "json") {
        runs.push(Run {
            file: "fixtures/manifest/template.mprx".into(),
            model: Some(manifest),
            component: Some("template".into()),
        });
    }
    runs
}

/// `mesh check --format json` for `run`: the diagnostics, and the path
/// they're reported against.
fn cli(run: &Run) -> (String, Vec<Value>) {
    let mut command = Command::cargo_bin("mesh").expect("the mesh binary");
    command
        .current_dir(examples_dir())
        .args(["check", "--format", "json"]);
    if let Some(model) = &run.model {
        command.args(["--model", model]);
    }
    if let Some(component) = &run.component {
        command.args(["--component", component]);
    }
    let output = command.arg(&run.file).output().expect("mesh check runs");
    let document: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|err| panic!("{}: stdout isn't JSON: {err}", run.file));
    let diagnostics = document["diagnostics"].as_array().expect("a list").clone();
    let path = diagnostics
        .first()
        .and_then(|diagnostic| diagnostic["path"].as_str())
        .unwrap_or(&run.file)
        .to_string();
    (path, diagnostics)
}

/// `byte` as the source map normalizes it: floored to a character and
/// clamped to its line's visible text. LSP positions can only say that
/// much, so the CLI's raw offsets are compared in this form.
fn normalized(source: &str, map: &SourceMap, byte: usize) -> usize {
    let at = map.line_column(source, byte, ColumnUnit::Utf8);
    map.offset(source, at, ColumnUnit::Utf8)
}

/// The byte span an LSP range covers.
fn span_of(source: &str, map: &SourceMap, unit: ColumnUnit, range: &Value) -> (usize, usize) {
    let offset = |position: &Value| {
        let at = LineColumn {
            line: position["line"].as_u64().expect("a line") as usize,
            column: position["character"].as_u64().expect("a character") as usize,
        };
        map.offset(source, at, unit)
    };
    (offset(&range["start"]), offset(&range["end"]))
}

fn json_span(source: &str, map: &SourceMap, span: &Value) -> (usize, usize) {
    let byte = |position: &Value| {
        let byte = position["byte"].as_u64().expect("a byte") as usize;
        normalized(source, map, byte)
    };
    (byte(&span["start"]), byte(&span["end"]))
}

/// Mismatches between the CLI's diagnostics and a publication, for
/// `source` in `unit`.
fn compare(
    what: &str,
    source: &str,
    unit: ColumnUnit,
    cli: &[Value],
    published: &Publication,
) -> Vec<String> {
    let map = SourceMap::new(source);
    let lsp: Vec<Value> = published
        .diagnostics
        .iter()
        .map(|diagnostic| serde_json::to_value(diagnostic).expect("serializes"))
        .collect();
    if cli.len() != lsp.len() {
        return vec![format!(
            "{what}: mesh check reports {} diagnostics, the server {}: {cli:#?} vs {lsp:#?}",
            cli.len(),
            lsp.len()
        )];
    }
    let mut mismatches = Vec::new();
    for (index, (expected, actual)) in cli.iter().zip(&lsp).enumerate() {
        let severity = match expected["severity"].as_str() {
            Some("error") => 1,
            Some("warning") => 2,
            other => panic!("unexpected severity {other:?}"),
        };
        let checks = [
            ("severity", json!(severity), actual["severity"].clone()),
            ("code", expected["code"].clone(), actual["code"].clone()),
            (
                "message",
                expected["message"].clone(),
                actual["message"].clone(),
            ),
            (
                "range",
                json!(json_span(source, &map, &expected["span"])),
                json!(span_of(source, &map, unit, &actual["range"])),
            ),
        ];
        for (field, want, got) in checks {
            if want != got {
                mismatches.push(format!("{what} #{index} {field}: {want} vs {got}"));
            }
        }
    }
    mismatches
}

/// Mismatches between the CLI's suggestions and the server's quick fixes
/// for the document `uri`.
fn compare_fixes(
    what: &str,
    client: &mut Client,
    uri: &str,
    source: &str,
    unit: ColumnUnit,
    cli: &[Value],
    published: &Publication,
) -> Vec<String> {
    let map = SourceMap::new(source);
    let mut mismatches = Vec::new();
    for (expected, diagnostic) in cli.iter().zip(&published.diagnostics) {
        let range = serde_json::to_value(diagnostic.range).expect("serializes");
        let response = client.request(
            "textDocument/codeAction",
            json!({
                "textDocument": { "uri": uri },
                "range": range,
                "context": { "diagnostics": [] }
            }),
        );
        let actions = response.response_result.expect("code actions");
        // Only the actions that fix this diagnostic: others may touch the
        // same range.
        let fixes: Vec<(String, (usize, usize))> = actions
            .as_array()
            .expect("a list")
            .iter()
            .filter(|action| {
                action["diagnostics"][0]["message"] == expected["message"]
                    && action["diagnostics"][0]["range"] == range
            })
            .map(|action| {
                let edit = &action["edit"]["documentChanges"][0]["edits"][0];
                (
                    edit["newText"].as_str().expect("new text").to_string(),
                    span_of(source, &map, unit, &edit["range"]),
                )
            })
            .collect();
        let wanted: Vec<(String, (usize, usize))> = expected["suggestions"]
            .as_array()
            .expect("suggestions")
            .iter()
            .map(|suggestion| {
                (
                    suggestion["replacement"]
                        .as_str()
                        .expect("a replacement")
                        .to_string(),
                    json_span(source, &map, &suggestion["span"]),
                )
            })
            .collect();
        if fixes != wanted {
            mismatches.push(format!("{what}: suggestions {wanted:?} vs fixes {fixes:?}"));
        }
    }
    mismatches
}

fn agree(encoding: &str, unit: ColumnUnit) {
    let root = examples_dir();
    let mut mismatches = Vec::new();
    let mut checked = 0;
    for run in runs() {
        let what = format!(
            "{} (model {:?}, component {:?}, {encoding})",
            run.file, run.model, run.component
        );
        let mut settings = json!({});
        if let Some(model) = &run.model {
            settings["model"] = json!(model);
        }
        if let Some(component) = &run.component {
            settings["components"] = json!({ run.file.clone(): component });
        }
        let (mut client, result) = Client::initialized(json!({
            "processId": null,
            "rootUri": file_uri(&root),
            "capabilities": { "general": { "positionEncodings": [encoding] } },
            "initializationOptions": settings,
        }));
        assert_eq!(result["capabilities"]["positionEncoding"], encoding);

        let source = fs::read_to_string(root.join(&run.file)).expect("the fixture reads");
        let uri = file_uri(&root.join(&run.file));
        client.open(&uri, 1, &source);
        let document = client.next_publication(&uri);

        let (path, cli_diagnostics) = cli(&run);
        if path == run.file {
            mismatches.extend(compare(&what, &source, unit, &cli_diagnostics, &document));
            mismatches.extend(compare_fixes(
                &what,
                &mut client,
                &uri,
                &source,
                unit,
                &cli_diagnostics,
                &document,
            ));
        } else {
            // The manifest's errors: published on the manifest...
            let model = run.model.as_deref().expect("a model");
            assert_eq!(path, model, "{what}");
            let manifest_source = fs::read_to_string(root.join(model)).expect("the manifest reads");
            let manifest_uri = file_uri(&root.join(model));
            let published = client.latest_publication(&manifest_uri, Duration::from_millis(50));
            mismatches.extend(compare(
                &format!("{what} manifest"),
                &manifest_source,
                unit,
                &cli_diagnostics,
                &published,
            ));
            // ...and the document checked as `mesh check` without a model.
            let (_, modelless) = cli(&Run {
                file: run.file.clone(),
                model: None,
                component: None,
            });
            mismatches.extend(compare(
                &format!("{what} without a model"),
                &source,
                unit,
                &modelless,
                &document,
            ));
        }
        checked += 1;
    }
    assert!(checked >= 80, "only {checked} runs were compared");
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn every_fixture_publishes_what_mesh_check_reports_in_utf16() {
    agree("utf-16", ColumnUnit::Utf16);
}

#[test]
fn every_fixture_publishes_what_mesh_check_reports_in_utf8() {
    agree("utf-8", ColumnUnit::Utf8);
}

/// Every prefix of the users page, as someone types it: each is sent as a
/// change to one open document, and each publication must be exactly what
/// `mesh check --format json` reports for that prefix. Most prefixes have
/// syntax errors, so the server computes editor recovery for them (outline
/// D3); this shows recovery never adds to or changes what is published
/// (invariants I2 and I3).
#[test]
fn every_prefix_publishes_what_mesh_check_reports() {
    let workspace = tempfile::tempdir().expect("a temp dir");
    let root = workspace
        .path()
        .canonicalize()
        .expect("the temp dir exists");
    fs::copy(
        examples_dir().join("components.json"),
        root.join("components.json"),
    )
    .expect("the manifest copies");
    let (mut client, _) = Client::initialized(json!({
        "processId": null,
        "rootUri": file_uri(&root),
        "capabilities": {},
        "initializationOptions": {
            "model": "components.json",
            "components": { "users-page.mprx": "users-page" }
        },
    }));
    let full = fs::read_to_string(examples_dir().join("users-page.mprx")).expect("reads");
    let path = root.join("users-page.mprx");
    let uri = file_uri(&path);
    let mut mismatches = Vec::new();
    let ends: Vec<usize> = full
        .char_indices()
        .map(|(end, _)| end)
        .chain([full.len()])
        .collect();
    for (version, end) in ends.into_iter().enumerate() {
        let source = &full[..end];
        let version = i32::try_from(version).expect("a small version");
        if version == 0 {
            client.open(&uri, version, source);
        } else {
            client.change(&uri, version, source);
        }
        let published = client.next_publication(&uri);
        assert_eq!(published.version, Some(version));

        fs::write(&path, source).expect("the prefix writes");
        let output = Command::cargo_bin("mesh")
            .expect("the mesh binary")
            .current_dir(&root)
            .args(["check", "--format", "json", "--model", "components.json"])
            .args(["--component", "users-page", "users-page.mprx"])
            .output()
            .expect("mesh check runs");
        let document: Value = serde_json::from_slice(&output.stdout).expect("JSON");
        let cli = document["diagnostics"].as_array().expect("a list").clone();
        mismatches.extend(compare(
            &format!("prefix of {end} bytes"),
            source,
            ColumnUnit::Utf16,
            &cli,
            &published,
        ));
    }
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}
