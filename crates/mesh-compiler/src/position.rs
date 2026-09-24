//! A byte offset located for display: the 1-based line and column both
//! presentations of a diagnostic print, and the line's text.
//!
//! This is a thin adapter over [`SourceMap`], which holds every rule
//! about positions; nothing here computes a line or column itself.

use crate::source_map::{ColumnUnit, SourceMap};
use std::ops::Range;

/// A byte offset located on its line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Located {
    /// The 1-based line number.
    pub(crate) line: usize,
    /// The 1-based column, in `char`s.
    pub(crate) column: usize,
    /// The line's visible text, as a byte range: no leading byte-order
    /// mark, no trailing `\r`, no `\n`.
    pub(crate) text: Range<usize>,
    /// The offset, floored to a `char` boundary and clamped into `text`.
    pub(crate) offset: usize,
}

/// Locates `byte` in `source`, which `map` was built from. Never panics.
pub(crate) fn locate(map: &SourceMap, source: &str, byte: usize) -> Located {
    let at = map.line_column(source, byte, ColumnUnit::Char);
    let text = map.line_text(source, at.line);
    let offset = source.floor_char_boundary(byte).clamp(text.start, text.end);
    Located {
        line: at.line + 1,
        column: at.column + 1,
        text,
        offset,
    }
}
