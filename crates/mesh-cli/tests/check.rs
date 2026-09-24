use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;

#[test]
fn check_reports_no_errors_for_the_canonical_pass_1_example() {
    Command::cargo_bin("mesh")
        .unwrap()
        .arg("check")
        .arg("../../examples/page.mprx")
        .assert()
        .success()
        .stdout(predicate::str::contains("no errors"));
}

#[test]
fn check_reports_an_error_for_invalid_source() {
    let mut file = tempfile::Builder::new()
        .suffix(".mprx")
        .tempfile()
        .expect("should create a temp file");
    write!(file, "<page").expect("should write to the temp file");

    Command::cargo_bin("mesh")
        .unwrap()
        .arg("check")
        .arg(file.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("error["));
}

fn temp_mprx(contents: &str) -> tempfile::NamedTempFile {
    let mut file = tempfile::Builder::new()
        .suffix(".mprx")
        .tempfile()
        .expect("should create a temp file");
    write!(file, "{contents}").expect("should write to the temp file");
    file
}

#[test]
fn check_exits_successfully_when_only_warnings_are_reported() {
    let file = temp_mprx(r#"<div class="a" class="b" />"#);

    Command::cargo_bin("mesh")
        .unwrap()
        .arg("check")
        .arg(file.path())
        .assert()
        .success()
        .stdout("no errors\n")
        .stderr(predicate::str::contains(
            "warning[duplicate-attribute]: duplicate attribute \"class\"",
        ))
        .stderr(predicate::str::contains("error[").not());
}

#[test]
fn check_renders_errors_with_a_source_snippet_and_fails() {
    let file = temp_mprx("<div></span>");

    Command::cargo_bin("mesh")
        .unwrap()
        .arg("check")
        .arg(file.path())
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicate::str::contains(
            "error[mismatched-closing-tag]: mismatched closing tag: opened with \"div\", closed with \"span\"\n",
        ))
        .stderr(predicate::str::contains(format!(
            " --> {}:1:1\n",
            file.path().display()
        )))
        .stderr(predicate::str::contains(
            "1 | <div></span>\n  | ^^^^^^^^^^^^\n\n",
        ));
}

#[test]
fn check_skips_a_leading_byte_order_mark_when_reporting_columns() {
    let file = temp_mprx("\u{feff}<div></span>");

    Command::cargo_bin("mesh")
        .unwrap()
        .arg("check")
        .arg(file.path())
        .assert()
        .code(1)
        .stderr(predicate::str::contains(format!(
            " --> {}:1:1\n",
            file.path().display()
        )))
        .stderr(predicate::str::contains(
            "1 | <div></span>\n  | ^^^^^^^^^^^^\n",
        ));
}

#[test]
fn check_rejects_a_deeply_nested_syntax_error_without_crashing() {
    // D17: v0.1 rejected this with exit 1. Locating the error must not
    // overflow the main thread's stack, which a crash (SIGABRT) would show
    // as a signal instead of an exit code. Each `<p>` is on its own line
    // so the rendered snippet stays one short line.
    let depth = 20_000;
    let file = temp_mprx(&format!(
        "{}{{a +}}\n{}",
        "<p>\n".repeat(depth),
        "</p>\n".repeat(depth)
    ));

    Command::cargo_bin("mesh")
        .unwrap()
        .arg("check")
        .arg(file.path())
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "error[syntax-error]: expected an expression\n",
        ))
        .stderr(predicate::str::contains(format!(
            " --> {}:{}:5\n",
            file.path().display(),
            depth + 1
        )));
}

fn temp_manifest(contents: &str) -> tempfile::NamedTempFile {
    let mut file = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .expect("should create a temp file");
    write!(file, "{contents}").expect("should write to the temp file");
    file
}

/// A valid manifest declaring one component, `page`.
const PAGE_MANIFEST: &str = r#"{
  "version": 1,
  "types": {},
  "components": {
    "page": { "props": {}, "events": {}, "commands": {}, "scope": {} }
  }
}"#;

#[test]
fn check_with_a_broken_manifest_reports_only_the_manifest_errors() {
    let manifest = temp_manifest(r#"{ "version": 1, "types": {}, "components": [] }"#);
    // The file has a syntax error, but a broken manifest stops the check
    // before the file is read.
    let file = temp_mprx("<page");

    Command::cargo_bin("mesh")
        .unwrap()
        .arg("check")
        .arg("--model")
        .arg(manifest.path())
        .arg(file.path())
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicate::str::starts_with(
            "error[manifest-invalid-value]: \"components\" must be an object, found an array\n",
        ))
        .stderr(predicate::str::contains(".json:1:"))
        .stderr(predicate::str::contains("syntax-error").not());
}

