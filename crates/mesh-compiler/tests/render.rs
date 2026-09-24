use mesh_compiler::render_diagnostic;
use mesh_syntax::{Diagnostic, DiagnosticCode, Severity, Span, Suggestion};

fn diagnostic(severity: Severity, message: &str, start_byte: usize, end_byte: usize) -> Diagnostic {
    Diagnostic {
        severity,
        code: DiagnosticCode::SYNTAX_ERROR,
        message: message.to_string(),
        span: Span {
            start_byte,
            end_byte,
        },
        suggestions: Vec::new(),
    }
}

#[test]
fn renders_a_single_line_error_with_snippet_and_carets() {
    let source = "<div></span>\n";
    let rendered = render_diagnostic(
        source,
        "page.mprx",
        &Diagnostic {
            code: DiagnosticCode::MISMATCHED_CLOSING_TAG,
            ..diagnostic(Severity::Error, "mismatched closing tag", 0, 12)
        },
    );
    assert_eq!(
        rendered,
        "error[mismatched-closing-tag]: mismatched closing tag\n \
         --> page.mprx:1:1\n  \
         |\n\
         1 | <div></span>\n  \
         | ^^^^^^^^^^^^"
    );
}

#[test]
fn labels_warnings_as_warnings() {
    let source = r#"<div class="a" class="b" />"#;
    let rendered = render_diagnostic(
        source,
        "page.mprx",
        &Diagnostic {
            code: DiagnosticCode::DUPLICATE_ATTRIBUTE,
            ..diagnostic(Severity::Warning, "duplicate attribute", 5, 14)
        },
    );
    assert_eq!(
        rendered,
        "warning[duplicate-attribute]: duplicate attribute\n \
         --> page.mprx:1:6\n  \
         |\n\
         1 | <div class=\"a\" class=\"b\" />\n  \
         |      ^^^^^^^^^"
    );
}

#[test]
fn reports_line_and_column_for_a_span_on_a_later_line() {
    let source = "<card>\n  <title>Users</heading>\n</card>\n";
    let rendered = render_diagnostic(
        source,
        "card.mprx",
        &diagnostic(Severity::Error, "m", 9, 31),
    );
    assert_eq!(
        rendered,
        "error[syntax-error]: m\n \
         --> card.mprx:2:3\n  \
         |\n\
         2 |   <title>Users</heading>\n  \
         |   ^^^^^^^^^^^^^^^^^^^^^^"
    );
}

#[test]
fn underlines_only_the_first_line_of_a_multi_line_span() {
    let source = "<a />\n<b />\n";
    let rendered = render_diagnostic(source, "x.mprx", &diagnostic(Severity::Error, "m", 0, 12));
    assert_eq!(
        rendered,
        "error[syntax-error]: m\n \
         --> x.mprx:1:1\n  \
         |\n\
         1 | <a />\n  \
         | ^^^^^"
    );
}

#[test]
fn renders_a_single_caret_for_a_zero_length_span_in_empty_source() {
    let rendered = render_diagnostic(
        "",
        "empty.mprx",
        &diagnostic(Severity::Error, "syntax error", 0, 0),
    );
    assert_eq!(
        rendered,
        "error[syntax-error]: syntax error\n \
         --> empty.mprx:1:1\n  \
         |\n\
         1 |\n  \
         | ^"
    );
}

#[test]
fn counts_columns_in_characters_not_bytes() {
    // "héllo " is 6 chars but 7 bytes; "wörld" is 5 chars but 6 bytes.
    let source = "héllo wörld";
    let rendered = render_diagnostic(source, "u.mprx", &diagnostic(Severity::Error, "m", 7, 13));
    assert_eq!(
        rendered,
        "error[syntax-error]: m\n \
         --> u.mprx:1:7\n  \
         |\n\
         1 | héllo wörld\n  \
         |       ^^^^^"
    );
}

#[test]
fn underline_preserves_tabs_so_carets_stay_aligned() {
    let source = "<a>\n\t<b />\n</a>";
    let rendered = render_diagnostic(source, "t.mprx", &diagnostic(Severity::Error, "m", 5, 10));
    assert_eq!(
        rendered,
        "error[syntax-error]: m\n \
         --> t.mprx:2:2\n  \
         |\n\
         2 | \t<b />\n  \
         | \t^^^^^"
    );
}

#[test]
fn strips_carriage_returns_from_crlf_sources() {
    // The span covers "<b />\r\n"; neither the \r nor the \n is shown or underlined.
    let source = "<a>\r\n<b />\r\n";
    let rendered = render_diagnostic(source, "w.mprx", &diagnostic(Severity::Error, "m", 5, 12));
    assert_eq!(
        rendered,
        "error[syntax-error]: m\n \
         --> w.mprx:2:1\n  \
         |\n\
         2 | <b />\n  \
         | ^^^^^"
    );
}

