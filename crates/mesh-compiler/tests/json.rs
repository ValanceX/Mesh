use mesh_compiler::{compile, render_json};
use mesh_syntax::{Diagnostic, DiagnosticCode, Severity, Span, Suggestion};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

fn diagnostic(
    severity: Severity,
    code: DiagnosticCode,
    message: &str,
    span: (usize, usize),
) -> Diagnostic {
    Diagnostic {
        severity,
        code,
        message: message.to_string(),
        span: Span {
            start_byte: span.0,
            end_byte: span.1,
        },
        suggestions: Vec::new(),
    }
}

fn parse(document: &str) -> Value {
    serde_json::from_str(document).expect("render_json prints JSON")
}

fn validator() -> jsonschema::Validator {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas/diagnostics-v1.schema.json");
    let schema = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("should read {}: {err}", path.display()));
    let schema = serde_json::from_str(&schema).expect("the schema is JSON");
    jsonschema::draft202012::new(&schema).expect("the schema is a valid 2020-12 schema")
}

#[test]
fn renders_no_diagnostics_as_an_empty_list() {
    assert_eq!(
        render_json("<page />", "page.mprx", &[]),
        r#"{"version":1,"diagnostics":[]}"#
    );
}

#[test]
fn renders_each_diagnostic_on_one_line_in_order() {
    let source = "<div class=\"a\" class=\"b\">\n</span>";
    let document = render_json(
        source,
        "page.mprx",
        &[
            diagnostic(
                Severity::Warning,
                DiagnosticCode::DUPLICATE_ATTRIBUTE,
                "duplicate attribute \"class\"",
                (5, 14),
            ),
            diagnostic(
                Severity::Error,
                DiagnosticCode::MISMATCHED_CLOSING_TAG,
                "mismatched closing tag",
                (0, 33),
            ),
        ],
    );
    assert_eq!(
        document,
        concat!(
            r#"{"version":1,"diagnostics":["#,
            r#"{"severity":"warning","code":"duplicate-attribute","message":"duplicate attribute \"class\"","path":"page.mprx","#,
            r#""span":{"start":{"byte":5,"line":1,"column":6},"end":{"byte":14,"line":1,"column":15}},"suggestions":[]},"#,
            r#"{"severity":"error","code":"mismatched-closing-tag","message":"mismatched closing tag","path":"page.mprx","#,
            r#""span":{"start":{"byte":0,"line":1,"column":1},"end":{"byte":33,"line":2,"column":8}},"suggestions":[]}"#,
            r#"]}"#
        )
    );
    assert!(!document.contains('\n'));
}

#[test]
fn renders_suggestions_with_their_spans() {
    let source = "<card user={usr} />";
    let document = render_json(
        source,
        "card.mprx",
        &[Diagnostic {
            suggestions: vec![Suggestion {
                replacement: "user".to_string(),
                span: Span {
                    start_byte: 12,
                    end_byte: 15,
                },
            }],
            ..diagnostic(
                Severity::Error,
                DiagnosticCode::UNKNOWN_REFERENCE,
                "unknown reference \"usr\"",
                (12, 15),
            )
        }],
    );
    assert_eq!(
        parse(&document)["diagnostics"][0]["suggestions"],
        json!([{
            "replacement": "user",
            "span": {
                "start": { "byte": 12, "line": 1, "column": 13 },
                "end": { "byte": 15, "line": 1, "column": 16 }
            }
        }])
    );
}

/// The JSON location is the one the terminal shows, in every source the
/// renderer handles specially.
#[test]
fn positions_match_the_rendered_location() {
    let cases: [(&str, usize, usize); 4] = [
        ("\u{feff}<a>{x}</a>", 6, 9),
        ("<a>\r\n  <b>{x}</b>\r\n</a>", 13, 16),
        ("<p>Größe {x}</p>", 12, 15),
        ("<a>\n\t<b>{x}</b>\n</a>", 8, 11),
    ];
    for (source, start, end) in cases {
        let diagnostic = diagnostic(
            Severity::Error,
            DiagnosticCode::UNKNOWN_REFERENCE,
            "unknown reference",
            (start, end),
        );
        let rendered = mesh_compiler::render_diagnostic(source, "f.mprx", &diagnostic);
        let location = rendered
            .lines()
            .find_map(|line| line.strip_prefix(" --> f.mprx:"))
            .expect("a location line");
        let json = parse(&render_json(source, "f.mprx", &[diagnostic]));
        let start = &json["diagnostics"][0]["span"]["start"];
        assert_eq!(
            format!("{}:{}", start["line"], start["column"]),
            location,
            "{source:?}"
        );
    }
}

#[test]
fn an_end_position_counts_characters_too() {
    let source = "\u{feff}<p>Größe</p>\r\n";
    let end = source.find("</p>").unwrap();
    let json = parse(&render_json(
        source,
        "f.mprx",
        &[diagnostic(
            Severity::Error,
            DiagnosticCode::SYNTAX_ERROR,
            "x",
            (3, end),
        )],
    ));
    assert_eq!(
        json["diagnostics"][0]["span"],
        json!({
            "start": { "byte": 3, "line": 1, "column": 1 },
            "end": { "byte": end, "line": 1, "column": 9 }
        })
    );
}

#[test]
fn the_schema_accepts_what_render_json_prints() {
    let validator = validator();
    let sources = [
        "<page />",
        "<page",
        "<div class=\"a\" class=\"b\"></span>",
        "<p>a < b</p>\n<q>",
        "",
    ];
    for source in sources {
        let document = parse(&render_json(source, "f.mprx", &compile(source).diagnostics));
        let errors: Vec<String> = validator
            .iter_errors(&document)
            .map(|error| error.to_string())
            .collect();
        assert!(errors.is_empty(), "{source:?}: {errors:#?}");
    }
}

#[test]
fn the_schema_rejects_what_render_json_never_prints() {
    let validator = validator();
    let valid = parse(&render_json("<p>", "f.mprx", &compile("<p>").diagnostics));
    assert!(validator.is_valid(&valid));
    let mut unknown_severity = valid.clone();
    unknown_severity["diagnostics"][0]["severity"] = json!("note");
    let mut unknown_field = valid.clone();
    unknown_field["diagnostics"][0]["related"] = json!([]);
    let mut other_version = valid.clone();
    other_version["version"] = json!(2);
    for invalid in [unknown_severity, unknown_field, other_version] {
        assert!(!validator.is_valid(&invalid), "{invalid}");
    }
}

/// `code` is a pattern, not an enum: the schema accepts every code MESH
/// has, and any other the kebab-case rule `DiagnosticCode`'s own tests
/// enforce allows, digits included, so adding a code never needs a new
/// schema.
#[test]
fn the_schema_accepts_every_code() {
    let validator = validator();
    let mut document = parse(&render_json("<p>", "f.mprx", &compile("<p>").diagnostics));
    let codes = DiagnosticCode::ALL.iter().map(|code| code.as_str());
    for code in codes.chain(["code-2"]) {
        document["diagnostics"][0]["code"] = json!(code);
        assert!(validator.is_valid(&document), "{code}");
    }
}
