//! Pins the highlighting query's captures.
//!
//! MPRX has no comments, so Tree-sitter's own highlight tests, whose
//! assertions are written in comments, can't assert anything. (That's why
//! the samples aren't in `test/highlight/`, where `tree-sitter test` would
//! run each as a highlight test with no assertions.) Instead, each
//! `test/captures/*.mprx` file has a `.captures` file beside it
//! listing every capture the query makes on it, in document order, one
//! per line: `<start row>:<column>-<end row>:<column> <capture> <text>`.
//!
//! After a deliberate change to the query or the samples, run the tests
//! with `MESH_BLESS=1` to rewrite the `.captures` files, and read the diff.
//! A blessed run still fails, so it never passes silently.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};

fn highlight_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("test/captures")
}

fn query() -> Query {
    Query::new(&super::LANGUAGE.into(), super::HIGHLIGHTS_QUERY)
        .unwrap_or_else(|err| panic!("queries/highlights.scm doesn't compile: {err}"))
}

/// Every capture `query` makes on `source`, one line each.
fn captures(query: &Query, source: &str) -> String {
    let mut parser = Parser::new();
    parser
        .set_language(&super::LANGUAGE.into())
        .expect("the grammar loads");
    let tree = parser
        .parse(source, None)
        .expect("the parser returns a tree");
    let mut cursor = QueryCursor::new();
    let mut captures = cursor.captures(query, tree.root_node(), source.as_bytes());
    let mut out = String::new();
    while let Some((found, index)) = captures.next() {
        let capture = found.captures()[*index];
        let node = capture.node;
        let (start, end) = (node.start_position(), node.end_position());
        let text = &source[node.byte_range()];
        let name = query.capture_names()[capture.index as usize];
        writeln!(
            out,
            "{}:{}-{}:{} {name} {text:?}",
            start.row, start.column, end.row, end.column
        )
        .expect("writing to a String");
    }
    out
}

fn samples() -> Vec<PathBuf> {
    let mut samples: Vec<PathBuf> = fs::read_dir(highlight_dir())
        .expect("test/captures exists")
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "mprx"))
        .collect();
    samples.sort();
    samples
}

#[test]
fn the_highlights_query_is_built_in() {
    // `build.rs` sets this when `queries/highlights.scm` exists.
    const { assert!(cfg!(with_highlights_query)) };
}

#[test]
fn every_sample_has_exactly_its_expected_captures() {
    let query = query();
    let bless = std::env::var_os("MESH_BLESS").is_some();
    let samples = samples();
    assert!(samples.len() >= 4, "{samples:?}");
    let mut failures = Vec::new();
    for sample in samples {
        let source = fs::read_to_string(&sample).expect("the sample reads");
        let actual = captures(&query, &source);
        let expected_path = sample.with_extension("captures");
        if bless {
            fs::write(&expected_path, &actual).expect("the captures write");
            failures.push(format!("blessed {}", expected_path.display()));
            continue;
        }
        let expected = fs::read_to_string(&expected_path).unwrap_or_default();
        if actual != expected {
            failures.push(format!(
                "{} differs from {}:\n{actual}",
                sample.display(),
                expected_path.display()
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Kinds that are deliberately left uncaptured: structure with nothing of
/// its own to colour, plain text, and the parts of a string its `string`
/// capture already covers.
const UNCAPTURED: &[&str] = &[
    "source_file",
    "element",
    "child",
    "text",
    "expression",
    "literal",
    "identifier",
    "string_content",
    "\"",
    "true",
    "false",
];

/// Every kind a tree can contain is either captured by the query or
/// listed in [`UNCAPTURED`], so a grammar change that adds one fails here
/// until someone decides how it's coloured.
#[test]
fn every_node_kind_is_highlighted_or_listed() {
    let language: tree_sitter::Language = super::LANGUAGE.into();
    let source = super::HIGHLIGHTS_QUERY;
    let mut missing = Vec::new();
    for id in 0..language.node_kind_count() {
        let id = u16::try_from(id).expect("kind ids fit u16");
        if !language.node_kind_is_visible(id) {
            continue;
        }
        let Some(kind) = language.node_kind_for_id(id) else {
            continue;
        };
        if kind == "ERROR" || UNCAPTURED.contains(&kind) {
            continue;
        }
        let spelled = if language.node_kind_is_named(id) {
            format!("({kind}")
        } else {
            format!("\"{}\"", kind.replace('\\', "\\\\").replace('"', "\\\""))
        };
        if !source.contains(&spelled) && !missing.contains(&kind) {
            missing.push(kind);
        }
    }
    assert!(
        missing.is_empty(),
        "not in queries/highlights.scm or UNCAPTURED: {missing:?}"
    );
}
