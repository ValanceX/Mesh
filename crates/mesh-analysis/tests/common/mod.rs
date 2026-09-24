//! Helpers shared by the analysis tests. Each test file uses some of them.
#![allow(dead_code)]

use mesh_analysis::Analysis;
use mesh_syntax::Span;

/// Parses and lowers `source`, then analyzes it as the template of
/// `template` in `manifest`.
pub fn analyze(manifest: &str, template: &str, source: &str) -> Analysis {
    let manifest = mesh_manifest::load(manifest).expect("the test manifest loads");
    let template = manifest
        .template(template)
        .expect("the template is declared");
    let ast = mesh_parser::parse(source).ast.expect("the source parses");
    let ir = mesh_semantic::lower(&ast).ir.expect("the source lowers");
    mesh_analysis::analyze(&ir, template)
}

/// The span of the `n`th (0-based) occurrence of `needle` in `source`.
pub fn nth(source: &str, needle: &str, n: usize) -> Span {
    let start_byte = source
        .match_indices(needle)
        .nth(n)
        .unwrap_or_else(|| panic!("{needle:?} occurs fewer than {} times", n + 1))
        .0;
    Span {
        start_byte,
        end_byte: start_byte + needle.len(),
    }
}

/// The span of the first occurrence of `needle` in `source`.
pub fn at(source: &str, needle: &str) -> Span {
    nth(source, needle, 0)
}

pub fn names(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| name.to_string()).collect()
}
