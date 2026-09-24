//! `CompileResult::analysis`: the canonical compile keeps what analysis
//! found, so tools read the same resolutions and types the diagnostics
//! came from.

use mesh_analysis::Target;
use mesh_compiler::{compile, compile_with, CompileOptions};
use std::path::PathBuf;

fn example(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

fn manifest() -> mesh_manifest::Manifest {
    mesh_manifest::load(&example("components.json")).expect("the example manifest is valid")
}

#[test]
fn compile_with_keeps_its_analysis() {
    let manifest = manifest();
    let template = manifest.template("users-page").expect("declared");
    let source = example("users-page.mprx").replace("user.name}", "usr.name}");
    let result = compile_with(&source, &CompileOptions::with_template(template));

    let analysis = result
        .analysis
        .as_ref()
        .expect("a file that parses is analyzed");
    assert!(analysis
        .resolutions()
        .iter()
        .any(|resolution| resolution.target == Target::Component("avatar".to_string())));
    // Every analysis diagnostic is one of its facts, in order, after the
    // parse and lowering diagnostics (there are none here).
    let fact_spans: Vec<_> = analysis.facts().iter().map(|fact| fact.span()).collect();
    let diagnostic_spans: Vec<_> = result.diagnostics.iter().map(|d| d.span).collect();
    assert_eq!(fact_spans, diagnostic_spans);
    assert!(!fact_spans.is_empty());
}

#[test]
fn compile_has_no_analysis() {
    let result = compile(&example("users-page.mprx"));
    assert!(result.ir.is_some());
    assert_eq!(result.analysis, None);
}

#[test]
fn a_syntax_error_has_no_analysis() {
    let manifest = manifest();
    let template = manifest.template("users-page").expect("declared");
    let result = compile_with(
        "<page title=\"x\">{user.}</page>",
        &CompileOptions::with_template(template),
    );
    assert!(result.ir.is_none());
    assert_eq!(result.analysis, None);
}
