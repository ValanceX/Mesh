use assert_cmd::Command;

#[test]
fn prints_the_version_for_both_version_flags() {
    for flag in ["--version", "-V"] {
        Command::cargo_bin("mesh")
            .unwrap()
            .arg(flag)
            .assert()
            .success()
            .stdout(format!("mesh {}\n", env!("CARGO_PKG_VERSION")))
            .stderr("");
    }
}
