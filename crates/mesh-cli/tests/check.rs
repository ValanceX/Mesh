use assert_cmd::Command;
use predicates::prelude::*;

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
    let dir = std::env::temp_dir();
    let path = dir.join("mesh_cli_invalid_test.mprx");
    std::fs::write(&path, "<page").unwrap();

    Command::cargo_bin("mesh")
        .unwrap()
        .arg("check")
        .arg(&path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("error:"));
}
