//! rustc-style rendering of [`Diagnostic`]s for terminal output.
//!
//! This is a presentation adapter: `Diagnostic` stays pure data in
//! `mesh-syntax`, and this module only formats it. Living in
//! `mesh-compiler` is a v0.1 placement choice, not a statement that
//! terminal presentation belongs to the compiler's semantic model.

use mesh_syntax::Diagnostic;

/// The UTF-8 byte-order mark, as `fs::read_to_string` leaves it at the
/// start of a file saved "with BOM".
const BOM: char = '\u{feff}';

/// Renders `diagnostic` rustc-style: a `severity: message` header, a
/// `--> path:line:column` location, and the source line the diagnostic
/// starts on, underlined with carets:
///
/// ```text
/// error: mismatched closing tag: opened with "title", closed with "heading"
///  --> card.mprx:2:3
///   |
/// 2 |   <title>Users</heading>
///   |   ^^^^^^^^^^^^^^^^^^^^^^
/// ```
///
/// Returns exactly one block with no trailing newline — separating
/// blocks is the caller's job.
///
/// Lines and columns are 1-based. Columns count Unicode scalar values
/// (`char`s), not bytes, and deliberately ignore terminal display width:
/// wide characters get no special alignment in v0.1. Tabs before the
/// span are echoed in the underline so carets stay aligned. A span
/// covering several lines is underlined only to the end of its first
/// line, and a trailing `\r` (CRLF sources) is never shown, counted, or
/// underlined; neither is a leading byte-order mark. An out-of-range
/// span, or one that splits a multi-byte character, is clamped rather
/// than panicking — a bad span should degrade the snippet, not crash
/// the CLI.
pub fn render_diagnostic(source: &str, path: &str, diagnostic: &Diagnostic) -> String {
    // Normalize the span before anything else. Every slice below uses
    // `start` and `end`, never the raw span offsets, so no span can make
    // this function slice mid-character or out of range.
    let start = source.floor_char_boundary(diagnostic.span.start_byte);
    let end = source
        .floor_char_boundary(diagnostic.span.end_byte)
        .max(start);

    let line_start = source[..start].rfind('\n').map_or(0, |i| i + 1);
    let line_end = source[start..]
        .find('\n')
        .map_or(source.len(), |i| start + i);
    // A leading byte-order mark is an encoding marker, not text: it is
    // never shown, counted as a column, or underlined. This only moves
    // where line 1's text starts; it works on the normalized `line_start`
    // and never replaces the normalization above.
    let text_start = if line_start == 0 && source.starts_with(BOM) {
        BOM.len_utf8()
    } else {
        line_start
    };
    let line_text = source[text_start..line_end].trim_end_matches('\r');
    let text_end = text_start + line_text.len();
    let line_number = source[..line_start].matches('\n').count() + 1;
    // A span starting inside the line ending (`\r` or `\n`) points just
    // past the last visible character, never into the line ending itself;
    // one starting on the byte-order mark points at the first character.
    let visible_start = start.clamp(text_start, text_end);
    let column = source[text_start..visible_start].chars().count() + 1;

    let indent: String = source[text_start..visible_start]
        .chars()
        .map(|c| if c == '\t' { '\t' } else { ' ' })
        .collect();
    let underline_end = end.min(text_end).max(visible_start);
    let carets = "^".repeat(source[visible_start..underline_end].chars().count().max(1));

    let gutter = " ".repeat(line_number.to_string().len());
    // Omit the separating space for an empty line so no output line ends
    // in trailing whitespace (fixture `.stderr` files must survive editors
    // that strip it, and `git diff --check`).
    let code = if line_text.is_empty() {
        String::new()
    } else {
        format!(" {line_text}")
    };

    format!(
        "{severity}: {message}\n\
         {gutter}--> {path}:{line_number}:{column}\n\
         {gutter} |\n\
         {line_number} |{code}\n\
         {gutter} | {indent}{carets}",
        severity = diagnostic.severity,
        message = diagnostic.message,
    )
}
