//! Corpus-driven integration tests: runs the real `mesh` binary over the
//! `.mprx` files in the workspace's `examples/` directory.
//!
//! - `examples/*.mprx` — clean examples; must check clean (exit 0,
//!   `no errors`, empty stderr). `CANONICAL_EXAMPLES` must be among them.
//! - `examples/fixtures/pass/*.mprx` — must exit 0 with `no errors`;
//!   stderr (warnings) must match the sibling `.stderr` file.
//! - `examples/fixtures/fail/*.mprx` — must exit 1 with empty stdout;
//!   stderr must match the sibling `.stderr` file.
//!
//! Discovery is deliberately non-recursive: only files directly inside
//! each of those three directories are found, and files in nested
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

/// Every `.mprx` file directly inside `examples/<relative_dir>` (not
/// recursive), as a path relative to `examples/`, sorted so failures
/// report in a stable order.
fn mprx_files(relative_dir: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(examples_dir().join(relative_dir))
        .unwrap_or_else(|err| panic!("should read examples/{relative_dir}: {err}"))
        .map(|entry| entry.expect("should read a directory entry").path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "mprx"))
        .map(|path| Path::new(relative_dir).join(path.file_name().expect("has a file name")))
        .collect();
    files.sort();
    // Guards against a moved/renamed directory making the test pass vacuously.
    assert!(
        !files.is_empty(),
        "examples/{relative_dir} should contain at least one .mprx file"
    );
    files
}

fn check(relative_path: &Path) -> Output {
    Command::cargo_bin("mesh")
        .unwrap()
        .current_dir(examples_dir())
        .arg("check")
        .arg(relative_path)
        .output()
        .expect("should run mesh check")
}

/// Checks every file in `relative_dir`, collecting all mismatches before
/// failing so one run reports the whole corpus, not just the first miss.
/// `expected_stderr` returning an empty string means "stderr must be empty".
fn assert_corpus(
    relative_dir: &str,
    expected_code: i32,
    expected_stderr: impl Fn(&Path) -> String,
) {
    let expected_stdout = if expected_code == 0 {
        "no errors\n"
    } else {
        ""
    };
    let mut failures = Vec::new();

    for file in mprx_files(relative_dir) {
        let output = check(&file);
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
    assert_corpus("", 0, |_| String::new());
}

#[test]
fn pass_fixtures_exit_zero_with_expected_warnings() {
    assert_corpus("fixtures/pass", 0, stderr_file);
}

#[test]
fn fail_fixtures_exit_one_with_expected_diagnostics() {
    assert_corpus("fixtures/fail", 1, stderr_file);
}
