//! Machine-readable rendering of [`Diagnostic`]s, for tools that feed
//! them back to whatever wrote the source (generate-check-repair loops).
//!
//! Like `render`, this is a presentation adapter over plain data. The
//! document's shape is published as a JSON Schema in
//! `schemas/diagnostics-v1.schema.json`.

use crate::position::locate;
use crate::source_map::{ColumnUnit, SourceMap};
use mesh_syntax::{Diagnostic, Span};
use serde::Serialize;

/// The version of the document's shape. It changes only if a field is
/// removed or changes meaning; adding a field doesn't change it.
const VERSION: u32 = 1;

#[derive(Serialize)]
struct Document<'a> {
    version: u32,
    diagnostics: Vec<JsonDiagnostic<'a>>,
}

#[derive(Serialize)]
struct JsonDiagnostic<'a> {
    severity: String,
    code: &'static str,
    message: &'a str,
    path: &'a str,
    span: JsonSpan,
    suggestions: Vec<JsonSuggestion<'a>>,
}

#[derive(Serialize)]
struct JsonSuggestion<'a> {
    replacement: &'a str,
    span: JsonSpan,
}

#[derive(Serialize)]
struct JsonSpan {
    start: Position,
    end: Position,
}

#[derive(Serialize)]
struct Position {
    byte: usize,
    line: usize,
    column: usize,
    utf16: usize,
    #[serde(rename = "utf16Column")]
    utf16_column: usize,
}

impl JsonSpan {
    fn new(map: &SourceMap, source: &str, span: Span) -> Self {
        JsonSpan {
            start: Position::new(map, source, span.start_byte),
            end: Position::new(map, source, span.end_byte),
        }
    }
}

impl Position {
    fn new(map: &SourceMap, source: &str, byte: usize) -> Self {
        let located = locate(map, source, byte);
        Position {
            byte,
            line: located.line,
            column: located.column,
            utf16: map.utf16_offset(source, byte),
            utf16_column: map.line_column(source, byte, ColumnUnit::Utf16).column + 1,
        }
    }
}

/// Renders `diagnostics`, all reported against `source` at `path`, as one
/// JSON document on a single line, with no trailing newline:
///
/// ```json
/// {"version":1,"diagnostics":[{"severity":"error","code":"unknown-reference",
///  "message":"unknown reference \"usr\": it isn't in the template's scope",
///  "path":"card.mprx","span":{"start":{"byte":12,"line":1,"column":13,"utf16":12,"utf16Column":13},
///  "end":{"byte":15,"line":1,"column":16,"utf16":15,"utf16Column":16}},"suggestions":[{"replacement":"user",
///  "span":{"start":{"byte":12,"line":1,"column":13,"utf16":12,"utf16Column":13},
///  "end":{"byte":15,"line":1,"column":16,"utf16":15,"utf16Column":16}}}]}]}
/// ```
///
/// (wrapped here for reading). Diagnostics keep the order given. Each
/// span gives its start and end as a byte offset into `source` and as a
/// 1-based line and column, computed exactly as [`render_diagnostic`]
/// computes the `-->` location: columns count `char`s, and a leading
/// byte-order mark and a line's `\r` are not columns. For JavaScript,
/// each position also gives `utf16`, the offset in UTF-16 code units
/// that corresponds to `byte` (a byte-order mark counts), and
/// `utf16Column`, the column in UTF-16 code units, with `column`'s rules.
/// Both come from [`SourceMap`]. `suggestions` is always present, and
/// usually empty.
///
/// [`render_diagnostic`]: crate::render_diagnostic
pub fn render_json(source: &str, path: &str, diagnostics: &[Diagnostic]) -> String {
    // One map for the whole document, so locating every span costs
    // O(log lines + line length), not a scan of the whole source.
    let map = SourceMap::new(source);
    let document = Document {
        version: VERSION,
        diagnostics: diagnostics
            .iter()
            .map(|diagnostic| JsonDiagnostic {
                severity: diagnostic.severity.to_string(),
                code: diagnostic.code.as_str(),
                message: &diagnostic.message,
                path,
                span: JsonSpan::new(&map, source, diagnostic.span),
                suggestions: diagnostic
                    .suggestions
                    .iter()
                    .map(|suggestion| JsonSuggestion {
                        replacement: &suggestion.replacement,
                        span: JsonSpan::new(&map, source, suggestion.span),
                    })
                    .collect(),
            })
            .collect(),
    };
    serde_json::to_string(&document).expect("a document of strings and numbers always serializes")
}
