//! The source map: byte offsets to lines and columns, in every unit, and
//! back. These tests pin each rule the outline's D2 lists.

use mesh_compiler::{ColumnUnit, LineColumn, SourceMap};

const UNITS: [ColumnUnit; 3] = [ColumnUnit::Char, ColumnUnit::Utf8, ColumnUnit::Utf16];

fn at(line: usize, column: usize) -> LineColumn {
    LineColumn { line, column }
}

fn line_column(source: &str, byte: usize, unit: ColumnUnit) -> LineColumn {
    SourceMap::new(source).line_column(source, byte, unit)
}

fn offset(source: &str, position: LineColumn, unit: ColumnUnit) -> usize {
    SourceMap::new(source).offset(source, position, unit)
}

#[test]
fn counts_lines_and_columns_from_zero() {
    let source = "<a>\n  <b />\n</a>";
    for unit in UNITS {
        assert_eq!(line_column(source, 0, unit), at(0, 0));
        assert_eq!(line_column(source, 6, unit), at(1, 2));
        assert_eq!(line_column(source, 12, unit), at(2, 0));
        assert_eq!(line_column(source, source.len(), unit), at(2, 4));
        assert_eq!(offset(source, at(1, 2), unit), 6);
    }
    assert_eq!(SourceMap::new(source).line_count(), 3);
}

#[test]
fn columns_differ_by_unit() {
    // `é` is 2 bytes and 1 UTF-16 unit; `😀` is 4 bytes and 2 UTF-16 units.
    let source = "é😀x";
    let x = source.find('x').unwrap();
    assert_eq!(line_column(source, x, ColumnUnit::Char), at(0, 2));
    assert_eq!(line_column(source, x, ColumnUnit::Utf16), at(0, 3));
    assert_eq!(line_column(source, x, ColumnUnit::Utf8), at(0, 6));
    assert_eq!(offset(source, at(0, 2), ColumnUnit::Char), x);
    assert_eq!(offset(source, at(0, 3), ColumnUnit::Utf16), x);
    assert_eq!(offset(source, at(0, 6), ColumnUnit::Utf8), x);
}

#[test]
fn floors_offsets_inside_a_character() {
    let source = "a😀b";
    // Bytes 2, 3 and 4 are inside the emoji, which starts at byte 1.
    for byte in 2..=4 {
        assert_eq!(line_column(source, byte, ColumnUnit::Char), at(0, 1));
        assert_eq!(line_column(source, byte, ColumnUnit::Utf16), at(0, 1));
        assert_eq!(line_column(source, byte, ColumnUnit::Utf8), at(0, 1));
    }
    // A UTF-8 column inside the emoji floors to its start.
    assert_eq!(offset(source, at(0, 3), ColumnUnit::Utf8), 1);
}

#[test]
fn floors_utf16_columns_inside_a_surrogate_pair() {
    let source = "a😀b";
    // UTF-16 column 2 is between the emoji's two surrogates.
    assert_eq!(offset(source, at(0, 2), ColumnUnit::Utf16), 1);
    assert_eq!(offset(source, at(0, 3), ColumnUnit::Utf16), 5);
}

#[test]
fn clamps_past_the_end_of_the_file() {
    let source = "ab\ncd";
    for unit in UNITS {
        assert_eq!(line_column(source, 99, unit), at(1, 2));
        assert_eq!(offset(source, at(7, 0), unit), source.len());
    }
}

#[test]
fn clamps_past_the_end_of_a_line() {
    for source in ["ab\ncd", "ab\r\ncd"] {
        for unit in UNITS {
            // Column 99 of line 0 is the end of `ab`, never the `\r` or `\n`.
            assert_eq!(offset(source, at(0, 99), unit), 2, "{source:?}");
            // An offset inside the line ending is just past `ab`.
            for byte in 2..source.find('c').unwrap() {
                assert_eq!(line_column(source, byte, unit), at(0, 2), "{source:?}");
            }
        }
    }
}

#[test]
fn lf_and_crlf_give_the_same_columns() {
    let lf = "<a>\n  <b />";
    let crlf = "<a>\r\n  <b />";
    for unit in UNITS {
        let b_lf = lf.find("<b").unwrap();
        let b_crlf = crlf.find("<b").unwrap();
        assert_eq!(line_column(lf, b_lf, unit), line_column(crlf, b_crlf, unit));
        assert_eq!(offset(crlf, at(1, 2), unit), b_crlf);
    }
}

