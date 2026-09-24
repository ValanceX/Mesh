//! Corpus-driven integration tests: runs the real `mesh` binary over the
//! `.mprx` files in the workspace's `examples/` directory.
//!
//! - `examples/*.mprx` — clean examples; must check clean (exit 0,
//!   `no errors`, empty stderr). `CANONICAL_EXAMPLES` must be among them.
//! - `examples/fixtures/pass/*.mprx` — must exit 0 with `no errors`;
//!   stderr (warnings) must match the sibling `.stderr` file.
//! - `examples/fixtures/fail/*.mprx` — must exit 1 with empty stdout;
//!   stderr must match the sibling `.stderr` file.
//! - `examples/*.mprx` again, checked against the example component
//!   manifest `examples/components.json` — must check clean too, and the
//!   manifest must declare every component those files name (see
//!   `example_manifest_declares_what_the_examples_name`).
//! - `examples/fixtures/manifest/fail/*.json` — broken manifests. Checking
//!   `fixtures/manifest/template.mprx` against each must exit 1 with empty
//!   stdout; stderr must match the sibling `.stderr` file.
//! - `examples/fixtures/check/{pass,fail}/*.mprx` — checked against the
//!   manifest `fixtures/check/components.json`, each as the template of its
//!   `template` component. Pass fixtures must check clean (exit 0,
//!   `no errors`, empty stderr); fail fixtures must exit 1 with empty
//!   stdout, and stderr must match the sibling `.stderr` file.
//!
//! Discovery is deliberately non-recursive: only files directly inside
//! each of those directories are found, and files in nested
//! subdirectories are ignored. `mesh check` runs with `examples/` as its
//! working directory and a relative path, so `-->` lines in `.stderr`
//! files read like `fixtures/fail/empty.mprx` wherever the repo is
//! checked out. To add a fixture, place a `.mprx` + `.stderr` pair
//! directly in `fixtures/pass/` or `fixtures/fail/` — no code changes
//! needed.

use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

/// The required v0.1 canonical examples, directly inside `examples/`.
/// Named explicitly so deleting or renaming one fails loudly rather than
/// silently shrinking the corpus.
const CANONICAL_EXAMPLES: [&str; 2] = ["user-card.mprx", "users-page.mprx"];

fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

/// The example component manifest, relative to `examples/`.
const EXAMPLE_MODEL: &str = "components.json";

/// The component each example is the template of, where it isn't the
/// file's stem. `user-card.mprx` is a usage snippet whose root is a
/// `<user-card>` instance, not the `user-card` template.
const EXAMPLE_COMPONENTS: [(&str, &str); 1] = [("user-card.mprx", "user-card-example")];

/// The file every broken-manifest fixture is checked with.
const MANIFEST_TEMPLATE: &str = "fixtures/manifest/template.mprx";

/// The manifest every `fixtures/check/` file is checked against, as the
/// template of its `template` component.
const CHECK_ARGS: [&str; 4] = [
    "--model",
    "fixtures/check/components.json",
    "--component",
    "template",
];

/// Every `.mprx` file directly inside `examples/<relative_dir>` (not
/// recursive), as a path relative to `examples/`, sorted so failures
/// report in a stable order.
fn mprx_files(relative_dir: &str) -> Vec<PathBuf> {
    files(relative_dir, "mprx")
}

/// Every `.<extension>` file directly inside `examples/<relative_dir>`,
/// as [`mprx_files`] finds `.mprx` files.
fn files(relative_dir: &str, extension: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(examples_dir().join(relative_dir))
        .unwrap_or_else(|err| panic!("should read examples/{relative_dir}: {err}"))
        .map(|entry| entry.expect("should read a directory entry").path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == extension))
        .map(|path| Path::new(relative_dir).join(path.file_name().expect("has a file name")))
        .collect();
    files.sort();
    // Guards against a moved/renamed directory making the test pass vacuously.
    assert!(
        !files.is_empty(),
        "examples/{relative_dir} should contain at least one .{extension} file"
    );
    files
}

/// Runs `mesh check <extra_args...> <relative_path>` in `examples/`.
fn check(relative_path: &Path, extra_args: &[&str]) -> Output {
    Command::cargo_bin("mesh")
        .unwrap()
        .current_dir(examples_dir())
        .arg("check")
        .args(extra_args)
        .arg(relative_path)
        .output()
        .expect("should run mesh check")
}

/// Checks every file in `relative_dir`, collecting all mismatches before
/// failing so one run reports the whole corpus, not just the first miss.
/// `expected_stderr` returning an empty string means "stderr must be empty".
fn assert_corpus(
    relative_dir: &str,
    extra_args: &[&str],
    expected_code: i32,
    expected_stderr: impl Fn(&Path) -> String,
) {
    let runs = mprx_files(relative_dir)
        .into_iter()
        .map(|file| {
            let output = check(&file, extra_args);
            (file, output)
        })
        .collect();
    assert_runs(runs, expected_code, expected_stderr);
}

/// Checks each `(file, output)` run the way [`assert_corpus`] does.
fn assert_runs(
    runs: Vec<(PathBuf, Output)>,
    expected_code: i32,
    expected_stderr: impl Fn(&Path) -> String,
) {
    let expected_stdout = if expected_code == 0 {
        "no errors\n"
    } else {
        ""
    };
    let mut failures = Vec::new();

    for (file, output) in runs {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let expected = expected_stderr(&file);

        if output.status.code() != Some(expected_code) {
            failures.push(format!(
                "{}: expected exit code {expected_code}, got {:?}",
                file.display(),
                output.status.code()
            ));
        }
        if stdout != expected_stdout {
            failures.push(format!(
                "{}: expected stdout {expected_stdout:?}, got {stdout:?}",
                file.display()
            ));
        }
        if stderr.trim_end() != expected.trim_end() {
            failures.push(format!(
                "{}: stderr mismatch\n--- expected ---\n{expected}\n--- actual ---\n{stderr}",
                file.display()
            ));
        }
    }

    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// Reads the `.stderr` file next to a fixture; every fixture must have one.
fn stderr_file(relative_path: &Path) -> String {
    let path = examples_dir().join(relative_path.with_extension("stderr"));
    fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "every fixture needs a .stderr file; could not read {}: {err}",
            path.display()
        )
    })
}

