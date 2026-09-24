//! Located syntax errors. Every shape `mesh-parser` classifies is pinned
//! here to its code and to the exact source text its span covers, along
//! with the shapes that must stay on the `syntax-error` fallback: a
//! correct `syntax-error` always beats a wrong specific code.

use mesh_syntax::DiagnosticCode;

/// One expected error: its code, the byte its span starts at, and the
/// source text the span covers.
type Located<'a> = (&'a str, usize, &'a str);

/// Parses `source` and returns every error as a [`Located`].
fn errors(source: &str) -> Vec<Located<'_>> {
    mesh_parser::parse(source)
        .errors
        .iter()
        .map(|error| {
            let span = error.span;
            (
                error.code.as_str(),
                span.start_byte,
                &source[span.start_byte..span.end_byte],
            )
        })
        .collect()
}

fn assert_located(cases: &[(&str, &[Located])]) {
    for (source, expected) in cases {
        assert_eq!(errors(source), *expected, "source: {source:?}");
    }
}

/// The message of the only error `source` produces.
fn only_message(source: &str) -> String {
    let errors = mesh_parser::parse(source).errors;
    assert_eq!(errors.len(), 1, "source: {source:?}, errors: {errors:?}");
    errors[0].message.clone()
}

#[test]
fn locates_an_unterminated_tag() {
    assert_located(&[
        ("<page", &[("unterminated-tag", 0, "<page")]),
        ("<page title=\"x\"\n", &[("unterminated-tag", 0, "<page")]),
        (
            "<page title=\"x\" <p />\n",
            &[("unterminated-tag", 0, "<page")],
        ),
        ("<page>a<b</page>\n", &[("unterminated-tag", 7, "<b")]),
        (
            "<page>\n  <p />\n  <q x=\"1\"\n</page>\n",
            &[("unterminated-tag", 17, "<q")],
        ),
        // The unfinished root also orphans its closing tag. That second,
        // follow-on report stays on the fallback.
        (
            "<page x=\"1\"\n  <p />\n</page>\n",
            &[
                ("unterminated-tag", 0, "<page"),
                ("syntax-error", 20, "</page>"),
            ],
        ),
    ]);
    assert_eq!(
        only_message("<page"),
        "unterminated tag `<page`: expected `>` or `/>`"
    );
}

#[test]
fn locates_a_missing_closing_tag() {
    assert_located(&[
        (
            "<page>\n  <title>Users</title>\n",
            &[("missing-closing-tag", 0, "<page>")],
        ),
        (
            "<page>\n  <p>x</p>\n",
            &[("missing-closing-tag", 0, "<page>")],
        ),
        ("<a>< /a>\n", &[("missing-closing-tag", 0, "<a>")]),
    ]);
    assert_eq!(
        only_message("<page>\n  <p>x</p>\n"),
        "`<page>` is never closed: expected `</page>`"
    );
}

#[test]
fn locates_a_less_than_sign_in_text() {
    assert_located(&[
        ("<p>a < b</p>\n", &[("less-than-in-text", 5, "<")]),
        (
            "<page>\n  <p>1 < 2 and 3 > 1</p>\n</page>\n",
            &[("less-than-in-text", 14, "<")],
        ),
        (
            "<page>\n  <p>a <= b</p>\n</page>\n",
            &[("less-than-in-text", 14, "<")],
        ),
        (
            "<page>\n  <p>x<5</p>\n</page>\n",
            &[("less-than-in-text", 13, "<")],
        ),
        (
            "<page>\n  <p>x < </p>\n</page>\n",
            &[("less-than-in-text", 14, "<")],
        ),
        (
            "<page>\n  <p>if a < b</p>\n  <q>{x}</q>\n</page>\n",
            &[("less-than-in-text", 17, "<")],
        ),
        // Multi-byte text before the `<` is located like any other text.
        (
            "<page>\n  <p>Größe < 10</p>\n</page>\n",
            &[("less-than-in-text", 20, "<")],
        ),
        (
            "<page>\n  <p>héllo < world</p>\n</page>\n",
            &[("less-than-in-text", 19, "<")],
        ),
    ]);
    assert_eq!(
        only_message("<p>a < b</p>\n"),
        "`<` in text starts a tag; to show a literal `<`, put the text in a string expression, \
         like `{\"a < b\"}`"
    );
}