#[test]
fn span_starting_in_a_crlf_line_ending_points_just_past_the_visible_line() {
    // Byte 3 is the `\r`, byte 4 the `\n`: both render as column 4 with one caret.
    let source = "<a>\r\n<b />";
    let expected = "error[syntax-error]: m\n \
         --> c.mprx:1:4\n  \
         |\n\
         1 | <a>\n  \
         |    ^";
    for start in [3, 4] {
        let rendered = render_diagnostic(
            source,
            "c.mprx",
            &diagnostic(Severity::Error, "m", start, start + 1),
        );
        assert_eq!(rendered, expected, "span starting at byte {start}");
    }
}

#[test]
fn span_ending_inside_a_crlf_line_ending_underlines_only_visible_text() {
    let source = "<a>\r\n";
    let rendered = render_diagnostic(source, "c.mprx", &diagnostic(Severity::Error, "m", 0, 4));
    assert_eq!(
        rendered,
        "error[syntax-error]: m\n \
         --> c.mprx:1:1\n  \
         |\n\
         1 | <a>\n  \
         | ^^^"
    );
}

#[test]
fn widens_the_gutter_for_multi_digit_line_numbers() {
    let source = "\n\n\n\n\n\n\n\n\n<a>";
    let rendered = render_diagnostic(source, "l.mprx", &diagnostic(Severity::Error, "m", 9, 12));
    assert_eq!(
        rendered,
        "error[syntax-error]: m\n  \
         --> l.mprx:10:1\n   \
         |\n\
         10 | <a>\n   \
         | ^^^"
    );
}

#[test]
fn clamps_an_out_of_range_span_instead_of_panicking() {
    let rendered = render_diagnostic("<a>", "o.mprx", &diagnostic(Severity::Error, "m", 10, 20));
    assert_eq!(
        rendered,
        "error[syntax-error]: m\n \
         --> o.mprx:1:4\n  \
         |\n\
         1 | <a>\n  \
         |    ^"
    );
}

#[test]
fn floors_a_span_that_splits_a_multi_byte_character() {
    // Byte 1 is inside "é" (bytes 0..2), so the start floors to 0.
    let rendered = render_diagnostic("é", "b.mprx", &diagnostic(Severity::Error, "m", 1, 2));
    assert_eq!(
        rendered,
        "error[syntax-error]: m\n \
         --> b.mprx:1:1\n  \
         |\n\
         1 | é\n  \
         | ^"
    );
}

#[test]
fn skips_a_leading_byte_order_mark() {
    // The BOM is bytes 0..3; `<div></span>` is bytes 3..15.
    let source = "\u{feff}<div></span>\n";
    let rendered = render_diagnostic(source, "bom.mprx", &diagnostic(Severity::Error, "m", 3, 15));
    assert_eq!(
        rendered,
        "error[syntax-error]: m\n \
         --> bom.mprx:1:1\n  \
         |\n\
         1 | <div></span>\n  \
         | ^^^^^^^^^^^^"
    );
}

#[test]
fn a_span_starting_on_the_byte_order_mark_points_at_the_first_character() {
    // v0.1 syntax errors span the whole document (0..len), and an empty
    // span at 0 is also possible: both start on the BOM itself.
    let source = "\u{feff}<a>";
    for (start, end, carets) in [(0, 6, "^^^"), (0, 0, "^"), (1, 2, "^")] {
        let rendered = render_diagnostic(
            source,
            "bom.mprx",
            &diagnostic(Severity::Error, "m", start, end),
        );
        assert_eq!(
            rendered,
            format!("error[syntax-error]: m\n --> bom.mprx:1:1\n  |\n1 | <a>\n  | {carets}"),
            "span {start}..{end}"
        );
    }
}

#[test]
fn renders_a_file_holding_only_a_byte_order_mark_like_an_empty_file() {
    let rendered = render_diagnostic(
        "\u{feff}",
        "bom.mprx",
        &diagnostic(Severity::Error, "m", 0, 0),
    );
    assert_eq!(
        rendered,
        "error[syntax-error]: m\n \
         --> bom.mprx:1:1\n  \
         |\n\
         1 |\n  \
         | ^"
    );
}

#[test]
fn skips_a_byte_order_mark_in_a_crlf_source() {
    // `<a>` is bytes 3..6, then `\r\n`.
    let source = "\u{feff}<a>\r\n<b />";
    let rendered = render_diagnostic(source, "bom.mprx", &diagnostic(Severity::Error, "m", 3, 8));
    assert_eq!(
        rendered,
        "error[syntax-error]: m\n \
         --> bom.mprx:1:1\n  \
         |\n\
         1 | <a>\n  \
         | ^^^"
    );
}

#[test]
fn a_byte_order_mark_does_not_shift_later_lines() {
    // `<b />` is bytes 7..12. This already passes; it pins that the BOM
    // handling only ever touches line 1.
    let source = "\u{feff}<a>\n<b />";
    let rendered = render_diagnostic(source, "bom.mprx", &diagnostic(Severity::Error, "m", 7, 12));
    assert_eq!(
        rendered,
        "error[syntax-error]: m\n \
         --> bom.mprx:2:1\n  \
         |\n\
         2 | <b />\n  \
         | ^^^^^"
    );
}