#[test]
fn canonical_v0_1_examples_are_present() {
    let discovered = mprx_files("");
    for name in CANONICAL_EXAMPLES {
        assert!(
            discovered.contains(&PathBuf::from(name)),
            "examples/{name} is a required v0.1 canonical example"
        );
    }
}

#[test]
fn examples_check_clean() {
    assert_corpus("", &[], 0, |_| String::new());
}

#[test]
fn pass_fixtures_exit_zero_with_expected_warnings() {
    assert_corpus("fixtures/pass", &[], 0, stderr_file);
}

#[test]
fn fail_fixtures_exit_one_with_expected_diagnostics() {
    assert_corpus("fixtures/fail", &[], 1, stderr_file);
}

#[test]
fn check_pass_fixtures_check_clean_against_their_model() {
    assert_corpus("fixtures/check/pass", &CHECK_ARGS, 0, |_| String::new());
}

#[test]
fn check_fail_fixtures_exit_one_with_expected_diagnostics() {
    assert_corpus("fixtures/check/fail", &CHECK_ARGS, 1, stderr_file);
}

#[test]
fn examples_check_clean_against_the_example_model() {
    let runs = mprx_files("")
        .into_iter()
        .map(|file| {
            let name = file.to_str().expect("a UTF-8 path");
            let mut args = vec!["--model", EXAMPLE_MODEL];
            if let Some((_, component)) = EXAMPLE_COMPONENTS.iter().find(|(f, _)| *f == name) {
                args.extend(["--component", component]);
            }
            let output = check(&file, &args);
            (file, output)
        })
        .collect();
    assert_runs(runs, 0, |_| String::new());
}

#[test]
fn broken_manifests_exit_one_with_expected_diagnostics() {
    let runs = files("fixtures/manifest/fail", "json")
        .into_iter()
        .map(|manifest| {
            let model = manifest.to_str().expect("a UTF-8 path").to_string();
            let output = check(Path::new(MANIFEST_TEMPLATE), &["--model", &model]);
            (manifest, output)
        })
        .collect();
    assert_runs(runs, 1, stderr_file);
}

/// The example manifest must model the examples truthfully, so later
/// passes can check the examples against it without editing it. This
/// checks only that the names line up: the component each example is the
/// template of, and the tag of every element in it, are declared.
/// `examples_check_clean_against_the_example_model` checks that the
/// examples use them correctly.
#[test]
fn example_manifest_declares_what_the_examples_name() {
    fn tags(element: &mesh_semantic::Element, out: &mut Vec<String>) {
        out.push(element.name.clone());
        for child in &element.children {
            if let mesh_semantic::Child::Element(child) = child {
                tags(child, out);
            }
        }
    }

    let model = fs::read_to_string(examples_dir().join(EXAMPLE_MODEL))
        .expect("should read the example manifest");
    let manifest = mesh_manifest::load(&model).expect("the example manifest should load");

    let mut missing = Vec::new();
    for file in mprx_files("") {
        let name = file.to_str().expect("a UTF-8 path");
        let template = EXAMPLE_COMPONENTS
            .iter()
            .find(|(f, _)| *f == name)
            .map_or_else(
                || {
                    name.strip_suffix(".mprx")
                        .expect("an .mprx file")
                        .to_string()
                },
                |(_, component)| component.to_string(),
            );
        if manifest.template(&template).is_err() {
            missing.push(format!("{name}: template component {template:?}"));
        }

        let source = fs::read_to_string(examples_dir().join(&file)).expect("should read");
        let ir = mesh_compiler::compile(&source)
            .ir
            .unwrap_or_else(|| panic!("{name} should compile"));
        let mut names = Vec::new();
        tags(&ir, &mut names);
        for tag in names {
            if !manifest.components().contains_key(&tag) {
                missing.push(format!("{name}: element <{tag}>"));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "examples/{EXAMPLE_MODEL} doesn't declare: {missing:#?}"
    );
}

/// Every code that checking against a model can produce (every code
/// after the `manifest-*` ones) has a fail fixture named after it, whose
/// expected output reports that code.
#[test]
fn every_model_code_has_its_own_fail_fixture() {
    let codes = mesh_syntax::DiagnosticCode::ALL;
    let first = codes
        .iter()
        .position(|code| *code == mesh_syntax::DiagnosticCode::MANIFEST_MISSING_COMPONENT)
        .expect("the last manifest code is listed")
        + 1;
    assert!(first < codes.len(), "no model codes yet");

    let mut missing = Vec::new();
    for code in &codes[first..] {
        let fixture = PathBuf::from(format!("fixtures/check/fail/{code}.mprx"));
        if !examples_dir().join(&fixture).is_file() {
            missing.push(format!("{}: no such fixture", fixture.display()));
        } else if !stderr_file(&fixture).contains(&format!("error[{code}]")) {
            missing.push(format!("{}: doesn't report {code}", fixture.display()));
        }
    }
    assert!(missing.is_empty(), "{missing:#?}");
}