#[test]
fn locates_a_hyphenated_attribute_name() {
    assert_located(&[
        (
            "<page data-id=\"x\" />\n",
            &[("hyphenated-attribute-name", 6, "data-id")],
        ),
        (
            "<page a-b={x} />\n",
            &[("hyphenated-attribute-name", 6, "a-b")],
        ),
        (
            "<page -a=\"x\" />\n",
            &[("hyphenated-attribute-name", 6, "-a")],
        ),
        (
            "<a data-={x} />\n",
            &[("hyphenated-attribute-name", 3, "data-")],
        ),
        (
            "<page aria-label=\"x\" data-id=\"y\" />\n",
            &[
                ("hyphenated-attribute-name", 6, "aria-label"),
                ("hyphenated-attribute-name", 21, "data-id"),
            ],
        ),
    ]);
    assert_eq!(
        only_message("<page data-id=\"x\" />\n"),
        "attribute name `data-id` can't contain `-`; use camelCase or `_` instead"
    );
}

/// A multi-byte character (or a non-ASCII space like U+00A0) right before
/// the hyphenated word must not be swallowed into the reported span: doing
/// so by one byte would land the span's start mid-character. The character
/// itself isn't a name character, so it's excluded from the located word;
/// widening stops at its next character boundary, not one byte in.
#[test]
fn hyphenated_attribute_name_after_a_multi_byte_character() {
    assert_located(&[
        (
            "<page é-id=\"x\" />\n",
            &[("hyphenated-attribute-name", 8, "-id")],
        ),
        (
            "<page ü-a=\"x\" />\n",
            &[("hyphenated-attribute-name", 8, "-a")],
        ),
        (
            "<page\u{a0}data-id=\"x\" />\n",
            &[("hyphenated-attribute-name", 7, "data-id")],
        ),
    ]);
}

#[test]
fn locates_a_single_brace_object() {
    assert_located(&[
        (
            "<page data={ k: \"v\" } />\n",
            &[("single-brace-object", 11, "{ k: \"v\" }")],
        ),
        (
            "<page data={ a: 1, b: 2 } />\n",
            &[("single-brace-object", 11, "{ a: 1, b: 2 }")],
        ),
        (
            "<page data={ \"k\": 1 } />\n",
            &[("single-brace-object", 11, "{ \"k\": 1 }")],
        ),
        (
            "<page>{ k: 1 }</page>\n",
            &[("single-brace-object", 6, "{ k: 1 }")],
        ),
        (
            "<page>\n  <title>x</title>\n  <p data={ k: \"v\" } />\n</page>\n",
            &[("single-brace-object", 36, "{ k: \"v\" }")],
        ),
    ]);
    assert_eq!(
        only_message("<page data={ k: \"v\" } />\n"),
        "an object needs its own braces inside `{...}`: write `{{ key: value }}`"
    );
}

#[test]
fn locates_a_trailing_comma_in_command_arguments() {
    assert_located(&[
        (
            "<page on.click={save(a, b,)} />\n",
            &[("command-trailing-comma", 25, ",")],
        ),
        (
            "<a x={save(a,) + 1} />\n",
            &[("command-trailing-comma", 12, ",")],
        ),
        (
            "<page on.click={go(a,)} >\n  <p>{go(b,)}</p>\n</page>\n",
            &[
                ("command-trailing-comma", 20, ","),
                ("command-trailing-comma", 36, ","),
            ],
        ),
    ]);
    assert_eq!(
        only_message("<page on.click={save(a, b,)} />\n"),
        "trailing comma in command arguments; remove the `,`"
    );
}

#[test]
fn locates_a_malformed_event_binding() {
    assert_located(&[
        (
            "<page on.={x} />\n",
            &[("malformed-event-binding", 6, "on.")],
        ),
        ("<page on.={x} />", &[("malformed-event-binding", 6, "on.")]),
        (
            "<page on.a.b={x} />\n",
            &[("malformed-event-binding", 6, "on.a.b")],
        ),
        (
            "<page on.my-event={x} />\n",
            &[("malformed-event-binding", 6, "on.my-event")],
        ),
        (
            "<page on.click=\"x\" />\n",
            &[("malformed-event-binding", 6, "on.click")],
        ),
        (
            "<a on.click />\n",
            &[("malformed-event-binding", 3, "on.click")],
        ),
        (
            "<page on.click={x} on.={y}>\n</page>\n",
            &[("malformed-event-binding", 19, "on.")],
        ),
        // Tree-sitter recovers both bindings as one region: one report.
        (
            "<page on.={x} on.a.b={y} />\n",
            &[("malformed-event-binding", 6, "on.")],
        ),
    ]);
    assert_eq!(
        only_message("<page on.a.b={x} />\n"),
        "malformed event binding `on.a.b`: expected `on.<event>={handler}`"
    );
}

#[test]
fn reports_an_empty_file_at_its_start() {
    for source in ["", "   \n", "\u{feff}", "\u{feff}  \n"] {
        assert_eq!(
            errors(source),
            [("syntax-error", 0, "")],
            "source: {source:?}"
        );
        assert_eq!(
            only_message(source),
            "expected a root element, but the file is empty"
        );
    }
}

