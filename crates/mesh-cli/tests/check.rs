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
        .stderr(predicate::str::contains("error:"));
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
            "warning: duplicate attribute \"class\"",
        ))
        .stderr(predicate::str::contains("error:").not());
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
            "error: mismatched closing tag: opened with \"div\", closed with \"span\"\n",
        ))
        .stderr(predicate::str::contains(format!(
            " --> {}:1:1\n",
            file.path().display()
        )))
        .stderr(predicate::str::contains(
            "1 | <div></span>\n  | ^^^^^^^^^^^^\n\n",
        ));
}
