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