#[test]
fn check_reports_an_unreadable_manifest() {
    let file = temp_mprx("<page />");

    Command::cargo_bin("mesh")
        .unwrap()
        .arg("check")
        .arg("--model")
        .arg("does-not-exist.json")
        .arg(file.path())
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicate::str::starts_with(
            "error: could not read does-not-exist.json: ",
        ));
}

#[test]
fn check_takes_the_component_from_the_file_name_unless_given() {
    let manifest = temp_manifest(PAGE_MANIFEST);
    let file = temp_mprx("<page />");
    let stem = file
        .path()
        .file_stem()
        .and_then(|stem| stem.to_str())
        .expect("a UTF-8 file stem")
        .to_string();

    // The temp file's stem isn't a declared component.
    Command::cargo_bin("mesh")
        .unwrap()
        .arg("check")
        .arg("--model")
        .arg(manifest.path())
        .arg(file.path())
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicate::str::starts_with(format!(
            "error[manifest-missing-component]: the manifest declares no component {stem:?}, which this file is the template of\n"
        )));

    Command::cargo_bin("mesh")
        .unwrap()
        .arg("check")
        .arg("--model")
        .arg(manifest.path())
        .arg("--component")
        .arg("page")
        .arg(file.path())
        .assert()
        .success()
        .stdout("no errors\n")
        .stderr("");
}