#[test]
fn points_at_the_extra_root_element() {
    assert_located(&[
        ("<a />\n<b />\n", &[("syntax-error", 6, "<b />")]),
        ("<a />\n<b />\n<c />\n", &[("syntax-error", 12, "<c />")]),
        ("\u{feff}<a />\n<b />\n", &[("syntax-error", 9, "<b />")]),
    ]);
    assert_eq!(
        only_message("<a />\n<b />\n"),
        "expected a single root element, found another one here"
    );
}

#[test]
fn points_at_a_missing_operand() {
    assert_located(&[
        ("<page>{a +}</page>\n", &[("syntax-error", 10, "")]),
        ("<page>{a ? b :}</page>\n", &[("syntax-error", 14, "")]),
        ("<a>{}</a>\n", &[("syntax-error", 4, "")]),
    ]);
    assert_eq!(
        only_message("<page>{a +}</page>\n"),
        "expected an expression"
    );
}

/// Shapes too ambiguous to classify stay `syntax-error`, reported at the
/// narrowest region Tree-sitter gives, with the generic message.
#[test]
fn falls_back_to_syntax_error_for_unclassified_shapes() {
    assert_located(&[
        ("<page on:click={x} />\n", &[("syntax-error", 6, "on:")]),
        ("<page on.click={go(,)} />\n", &[("syntax-error", 19, ",")]),
        (
            "<page on.click={go(a,,b)} />\n",
            &[("syntax-error", 20, ",")],
        ),
        ("<page a=\"1\" b= />\n", &[("syntax-error", 12, "b=")]),
        ("<a b=\"1\" -/>\n", &[("syntax-error", 9, "-")]),
        ("<a /> hello\n", &[("syntax-error", 6, "hello")]),
        ("hello <a />\n", &[("syntax-error", 0, "hello")]),
        ("<page>{</page>\n", &[("syntax-error", 6, "{")]),
        (
            "<page>\n  <!-- c -->\n</page>\n",
            &[("syntax-error", 9, "<!-- c -->")],
        ),
        (
            "<page>\n  <p>a <$b</p>\n</page>\n",
            &[("syntax-error", 14, "<$b")],
        ),
        (
            "<page>\n  <p>{items[0]}</p>\n</page>\n",
            &[("syntax-error", 13, "items")],
        ),
        ("<a on. />\n", &[("syntax-error", 0, "<a on. />\n")]),
        // Which element is unclosed is a guess here, so the whole file is
        // reported rather than a possibly wrong element.
        (
            "<page>\n  <p>x\n</page>\n",
            &[("syntax-error", 0, "<page>\n  <p>x\n</page>\n")],
        ),
        (
            "<page>\n  <p>\n  <q>x</q>\n</page>\n",
            &[("syntax-error", 0, "<page>\n  <p>\n  <q>x</q>\n</page>\n")],
        ),
        (
            "<a></a>\n<b />\n",
            &[("syntax-error", 0, "<a></a>\n<b />\n")],
        ),
        // Tree-sitter recovers from this one with a single whole-file
        // `ERROR`, so there is no narrower region to report. The
        // classifier reports what the parser gives it; it doesn't guess a
        // span with Unicode-specific rules.
        (
            "<page>\n  <p>hello < wörld</p>\n</page>\n",
            &[(
                "syntax-error",
                0,
                "<page>\n  <p>hello < wörld</p>\n</page>\n",
            )],
        ),
    ]);
    assert_eq!(only_message("<page on:click={x} />\n"), "invalid syntax");
}

#[test]
fn locates_errors_in_crlf_and_bom_sources() {
    assert_located(&[
        (
            "<page>\r\n  <p>a < b</p>\r\n</page>\r\n",
            &[("less-than-in-text", 15, "<")],
        ),
        (
            "<page>\r\n  <card data-id=\"1\" />\r\n</page>\r\n",
            &[("hyphenated-attribute-name", 16, "data-id")],
        ),
        (
            "\u{feff}<page title=\"x\"\n",
            &[("unterminated-tag", 3, "<page")],
        ),
    ]);
}

#[test]
fn reports_every_error_in_a_file_in_source_order() {
    assert_located(&[(
        "<page on.click={go(a,)}>\n  <p>a < b</p>\n  <q data-id=\"1\" />\n  <r x={ k: 1 } />\n</page>\n",
        &[
            ("command-trailing-comma", 20, ","),
            ("less-than-in-text", 32, "<"),
            ("hyphenated-attribute-name", 45, "data-id"),
            ("single-brace-object", 67, "{ k: 1 }"),
        ],
    )]);
}

