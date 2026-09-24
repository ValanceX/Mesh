//! rustc-style rendering of [`Diagnostic`]s for terminal output.
//!
//! This is a presentation adapter: `Diagnostic` stays pure data in
//! `mesh-syntax`, and this module only formats it. Living in
//! `mesh-compiler` is a v0.1 placement choice, not a statement that
//! terminal presentation belongs to the compiler's semantic model.

use crate::position::locate;
use crate::source_map::SourceMap;
use mesh_syntax::Diagnostic;

/// Renders `diagnostic` rustc-style: a `severity[code]: message` header, a
/// `--> path:line:column` location, the source line the diagnostic
/// starts on, underlined with carets, and a `= help:` line for each of
/// its suggestions:
///
/// ```text
/// error[mismatched-closing-tag]: mismatched closing tag: opened with "title", closed with "heading"
///  --> card.mprx:2:3
///   |
/// 2 |   <title>Users</heading>
///   |   ^^^^^^^^^^^^^^^^^^^^^^
/// ```
///
/// ```text
/// error[unknown-reference]: unknown reference "usr": it isn't in the template's scope
///  --> card.mprx:2:15
///   |
/// 2 |   <card user={usr} />
///   |               ^^^
///   = help: did you mean "user"?
/// ```
///
/// Returns exactly one block with no trailing newline — separating
/// blocks is the caller's job.
///
/// Lines and columns are 1-based. Columns count Unicode scalar values
/// (`char`s), not bytes, and deliberately ignore terminal display width:
/// wide characters get no special alignment. Tabs before the
/// span are echoed in the underline so carets stay aligned. A span
/// covering several lines is underlined only to the end of its first
/// line, and a trailing `\r` (CRLF sources) is never shown, counted, or
/// underlined; neither is a leading byte-order mark. An out-of-range
/// span, or one that splits a multi-byte character, is clamped rather
/// than panicking — a bad span should degrade the snippet, not crash
/// the CLI.
pub fn render_diagnostic(source: &str, path: &str, diagnostic: &Diagnostic) -> String {
    // Normalize the span before anything else. Every slice below uses
    // `start` and `end`, or offsets `locate` normalized, never the raw
    // span offsets, so no span can make this function slice mid-character
    // or out of range.
    let start = source.floor_char_boundary(diagnostic.span.start_byte);
    let end = source
        .floor_char_boundary(diagnostic.span.end_byte)
        .max(start);
    // `at.offset` is where the carets start: a span starting inside the
    // line ending (`\r` or `\n`) points just past the last visible
    // character, and one starting on the byte-order mark points at the
    // first character.
    let at = locate(&SourceMap::new(source), source, start);
    let line_number = at.line;
    let column = at.column;
    let line_text = &source[at.text.clone()];

    let indent: String = source[at.text.start..at.offset]
        .chars()
        .map(|c| if c == '\t' { '\t' } else { ' ' })
        .collect();
    let underline_end = end.min(at.text.end).max(at.offset);
    let carets = "^".repeat(source[at.offset..underline_end].chars().count().max(1));

    let gutter = " ".repeat(line_number.to_string().len());
    // Omit the separating space for an empty line so no output line ends
    // in trailing whitespace (fixture `.stderr` files must survive editors
    // that strip it, and `git diff --check`).
    let snippet = if line_text.is_empty() {
        String::new()
    } else {
        format!(" {line_text}")
    };

    let mut block = format!(
        "{severity}[{code}]: {message}\n\
         {gutter}--> {path}:{line_number}:{column}\n\
         {gutter} |\n\
         {line_number} |{snippet}\n\
         {gutter} | {indent}{carets}",
        severity = diagnostic.severity,
        code = diagnostic.code,
        message = diagnostic.message,
    );
    for suggestion in &diagnostic.suggestions {
        block.push_str(&format!(
            "\n{gutter} = help: did you mean {:?}?",
            suggestion.replacement
        ));
    }
    block
}
