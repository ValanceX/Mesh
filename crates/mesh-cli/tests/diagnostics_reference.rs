//! Keeps the diagnostics reference (`docs/manual/diagnostics.md`) in step
//! with the codes MESH can emit: every code has a `` ### `code` `` heading,
//! and every code-shaped heading names a real code.

use mesh_syntax::DiagnosticCode;
use std::fs;
use std::path::Path;

fn reference() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/manual/diagnostics.md");
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("should read {}: {err}", path.display()))
}

/// The text of every `` ### `...` `` heading that looks like a code: no
/// spaces inside the backticks. (`could not read <file>: <reason>` is a
/// CLI message with no code, so it's skipped.)
fn code_headings(reference: &str) -> Vec<&str> {
    reference
        .lines()
        .filter_map(|line| line.strip_prefix("### `")?.strip_suffix('`'))
        .filter(|heading| !heading.contains(' '))
        .collect()
}

#[test]
fn every_code_has_a_reference_entry() {
    let reference = reference();
    let headings = code_headings(&reference);
    let missing: Vec<&str> = DiagnosticCode::ALL
        .iter()
        .map(|code| code.as_str())
        .filter(|code| !headings.contains(code))
        .collect();
    assert!(
        missing.is_empty(),
        "docs/manual/diagnostics.md has no ### heading for: {missing:?}"
    );
}

#[test]
fn every_reference_entry_is_a_real_code() {
    let reference = reference();
    let unknown: Vec<&str> = code_headings(&reference)
        .into_iter()
        .filter(|heading| {
            !DiagnosticCode::ALL
                .iter()
                .any(|code| code.as_str() == *heading)
        })
        .collect();
    assert!(
        unknown.is_empty(),
        "docs/manual/diagnostics.md documents codes MESH doesn't have: {unknown:?}"
    );
}

/// Every example block that shows a fixture's output must match that
/// fixture's `.stderr` exactly (the examples show paths from the repo
/// root; the `.stderr` files show them from `examples/`).
#[test]
fn every_example_matches_its_fixture() {
    let reference = reference();
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut checked = 0;
    for block in reference.split("```text\n").skip(1) {
        let block = block
            .split("```")
            .next()
            .expect("split yields at least one piece");
        let Some(location) = block
            .lines()
            .find_map(|line| line.strip_prefix(" --> examples/"))
        else {
            continue;
        };
        let fixture = location
            .split(':')
            .next()
            .expect("split yields at least one piece");
        let stderr_path = examples.join(fixture).with_extension("stderr");
        let expected = fs::read_to_string(&stderr_path)
            .unwrap_or_else(|err| panic!("should read {}: {err}", stderr_path.display()));
        assert_eq!(
            block.replace(" --> examples/", " --> "),
            expected,
            "docs/manual/diagnostics.md example for {fixture} doesn't match its .stderr"
        );
        checked += 1;
    }
    // Guards against a format change making the test pass vacuously.
    assert!(checked >= 10, "only {checked} examples were checked");
}