/// D17: sources v0.1 accepted, or rejected only during validation, are
/// untouched by syntax-error classification.
#[test]
fn leaves_valid_syntax_alone() {
    for source in [
        "<p>a - b</p>\n",
        "<p>a > b</p>\n",
        "<a x={a<b} />\n",
        "<page>{[1, 2,]}</page>\n",
        "<a x={ {k: 1} } />\n",
        "<my-card></my-card>\n",
        "<div></span>",
    ] {
        let result = mesh_parser::parse(source);
        assert!(
            result.errors.is_empty(),
            "source: {source:?}, errors: {:?}",
            result.errors
        );
        assert!(result.ast.is_some(), "source: {source:?}");
    }
}

#[test]
fn every_syntax_code_is_a_known_code() {
    let sources = [
        "<page",
        "<page>\n  <p>x</p>\n",
        "<p>a < b</p>\n",
        "<page data-id=\"x\" />\n",
        "<page data={ k: \"v\" } />\n",
        "<page on.click={save(a, b,)} />\n",
        "<page on.={x} />\n",
        "<page on:click={x} />\n",
    ];
    for source in sources {
        for error in mesh_parser::parse(source).errors {
            assert!(
                DiagnosticCode::ALL.contains(&error.code),
                "{:?} is not in DiagnosticCode::ALL",
                error.code
            );
        }
    }
}

/// D17 regression: v0.1 rejected every one of these with exit 1, however
/// deep the nesting. Locating the error must not overflow the stack
/// (SIGABRT) or take quadratic time on the way down: the walk is
/// iterative, and classification reads parents and siblings from the
/// walk's own path rather than Tree-sitter's `Node::parent()`, which
/// re-walks from the root and recurses once per level for a zero-width
/// `MISSING` node. 20,000 levels is far past where either used to break
/// on a test thread's stack, and each case covers a different classifier
/// path: a `MISSING` operand, a `MISSING` `/>` read three ancestors up, an
/// `ERROR` checked against its parent, a trailing comma found by walking
/// ancestors, and an `ERROR` checked against its siblings.
#[test]
fn locates_errors_in_deeply_nested_sources() {
    let depth = 20_000;
    let nest = |inner: &str| format!("{}{inner}{}", "<p>".repeat(depth), "</p>".repeat(depth));
    // Every case's error starts inside the innermost `<p>`, which ends at
    // this byte.
    let inside = 3 * depth;

    let cases = [
        (nest("{a +}"), ("syntax-error", inside + 4, "")),
        (nest("a < b"), ("less-than-in-text", inside + 2, "<")),
        (
            nest("<q data-id=\"x\" />"),
            ("hyphenated-attribute-name", inside + 3, "data-id"),
        ),
        (
            nest("<q x={f(a, b,)} />"),
            ("command-trailing-comma", inside + 12, ","),
        ),
        (
            nest("<q x={ k: 1 } />"),
            ("single-brace-object", inside + 5, "{ k: 1 }"),
        ),
    ];
    for (source, expected) in &cases {
        assert_eq!(
            errors(source),
            [*expected],
            "inner: {:?}",
            &source[inside..]
        );
    }
}

/// The walk keeps the path to the node it's visiting, cutting it back as
/// it moves from one branch to the next. Here the `c-d` error comes right
/// after a sibling attribute that holds an error of its own: it's only
/// classified as a hyphenated name because its parent is read as the
/// `<q ...>` tag, not as a node left over from the `{{b +}}` branch.
#[test]
fn reads_the_right_parent_after_a_sibling_with_an_error() {
    let depth = 20_000;
    let inner = "<q a={{b +}} c-d=\"1\" />";
    let deep = format!("{}{inner}{}", "<p>".repeat(depth), "</p>".repeat(depth));
    let inside = 3 * depth;

    assert_located(&[(
        inner,
        &[
            ("syntax-error", 7, "b +"),
            ("hyphenated-attribute-name", 13, "c-d"),
        ],
    )]);
    assert_eq!(
        errors(&deep),
        [
            ("syntax-error", inside + 7, "b +"),
            ("hyphenated-attribute-name", inside + 13, "c-d"),
        ]
    );
}

/// A file with thousands of sibling mistakes reports every one, in
/// source order.
#[test]
fn reports_thousands_of_sibling_errors_in_source_order() {
    let count = 2_000;
    let line = "<t>a < b</t>\n";
    let source = format!("<r>\n{}</r>\n", line.repeat(count));

    let expected: Vec<Located> = (0..count)
        .map(|i| ("less-than-in-text", 4 + i * line.len() + 5, "<"))
        .collect();
    assert_eq!(errors(&source), expected);
}
