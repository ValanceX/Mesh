use mesh_compiler::render_diagnostic;
use mesh_syntax::{Diagnostic, Severity, Span};

fn diagnostic(severity: Severity, message: &str, start_byte: usize, end_byte: usize) -> Diagnostic {
    Diagnostic {
        severity,
        message: message.to_string(),
        span: Span {
            start_byte,
            end_byte,
        },
    }
}

#[test]
fn renders_a_single_line_error_with_snippet_and_carets() {
    let source = "<div></span>\n";
    let rendered = render_diagnostic(
        source,
        "page.mprx",
        &diagnostic(Severity::Error, "mismatched closing tag", 0, 12),
    );
    assert_eq!(
        rendered,
        "error: mismatched closing tag\n \
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
        &diagnostic(Severity::Warning, "duplicate attribute", 5, 14),
    );
    assert_eq!(
        rendered,
        "warning: duplicate attribute\n \
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
        "error: m\n \
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
        "error: m\n \
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
        "error: syntax error\n \
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
        "error: m\n \
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
        "error: m\n \
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
        "error: m\n \
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
    let expected = "error: m\n \
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
        "error: m\n \
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
        "error: m\n  \
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
        "error: m\n \
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
        "error: m\n \
         --> b.mprx:1:1\n  \
         |\n\
         1 | é\n  \
         | ^"
    );
}
