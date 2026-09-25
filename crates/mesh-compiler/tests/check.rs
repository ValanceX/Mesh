//! The check operation (outline D3): the rules `mesh check` follows, as
//! compiler functions. `crates/mesh-cli/tests/fixtures.rs` shows that the
//! CLI, a host of this module, prints exactly what `check::run` renders.

use mesh_compiler::check::{self, Document, Model, ModelInput, Request};
use mesh_compiler::{compile, compile_with, CompileOptions};
use std::fs;
use std::path::{Path, PathBuf};

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn read(relative: &str) -> String {
    fs::read_to_string(examples().join(relative))
        .unwrap_or_else(|err| panic!("should read examples/{relative}: {err}"))
}

/// Every `.<extension>` file directly in `examples/<dir>`, relative to
/// `examples/`, sorted.
fn files(dir: &str, extension: &str) -> Vec<String> {
    let mut found: Vec<String> = fs::read_dir(examples().join(dir))
        .expect("the directory exists")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == extension))
        .map(|path| {
            let name = path.file_name().expect("a name").to_string_lossy();
            if dir.is_empty() {
                name.into_owned()
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

#[test]
fn without_a_model_it_is_compile() {
    for dir in ["", "fixtures/pass", "fixtures/fail"] {
        for file in files(dir, "mprx") {
            let text = read(&file);
            assert_eq!(
                check::source(&text, None),
                compile(&text).diagnostics,
                "{file}"
            );
        }
    }
}

#[test]
fn with_a_model_it_is_compile_with() {
    let mut runs = vec![(
        "examples".to_string(),
        "components.json",
        "users-page".to_string(),
        "users-page.mprx".to_string(),
    )];
    for dir in ["fixtures/check/pass", "fixtures/check/fail"] {
        for file in files(dir, "mprx") {
            runs.push((
                dir.to_string(),
                "fixtures/check/components.json",
                "template".to_string(),
                file,
            ));
        }
    }
    for (_, manifest, component, file) in runs {
        let manifest_text = read(manifest);
        let text = read(&file);
        let model = Model::load(&manifest_text, &component).expect("the model loads");
        let loaded = mesh_manifest::load(&manifest_text).expect("loads");
        let template = loaded.template(&component).expect("declared");
        assert_eq!(
            check::source(&text, Some(&model)),
            compile_with(&text, &CompileOptions::with_template(template)).diagnostics,
            "{file}"
        );
    }
}

#[test]
fn a_broken_manifest_reports_only_its_errors() {
    for file in files("fixtures/manifest/fail", "json") {
        let text = read(&file);
        // Every one of these is checked as `template`, as `mesh check`
        // checks them with `fixtures/manifest/template.mprx`. Some load
        // but don't declare it.
        let expected = match mesh_manifest::load(&text) {
            Err(errors) => errors,
            Ok(manifest) => vec![manifest.template("template").map(|_| ()).unwrap_err()],
        };
        let found = Model::load(&text, "template").unwrap_err();
        assert_eq!(found, expected, "{file}");
    }
}

#[test]
fn a_missing_component_is_reported_on_the_manifest() {
    let manifest = read("components.json");
    let found = Model::load(&manifest, "no-such-component").unwrap_err();
    let codes: Vec<&str> = found.iter().map(|d| d.code.as_str()).collect();
    assert_eq!(codes, ["manifest-missing-component"]);
}

#[test]
fn run_reports_on_the_manifest_or_the_source() {
    let manifest = read("components.json");
    let source = "<users-page><usr /></users-page>";

    let request = Request::new(source, "page.mprx").with_model(ModelInput::new(
        &manifest,
        "components.json",
        "users-page",
    ));
    let report = check::run(&request);
    assert_eq!(report.document, Document::Source);

    let missing = Request::new(source, "page.mprx").with_model(ModelInput::new(
        &manifest,
        "components.json",
        "nope",
    ));
    let report = check::run(&missing);
    assert_eq!(report.document, Document::Manifest);
    let document: serde_json::Value =
        serde_json::from_str(&report.render_json(&missing)).expect("JSON");
    assert_eq!(document["diagnostics"][0]["path"], "components.json");
}

#[test]
fn run_never_checks_the_source_after_a_manifest_error() {
    let request =
        Request::new("<a", "broken.mprx").with_model(ModelInput::new("{", "broken.json", "a"));
    let report = check::run(&request);
    assert_eq!(report.document, Document::Manifest);
    assert!(report
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code.as_str().starts_with("manifest-")));
}

#[test]
fn without_a_model_run_is_source() {
    let request = Request::new("<a></b>", "a.mprx");
    let report = check::run(&request);
    assert_eq!(report.document, Document::Source);
    assert_eq!(report.diagnostics, check::source("<a></b>", None));
    assert!(report.has_errors());
}

#[test]
fn warnings_alone_are_not_errors() {
    let report = check::run(&Request::new("<a x=\"1\" x=\"2\" />", "a.mprx"));
    assert!(!report.diagnostics.is_empty());
    assert!(!report.has_errors());
}