#[test]
fn check_with_a_model_adds_analysis_diagnostics_after_the_files_own() {
    let manifest = temp_manifest(PAGE_MANIFEST);
    let file = temp_mprx(r#"<page a="1" a="2"></pages>"#);

    let without = Command::cargo_bin("mesh")
        .unwrap()
        .arg("check")
        .arg(file.path())
        .output()
        .expect("should run mesh check");
    let with = Command::cargo_bin("mesh")
        .unwrap()
        .arg("check")
        .arg("--model")
        .arg(manifest.path())
        .arg("--component")
        .arg("page")
        .arg(file.path())
        .output()
        .expect("should run mesh check");

    assert_eq!(with.status.code(), Some(1));
    assert_eq!(with.stdout, without.stdout);
    let without = String::from_utf8(without.stderr).expect("UTF-8 stderr");
    let with = String::from_utf8(with.stderr).expect("UTF-8 stderr");
    let added = with
        .strip_prefix(&without)
        .unwrap_or_else(|| panic!("the file's own diagnostics come first:\n{with}"));
    assert!(
        added.starts_with("error[unknown-prop]: component \"page\" has no prop \"a\"\n"),
        "{added}"
    );
    assert_eq!(added.matches("error[").count(), 1, "{added}");
}

#[test]
fn component_needs_a_model() {
    Command::cargo_bin("mesh")
        .unwrap()
        .arg("check")
        .arg("--component")
        .arg("page")
        .arg("../../examples/page.mprx")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--model <FILE>"));
}

#[test]
fn json_prints_one_document_on_stdout_and_exits_as_human_output_does() {
    Command::cargo_bin("mesh")
        .unwrap()
        .current_dir("../../examples")
        .args([
            "check",
            "--format",
            "json",
            "--model",
            "fixtures/check/components.json",
            "--component",
            "template",
            "fixtures/check/fail/unknown-reference.mprx",
        ])
        .assert()
        .code(1)
        .stderr("")
        .stdout(concat!(
            r#"{"version":1,"diagnostics":[{"severity":"error","code":"unknown-reference","#,
            r#""message":"unknown reference \"usr\": it isn't in the template's scope","#,
            r#""path":"fixtures/check/fail/unknown-reference.mprx","#,
            r#""span":{"start":{"byte":13,"line":1,"column":14},"end":{"byte":16,"line":1,"column":17}},"#,
            r#""suggestions":[{"replacement":"user","#,
            r#""span":{"start":{"byte":13,"line":1,"column":14},"end":{"byte":16,"line":1,"column":17}}}]}]}"#,
            "\n"
        ));
}

#[test]
fn json_prints_an_empty_list_instead_of_no_errors() {
    Command::cargo_bin("mesh")
        .unwrap()
        .args(["check", "--format", "json", "../../examples/page.mprx"])
        .assert()
        .success()
        .stderr("")
        .stdout("{\"version\":1,\"diagnostics\":[]}\n");
}

#[test]
fn json_keeps_warnings_and_exit_status_zero() {
    let file = temp_mprx(r#"<div class="a" class="b" />"#);

    Command::cargo_bin("mesh")
        .unwrap()
        .arg("check")
        .args(["--format", "json"])
        .arg(file.path())
        .assert()
        .success()
        .stderr("")
        .stdout(predicate::str::contains(
            r#"{"severity":"warning","code":"duplicate-attribute","#,
        ))
        .stdout(predicate::str::contains("no errors").not());
}

#[test]
fn json_reports_manifest_errors_against_the_manifest() {
    let manifest = temp_manifest(r#"{ "version": 1, "types": {}, "components": [] }"#);
    let file = temp_mprx("<page");

    Command::cargo_bin("mesh")
        .unwrap()
        .arg("check")
        .args(["--format", "json", "--model"])
        .arg(manifest.path())
        .arg(file.path())
        .assert()
        .code(1)
        .stderr("")
        .stdout(predicate::str::contains(
            r#""code":"manifest-invalid-value""#,
        ))
        .stdout(predicate::str::contains(format!(
            r#""path":{}"#,
            // As JSON escapes it (a Windows path's `\` is doubled).
            serde_json::to_string(&manifest.path().display().to_string())
                .expect("a string serializes")
        )))
        .stdout(predicate::str::contains("syntax-error").not());
}

#[test]
fn json_leaves_an_unreadable_file_on_stderr() {
    Command::cargo_bin("mesh")
        .unwrap()
        .args(["check", "--format", "json", "does-not-exist.mprx"])
        .assert()
        .code(1)
        .stdout("")
        .stderr(predicate::str::starts_with(
            "error: could not read does-not-exist.mprx: ",
        ));
}

#[test]
fn rejects_an_unknown_format() {
    Command::cargo_bin("mesh")
        .unwrap()
        .args(["check", "--format", "xml", "../../examples/page.mprx"])
        .assert()
        .code(2)
        .stdout("")
        .stderr(predicate::str::contains("[possible values: human, json]"));
}

/// The CLI manual's JSON example is what `mesh` prints for the command
/// it shows, and its indented copy is the same document.
#[test]
fn the_cli_manuals_json_example_is_what_mesh_prints() {
    let manual = std::fs::read_to_string("../../docs/manual/mesh-cli.md")
        .expect("should read the CLI manual");
    let mut lines = manual.lines();
    let command = lines
        .find_map(|line| line.strip_prefix("$ mesh check --format json --model "))
        .expect("the manual shows a JSON example");
    let printed = lines.next().expect("the example shows its output");
    let args: Vec<&str> = command.split_whitespace().collect();

    Command::cargo_bin("mesh")
        .unwrap()
        .current_dir("../..")
        .args(["check", "--format", "json", "--model"])
        .args(&args)
        .assert()
        .stdout(format!("{printed}\n"));

    let indented = manual
        .split("```json\n")
        .nth(1)
        .and_then(|block| block.split("```").next())
        .expect("the manual shows the document indented");
    let parse = |text: &str| -> serde_json::Value {
        serde_json::from_str(text).expect("the example is JSON")
    };
    assert_eq!(parse(indented), parse(printed));
}

/// Runs `mesh check` on a file with thousands of errors, far more output
/// than a pipe buffer holds, with the stream `closed` already closed by
/// its reader, and returns the exit status and stderr (or `""` if stderr
/// was the stream closed).
fn check_with_a_closed_pipe(format: &str, closed: &str) -> (Option<i32>, String) {
    use std::process::{Command, Stdio};
    let file = temp_mprx(&format!("<p>{}</p>", "{a +}".repeat(5_000)));
    let mut command = Command::new(assert_cmd::cargo::cargo_bin("mesh"));
    command
        .args(["check", "--format", format])
        .arg(file.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("mesh starts");
    // Close the reading end before `mesh` can write, so every write to it
    // fails with a broken pipe.
    let mut stderr = match closed {
        "stdout" => {
            drop(child.stdout.take());
            child.stderr.take()
        }
        _ => {
            drop(child.stderr.take());
            None
        }
    };
    let mut captured = String::new();
    if let Some(stderr) = stderr.as_mut() {
        std::io::Read::read_to_string(stderr, &mut captured).expect("stderr is UTF-8");
    }
    // Drain stdout if it's still open, so `mesh` can't block on it.
    if let Some(mut stdout) = child.stdout.take() {
        std::io::copy(&mut stdout, &mut std::io::sink()).expect("stdout can be drained");
    }
    let status = child.wait().expect("mesh exits");
    (status.code(), captured)
}

#[test]
fn json_output_to_a_closed_pipe_exits_quietly() {
    let (code, stderr) = check_with_a_closed_pipe("json", "stdout");
    assert_eq!(code, Some(1), "stderr: {stderr}");
    assert!(!stderr.contains("panicked"), "stderr: {stderr}");
}

#[test]
fn human_output_to_a_closed_pipe_exits_quietly() {
    // With stderr closed there's nothing to read a panic from, so the
    // exit status is the evidence: a panic exits 101.
    let (code, _) = check_with_a_closed_pipe("human", "stderr");
    assert_eq!(code, Some(1));
}