/// A lone `\r` is ordinary text, as it is in `mesh check`'s output. LSP
/// would read it as a line break; the outline records that as a known
/// limitation, and this test makes changing it a visible decision.
#[test]
fn a_lone_carriage_return_is_text() {
    let source = "a\rb";
    assert_eq!(line_column(source, 2, ColumnUnit::Char), at(0, 2));
    assert_eq!(SourceMap::new(source).line_count(), 1);
}

#[test]
fn a_byte_order_mark_is_not_a_column() {
    let source = "\u{feff}<a>\n<b>";
    for unit in UNITS {
        assert_eq!(line_column(source, 0, unit), at(0, 0));
        assert_eq!(line_column(source, 3, unit), at(0, 0));
        assert_eq!(line_column(source, 4, unit), at(0, 1));
        assert_eq!(offset(source, at(0, 0), unit), 3);
        assert_eq!(offset(source, at(0, 1), unit), 4);
        assert_eq!(offset(source, at(1, 0), unit), 7);
    }
    assert_eq!(SourceMap::new(source).line_text(source, 0), 3..6);
}

#[test]
fn zero_width_spans_stay_zero_width() {
    let source = "<a x={1 +} />";
    let byte = source.find('}').unwrap();
    for unit in UNITS {
        let start = line_column(source, byte, unit);
        let end = line_column(source, byte, unit);
        assert_eq!(start, end);
        assert_eq!(offset(source, start, unit), offset(source, end, unit));
    }
}

#[test]
fn an_empty_source_has_one_empty_line() {
    let map = SourceMap::new("");
    assert_eq!(map.line_count(), 1);
    assert_eq!(map.line_text("", 0), 0..0);
    for unit in UNITS {
        assert_eq!(map.line_column("", 5, unit), at(0, 0));
        assert_eq!(map.offset("", at(3, 3), unit), 0);
    }
}

#[test]
fn a_trailing_newline_starts_an_empty_last_line() {
    let source = "a\n";
    let map = SourceMap::new(source);
    assert_eq!(map.line_count(), 2);
    assert_eq!(map.line_column(source, 2, ColumnUnit::Char), at(1, 0));
    assert_eq!(map.line_text(source, 1), 2..2);
}

/// Where `byte` lands once normalized: floored to a character, and
/// clamped into its line's visible text (so an offset on the BOM or in a
/// line ending moves to the nearest visible position).
fn normalized(source: &str, map: &SourceMap, byte: usize) -> usize {
    let floored = source.floor_char_boundary(byte);
    let line = map.line_column(source, floored, ColumnUnit::Utf8).line;
    let text = map.line_text(source, line);
    floored.clamp(text.start, text.end)
}

#[test]
fn round_trips_every_byte() {
    let sources = [
        "\u{feff}<p a=\"é\">日本😀</p>\r\n<q />\n\n  x𝄞y\r\n",
        "plain\nascii\n",
        "😀😀\r\n😀",
    ];
    for source in sources {
        let map = SourceMap::new(source);
        for byte in 0..=source.len() + 2 {
            for unit in UNITS {
                let position = map.line_column(source, byte, unit);
                assert_eq!(
                    map.offset(source, position, unit),
                    normalized(source, &map, byte),
                    "{source:?} byte {byte} {unit:?}"
                );
            }
        }
    }
}

#[test]
fn utf16_offsets_count_code_units_from_the_start() {
    // The BOM is 1 unit, `é` 1, `😀` 2, and `\r\n` 2, so `x` is at 6.
    let source = "\u{feff}é😀\r\nx";
    let map = SourceMap::new(source);
    let x = source.find('x').unwrap();
    assert_eq!(map.utf16_offset(source, 0), 0);
    assert_eq!(map.utf16_offset(source, 3), 1);
    assert_eq!(map.utf16_offset(source, x), 6);
    // Inside `😀` floors to its start; past the end clamps to the end.
    assert_eq!(map.utf16_offset(source, source.find('😀').unwrap() + 2), 2);
    assert_eq!(map.utf16_offset(source, source.len() + 5), 7);
}

#[test]
fn utf16_offsets_round_trip_every_character() {
    let sources = [
        "\u{feff}<p a=\"é\">日本😀</p>\r\n<q />\n\n  x𝄞y\r\n",
        "plain\nascii\n",
        "😀😀\r\n😀",
        "",
    ];
    for source in sources {
        let map = SourceMap::new(source);
        for byte in 0..=source.len() + 2 {
            let floored = source.floor_char_boundary(byte);
            assert_eq!(
                map.utf16_offset(source, byte),
                source[..floored].encode_utf16().count(),
                "{source:?} byte {byte}"
            );
        }
    }
}
