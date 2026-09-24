//! Where a byte offset falls in a source, as a line and column, and back.
//!
//! This is the one definition of a source position in MESH. The terminal
//! renderer, the JSON output and the language server all go through it,
//! so every presentation of a position agrees by construction.

use std::ops::Range;

/// The UTF-8 byte-order mark, as `fs::read_to_string` leaves it at the
/// start of a file saved "with BOM".
const BOM: char = '\u{feff}';

/// What a column counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColumnUnit {
    /// Unicode scalar values (`char`s): the unit of `mesh check`'s human
    /// and JSON output. LSP's `utf-32` encoding counts the same way.
    Char,
    /// UTF-8 bytes.
    Utf8,
    /// UTF-16 code units: the default unit of LSP positions.
    Utf16,
}

impl ColumnUnit {
    fn width(self, c: char) -> usize {
        match self {
            ColumnUnit::Char => 1,
            ColumnUnit::Utf8 => c.len_utf8(),
            ColumnUnit::Utf16 => c.len_utf16(),
        }
    }
}

/// A 0-based line and column. What the column counts depends on the
/// [`ColumnUnit`] it was computed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LineColumn {
    pub line: usize,
    pub column: usize,
}

/// A map from byte offsets to lines and columns, and back, for one
/// source text.
///
/// Build it once with [`SourceMap::new`] and pass the same text to each
/// method: the map holds only where each line starts, not the text, so it
/// can be kept beside the text it describes. Every method is total. No
/// offset or position makes one panic:
///
/// - A line ends at `\n`. A `\r` just before it isn't part of the line's
///   text, so LF and CRLF files give the same columns. A lone `\r` is
///   ordinary text, as it is in `mesh check`'s output; LSP reads one as a
///   line break, so positions in such a file disagree with an editor's.
/// - A leading byte-order mark isn't text: an offset on it is line 0,
///   column 0.
/// - An offset past the end clamps to the end, and one inside a
///   multi-byte character floors to that character's start. One inside a
///   line ending is just past the line's last visible character.
/// - A line past the last clamps to the end of the file, and a column
///   past the end of a line clamps to the end of its visible text. A
///   column inside a character (a UTF-16 column between the two halves of
///   a surrogate pair, or a UTF-8 column inside a multi-byte sequence)
///   floors to that character's start.
///
/// Lines and columns are 0-based. The CLI's output adds one to each.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMap {
    /// The byte offset each line starts at: 0, then one past every `\n`.
    line_starts: Vec<usize>,
    /// The length of the source, to catch a map used with another text.
    len: usize,
    /// The length of a leading byte-order mark: 0 or 3.
    bom: usize,
}

impl SourceMap {
    /// Maps `source`. O(n) in its length.
    pub fn new(source: &str) -> SourceMap {
        let line_starts = std::iter::once(0)
            .chain(source.match_indices('\n').map(|(i, _)| i + 1))
            .collect();
        SourceMap {
            line_starts,
            len: source.len(),
            bom: if source.starts_with(BOM) {
                BOM.len_utf8()
            } else {
                0
            },
        }
    }

    /// The number of lines. An empty source has one, empty line, and a
    /// source that ends in `\n` has an empty last line after it.
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// The byte range of `line`'s visible text: no leading byte-order
    /// mark, no trailing `\r`, no `\n`. A line past the last is the empty
    /// range at the end of the source.
    pub fn line_text(&self, source: &str, line: usize) -> Range<usize> {
        self.check(source);
        let Some(&line_start) = self.line_starts.get(line) else {
            return self.len..self.len;
        };
        let start = if line == 0 { self.bom } else { line_start };
        let end = self
            .line_starts
            .get(line + 1)
            .map_or(self.len, |next| next - 1);
        let end = start + source[start..end].trim_end_matches('\r').len();
        start..end
    }

    /// The line and column of `byte`, counted in `unit`.
    pub fn line_column(&self, source: &str, byte: usize, unit: ColumnUnit) -> LineColumn {
        self.check(source);
        let byte = source.floor_char_boundary(byte);
        // The last line that starts at or before `byte`. Line 0 starts at
        // 0, so there always is one.
        let line = self.line_starts.partition_point(|&start| start <= byte) - 1;
        let text = self.line_text(source, line);
        let byte = byte.clamp(text.start, text.end);
        let column = source[text.start..byte]
            .chars()
            .map(|c| unit.width(c))
            .sum();
        LineColumn { line, column }
    }

    /// The byte offset of `position`, whose column counts `unit`.
    pub fn offset(&self, source: &str, position: LineColumn, unit: ColumnUnit) -> usize {
        self.check(source);
        if position.line >= self.line_count() {
            return self.len;
        }
        let text = self.line_text(source, position.line);
        let mut column = 0;
        for (index, c) in source[text.clone()].char_indices() {
            let width = unit.width(c);
            // Floors: a column that lands inside `c` is `c`'s start.
            if column + width > position.column {
                return text.start + index;
            }
            column += width;
        }
        text.end
    }

    /// Catches a map used with a text it wasn't built from, in debug
    /// builds. That is a caller's bug: the totality guarantees above hold
    /// only for the text the map was built from.
    fn check(&self, source: &str) {
        debug_assert_eq!(
            source.len(),
            self.len,
            "a SourceMap must be used with the text it was built from"
        );
    }
}
