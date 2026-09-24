//! Where a byte offset falls in a source, as a 1-based line and column.
//!
//! Both presentations of a diagnostic use this, so the terminal renderer
//! and the JSON output always agree on where a span starts.

use std::ops::Range;

/// The UTF-8 byte-order mark, as `fs::read_to_string` leaves it at the
/// start of a file saved "with BOM".
const BOM: char = '\u{feff}';

/// A byte offset located on its line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Located {
    /// The 1-based line number.
    pub(crate) line: usize,
    /// The 1-based column: `char`s from the start of the line's visible
    /// text to `offset`.
    pub(crate) column: usize,
    /// The line's visible text, as a byte range: no leading byte-order
    /// mark, no trailing `\r`, no `\n`.
    pub(crate) text: Range<usize>,
    /// The offset, floored to a `char` boundary and clamped into `text`.
    pub(crate) offset: usize,
}

/// Locates `byte` in `source`. Never panics: an offset past the end is
/// clamped to it, and one inside a multi-byte character is floored to
/// that character's start.
///
/// Lines and columns are 1-based. Columns count Unicode scalar values
/// (`char`s), not bytes or display width. An offset inside a line ending
/// (`\r` or `\n`) is just past the line's last visible character, and a
/// leading byte-order mark is not a column: an offset on it is line 1,
/// column 1.
pub(crate) fn locate(source: &str, byte: usize) -> Located {
    // Every slice below uses this normalized offset, never the raw one,
    // so no offset can make this function slice mid-character or out of
    // range.
    let start = source.floor_char_boundary(byte);
    let line_start = source[..start].rfind('\n').map_or(0, |i| i + 1);
    let line_end = source[start..]
        .find('\n')
        .map_or(source.len(), |i| start + i);
    // A leading byte-order mark is an encoding marker, not text: it is
    // never shown or counted as a column. This only moves where line 1's
    // text starts, after the normalization above.
    let text_start = if line_start == 0 && source.starts_with(BOM) {
        BOM.len_utf8()
    } else {
        line_start
    };
    let text_end = text_start + source[text_start..line_end].trim_end_matches('\r').len();
    let offset = start.clamp(text_start, text_end);
    Located {
        line: source[..line_start].matches('\n').count() + 1,
        column: source[text_start..offset].chars().count() + 1,
        text: text_start..text_end,
        offset,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_column(source: &str, byte: usize) -> (usize, usize) {
        let located = locate(source, byte);
        (located.line, located.column)
    }

    #[test]
    fn counts_lines_and_columns_from_one() {
        let source = "<a>\n  <b />\n</a>";
        assert_eq!(line_column(source, 0), (1, 1));
        assert_eq!(line_column(source, 6), (2, 3));
        assert_eq!(line_column(source, 12), (3, 1));
        assert_eq!(line_column(source, source.len()), (3, 5));
    }

    #[test]
    fn counts_columns_in_chars() {
        let source = "<p>Größe</p>";
        let close = source.find("</p>").unwrap();
        assert_eq!(line_column(source, close), (1, 9));
    }

    #[test]
    fn an_offset_in_a_line_ending_is_just_past_the_visible_text() {
        let source = "<a>\r\n<b>";
        assert_eq!(line_column(source, 3), (1, 4));
        assert_eq!(line_column(source, 4), (1, 4));
        assert_eq!(line_column(source, 5), (2, 1));
    }

    #[test]
    fn a_byte_order_mark_is_not_a_column() {
        let source = "\u{feff}<a>";
        assert_eq!(line_column(source, 0), (1, 1));
        assert_eq!(line_column(source, 3), (1, 1));
        assert_eq!(line_column(source, 4), (1, 2));
        assert_eq!(locate(source, 0).text, 3..6);
    }

    #[test]
    fn clamps_and_floors_offsets() {
        let source = "é\n";
        assert_eq!(line_column(source, 1), (1, 1));
        assert_eq!(line_column(source, 99), (2, 1));
        assert_eq!(line_column("", 5), (1, 1));
    }
}
