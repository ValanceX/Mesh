//! `mesh compile` (v0.5 D2): the check `mesh check` makes, and, only when
//! it finds no error, the template.

use assert_cmd::Command;
use mesh_compiler::check;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

/// Every `.mprx` directly in `dir` (relative to `examples/`), sorted.
fn sources(dir: &str) -> Vec<String> {
    let mut files: Vec<String> = fs::read_dir(examples().join(dir))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "mprx"))
        .map(|path| {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if dir.is_empty() {
                name
            } else {
                format!("{dir}/{name}")
            }
        })
        .collect();
    files.sort();
    files
}

/// Every corpus run with a model: (file, manifest, component), all
/// relative to `examples/`, as `fixtures.rs` runs them.
fn runs() -> Vec<(String, &'static str, String)> {
    let mut runs = Vec::new();
    for file in sources("") {
        let stem = file.trim_end_matches(".mprx").to_string();
        let component = if stem == "user-card" {
            "user-card-example".into()
        } else {
            stem
        };
        runs.push((file, "components.json", component));
    }
    for dir in ["fixtures/check/pass", "fixtures/check/fail"] {
        for file in sources(dir) {
            runs.push((
                file,
                "fixtures/check/components.json",
                "template".to_string(),
            ));
        }
    }
    runs
}

fn mesh(args: &[&str]) -> Output {
    Command::cargo_bin("mesh")
        .unwrap()
        .current_dir(examples())
        .args(args)
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// The template `check::template` gives for a run, as `mesh compile`
/// writes it.
fn library_template(file: &str, manifest: &str, component: &str) -> Option<String> {
    let model = check::Model::load(
        &fs::read_to_string(examples().join(manifest)).unwrap(),
        component,
    )
    .unwrap();
    let source = fs::read_to_string(examples().join(file)).unwrap();
    check::template(&source, &model)
        .template
        .map(|template| mesh_template::to_json(&template) + "\n")
}

#[test]
fn compile_agrees_with_check_and_writes_a_template_exactly_when_it_is_clean() {
    let dir = tempfile::tempdir().unwrap();
    let mut written = 0;
    for (file, manifest, component) in runs() {
        let common = ["--model", manifest, "--component", &component];
        let check = mesh(&[&["check", &file][..], &common].concat());
        let expected = library_template(&file, manifest, &component);
        assert_eq!(expected.is_some(), check.status.success(), "{file}");

        // Human, to stdout.
        let compile = mesh(&[&["compile", &file][..], &common].concat());
        assert_eq!(compile.status.code(), check.status.code(), "{file}");
        assert_eq!(
            text(&compile.stderr),
            text(&check.stderr),
            "{file}: the same diagnostics"
        );
        match &expected {
            Some(template) => assert_eq!(&text(&compile.stdout), template, "{file}"),
            None => assert!(compile.stdout.is_empty(), "{file}: no template"),
        }

        // JSON, to a file.
        let output = dir
            .path()
            .join(format!("{}.template.json", file.replace('/', "_")));
        let output_arg = output.to_string_lossy().into_owned();
        let check_json = mesh(&[&["check", &file, "--format", "json"][..], &common].concat());
        let compile_json = mesh(
            &[
                &[
                    "compile",
                    &file,
                    "--format",
                    "json",
                    "--output",
                    &output_arg,
                ][..],
                &common,
            ]
            .concat(),
        );
        assert_eq!(
            compile_json.status.code(),
            check_json.status.code(),
            "{file}"
        );
        assert_eq!(
            text(&compile_json.stdout),
            text(&check_json.stdout),
            "{file}"
        );
        match &expected {
            Some(template) => {
                assert_eq!(&fs::read_to_string(&output).unwrap(), template, "{file}");
                written += 1;
            }
            None => assert!(!output.exists(), "{file}: no file for a source with errors"),
        }
    }
    assert!(written >= 20);
}

#[test]
fn output_writes_the_file_and_nothing_to_stdout() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("page.template.json");
    let result = mesh(&[
        "compile",
        "users-page.mprx",
        "--model",
        "components.json",
        "--output",
        &output.to_string_lossy(),
    ]);
    assert!(result.status.success());
    assert!(result.stdout.is_empty() && result.stderr.is_empty());
    let template = mesh_template::from_json(&fs::read_to_string(&output).unwrap()).unwrap();
    assert_eq!(template.component, "users-page");
}

#[test]
fn a_source_with_errors_leaves_an_existing_output_alone() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("out.json");
    fs::write(&output, "previous").unwrap();
    let result = mesh(&[
        "compile",
        "fixtures/check/fail/content-not-text.mprx",
        "--model",
        "fixtures/check/components.json",
        "--component",
        "template",
        "--output",
        &output.to_string_lossy(),
    ]);
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(fs::read_to_string(&output).unwrap(), "previous");
}

#[test]
fn json_without_output_is_a_usage_error() {
    let result = mesh(&[
        "compile",
        "users-page.mprx",
        "--model",
        "components.json",
        "--format",
        "json",
    ]);
    assert_eq!(result.status.code(), Some(2), "clap's usage error");
    assert!(text(&result.stderr).contains("--output"));
}

#[test]
fn compile_needs_a_model() {
    let result = mesh(&["compile", "users-page.mprx"]);
    assert_eq!(result.status.code(), Some(2));
    assert!(text(&result.stderr).contains("--model"));
}

#[test]
fn a_broken_manifest_is_reported_as_check_reports_it() {
    let mut manifests: Vec<PathBuf> = fs::read_dir(examples().join("fixtures/manifest/fail"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    manifests.sort();
    assert!(!manifests.is_empty());
    for path in manifests {
        let manifest = format!(
            "fixtures/manifest/fail/{}",
            path.file_name().unwrap().to_string_lossy()
        );
        let check = mesh(&["check", "users-page.mprx", "--model", &manifest]);
        let compile = mesh(&["compile", "users-page.mprx", "--model", &manifest]);
        assert_eq!(compile.status.code(), Some(1), "{manifest}");
        assert_eq!(text(&compile.stderr), text(&check.stderr), "{manifest}");
        assert!(compile.stdout.is_empty(), "{manifest}");
    }
}

/// The CLI manual's `mesh compile` example is what `mesh` prints, apart
/// from the `compiler` version, which changes with every release.
#[test]
fn the_cli_manuals_compile_example_is_what_mesh_prints() {
    let manual = fs::read_to_string(examples().join("../docs/manual/mesh-cli.md")).unwrap();
    let mut lines = manual.lines();
    let command = lines
        .find_map(|line| line.strip_prefix("$ mesh compile "))
        .expect("the manual shows a compile example");
    let printed = lines.next().expect("the example shows its output");
    let args: Vec<&str> = command.split_whitespace().collect();
    let output = Command::cargo_bin("mesh")
        .unwrap()
        .current_dir(examples().join(".."))
        .arg("compile")
        .args(&args)
        .output()
        .unwrap();
    assert!(output.status.success());
    let parse = |text: &str| -> serde_json::Value {
        let mut value: serde_json::Value = serde_json::from_str(text).expect("JSON");
        value["compiler"] = serde_json::Value::Null;
        value
    };
    assert_eq!(parse(&text(&output.stdout)), parse(printed));
}