/// The renderer's contract: any `(source, span)` pair renders without
/// panicking, and never echoes a byte-order mark or a line-ending `\r`.
/// Pass 1's located syntax errors add zero-width spans and spans at byte
/// 0 of BOM files, so this sweeps every span, zero-width, reversed and
/// out-of-range ones included, over sources that combine a BOM, CRLF line
/// endings and multi-byte characters.
#[test]
fn never_panics_for_any_span_over_bom_crlf_and_multi_byte_sources() {
    let sources = [
        "",
        "\u{feff}",
        "\u{feff}\r\n",
        "\u{feff}<a>\r\n<b />",
        "\u{feff}é\r\nü",
        "héllo\r\nwörld\n",
        "\r\n\r\n",
    ];
    for source in sources {
        for start in 0..=source.len() + 2 {
            for end in 0..=source.len() + 2 {
                let rendered = render_diagnostic(
                    source,
                    "p.mprx",
                    &diagnostic(Severity::Error, "m", start, end),
                );
                assert!(
                    !rendered.contains(['\u{feff}', '\r']),
                    "span {start}..{end} of {source:?}: {rendered:?}"
                );
            }
        }
    }
}

/// Pass 1's located spans through Pass 0's renderer. Each source has a
/// byte-order mark, and most have CRLF line endings and multi-byte text;
/// the empty-file and missing-operand spans are zero-width. Every span
/// the compiler emits lies on character boundaries inside the source, and
/// renders at the line and column an editor shows.
#[test]
fn renders_located_syntax_errors_in_bom_crlf_and_multi_byte_sources() {
    let cases = [
        (
            "\u{feff}",
            "error[syntax-error]: expected a root element, but the file is empty\n \
             --> x.mprx:1:1\n  \
             |\n\
             1 |\n  \
             | ^",
        ),
        (
            "\u{feff}<page",
            "error[unterminated-tag]: unterminated tag `<page`: expected `>` or `/>`\n \
             --> x.mprx:1:1\n  \
             |\n\
             1 | <page\n  \
             | ^^^^^",
        ),
        (
            "\u{feff}<page>\r\n  <text>Größe < 10</text>\r\n</page>\r\n",
            "error[less-than-in-text]: `<` in text starts a tag; to show a literal `<`, put the text in a string expression, like `{\"a < b\"}`\n \
             --> x.mprx:2:15\n  \
             |\n\
             2 |   <text>Größe < 10</text>\n  \
             |               ^",
        ),
        (
            "\u{feff}<page title=\"é\" data-id=\"ü\" />\r\n",
            "error[hyphenated-attribute-name]: attribute name `data-id` can't contain `-`; use camelCase or `_` instead\n \
             --> x.mprx:1:17\n  \
             |\n\
             1 | <page title=\"é\" data-id=\"ü\" />\n  \
             |                 ^^^^^^^",
        ),
        (
            "\u{feff}<page>{\"é\" +}</page>\r\n",
            "error[syntax-error]: expected an expression\n \
             --> x.mprx:1:13\n  \
             |\n\
             1 | <page>{\"é\" +}</page>\n  \
             |             ^",
        ),
    ];
    for (source, expected) in cases {
        let result = mesh_compiler::compile(source);
        assert_eq!(result.diagnostics.len(), 1, "source: {source:?}");
        let diagnostic = &result.diagnostics[0];
        let span = diagnostic.span;
        assert!(
            span.start_byte <= span.end_byte
                && span.end_byte <= source.len()
                && source.is_char_boundary(span.start_byte)
                && source.is_char_boundary(span.end_byte),
            "span {span:?} of {source:?}"
        );
        assert_eq!(
            render_diagnostic(source, "x.mprx", diagnostic),
            expected,
            "source: {source:?}"
        );
    }
}

#[test]
fn renders_each_suggestion_as_a_help_line() {
    let source = "<page>\n  <card user={usr} />\n</page>\n";
    let rendered = render_diagnostic(
        source,
        "x.mprx",
        &Diagnostic {
            suggestions: vec![Suggestion {
                replacement: "user".to_string(),
                span: Span {
                    start_byte: 21,
                    end_byte: 24,
                },
            }],
            ..diagnostic(Severity::Error, "unknown reference \"usr\"", 21, 24)
        },
    );
    assert_eq!(
        rendered,
        "error[syntax-error]: unknown reference \"usr\"\n \
         --> x.mprx:2:15\n  \
         |\n\
         2 |   <card user={usr} />\n  \
         |               ^^^\n  \
         = help: did you mean \"user\"?"
    );
}

#[test]
fn a_diagnostic_without_suggestions_has_no_help_line() {
    let rendered = render_diagnostic(
        "<page />",
        "x.mprx",
        &diagnostic(Severity::Error, "boom", 1, 5),
    );
    assert!(!rendered.contains("help"), "{rendered}");
    assert!(rendered.ends_with("^^^^"), "{rendered}");
}
