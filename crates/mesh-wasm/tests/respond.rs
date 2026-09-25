//! The safe core of the WebAssembly build is the check operation, and
//! nothing else: for every corpus run `mesh check` makes, `respond` is
//! exactly `check::run(..).render_json(..)`. (The same process and the
//! same serializer, so this compares bytes; the cross-host agreement,
//! between the JS API and the CLI, compares parsed documents instead.)

use mesh_compiler::check::{self, ModelInput, Request};
use mesh_wasm::respond;
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

/// A run: a file, and optionally a manifest and component.
type Run = (String, Option<(String, String)>);

/// The corpus `crates/mesh-cli/tests/fixtures.rs` runs, with every
/// component explicit.
fn runs() -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    for dir in ["", "fixtures/pass", "fixtures/fail"] {
        runs.extend(files(dir, "mprx").into_iter().map(|file| (file, None)));
    }
    for dir in ["fixtures/check/pass", "fixtures/check/fail"] {
        runs.extend(files(dir, "mprx").into_iter().map(|file| {
            let model = ("fixtures/check/components.json".into(), "template".into());
            (file, Some(model))
        }));
    }
    for file in files("", "mprx") {
        let component = match file.as_str() {
            "user-card.mprx" => "user-card-example".to_string(),
            other => other.trim_end_matches(".mprx").to_string(),
        };
        runs.push((file, Some(("components.json".into(), component))));
    }
    for manifest in files("fixtures/manifest/fail", "json") {
        let model = (manifest, "template".to_string());
        runs.push(("fixtures/manifest/template.mprx".into(), Some(model)));
    }
    runs
}

#[test]
fn respond_is_the_check_operation() {
    let runs = runs();
    assert!(runs.len() >= 90, "only {} runs", runs.len());
    for (file, model) in runs {
        let source = read(&file);
        let manifest = model.as_ref().map(|(path, _)| read(path));
        let mut request = Request::new(&source, &file);
        if let (Some((path, component)), Some(text)) = (&model, &manifest) {
            request = request.with_model(ModelInput::new(text, path, component));
        }
        let expected = check::run(&request).render_json(&request);
        let model = match (&model, &manifest) {
            (Some((path, component)), Some(text)) => {
                Some((text.as_str(), path.as_str(), component.as_str()))
            }
            _ => None,
        };
        assert_eq!(respond(&source, &file, model), expected, "{file}");
    }
}

#[test]
fn a_run_without_a_model_needs_no_manifest() {
    assert_eq!(
        respond("<a></b>", "a.mprx", None),
        check::run(&Request::new("<a></b>", "a.mprx"))
            .render_json(&Request::new("<a></b>", "a.mprx"))
    );
}

/// `respond_compile` is exactly `check::template`, rendered: the
/// diagnostics document against the source, and the template (or null).
#[test]
fn respond_compile_is_the_compile_operation() {
    let manifest = read("fixtures/check/components.json");
    let model = check::Model::load(&manifest, "template").expect("the model loads");
    let mut runs = files("fixtures/check/pass", "mprx");
    runs.extend(files("fixtures/check/fail", "mprx"));
    for file in runs {
        let source = read(&file);
        let compiled = check::template(&source, &model);
        let template = compiled
            .template
            .as_ref()
            .map_or_else(|| "null".to_string(), mesh_template::to_json);
        let expected = format!(
            "{{\"diagnostics\":{},\"template\":{template}}}",
            mesh_compiler::render_json(&source, &file, &compiled.diagnostics)
        );
        assert_eq!(
            mesh_wasm::respond_compile(&source, &file, &manifest, "components.json", "template"),
            expected,
            "{file}"
        );
    }
}

#[test]
fn respond_compile_reports_a_broken_manifest_against_it() {
    let result: serde_json::Value = serde_json::from_str(&mesh_wasm::respond_compile(
        "<page />",
        "page.mprx",
        "{ not json",
        "broken.json",
        "page",
    ))
    .expect("the result is JSON");
    assert_eq!(result["template"], serde_json::Value::Null);
    assert_eq!(
        result["diagnostics"]["diagnostics"][0]["path"],
        "broken.json"
    );
    assert_eq!(
        result["diagnostics"]["diagnostics"][0]["code"],
        "manifest-syntax-error"
    );
}
