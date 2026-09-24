//! The nesting limit. Every shape that nests is pinned to how many levels
//! it counts, by parsing it exactly at `MAX_NESTING_DEPTH` and one level
//! past it.

use mesh_parser::MAX_NESTING_DEPTH;

/// `levels` elements, each nested in the one before: `levels` deep.
fn elements(levels: usize) -> String {
    format!("{}{}", "<a>".repeat(levels), "</a>".repeat(levels))
}

/// An attribute expression whose innermost `y` is `depth` levels deep
/// (the element is one level, and `nest(p)` must add `p + 1` levels).
fn expression(depth: usize, nest: Nest) -> String {
    format!("<a x={{{}}} />", nest(depth - 2))
}

/// Writes an expression that nests `p` times.
type Nest = fn(usize) -> String;

/// Every way to nest an expression, each as a function of how many times
/// it nests (`p`). Each nests `p + 1` expressions deep.
fn shapes() -> Vec<(&'static str, Nest)> {
    vec![
        ("parentheses", |p| {
            format!("{}y{}", "(".repeat(p), ")".repeat(p))
        }),
        ("unary", |p| format!("{}y", "!".repeat(p))),
        ("member access", |p| format!("y{}", ".a".repeat(p))),
        ("arrays", |p| format!("{}y{}", "[".repeat(p), "]".repeat(p))),
        ("objects", |p| {
            format!("{}y{}", "{k: ".repeat(p), "}".repeat(p))
        }),
        ("binary", |p| format!("y{}", " + y".repeat(p))),
        ("conditional", |p| format!("{}y", "y ? y : ".repeat(p))),
        ("commands", |p| {
            format!("{}y{}", "f(".repeat(p), ")".repeat(p))
        }),
    ]
}

/// The code and covered source text of each error `source` produces.
fn errors(source: &str) -> Vec<(&'static str, &str)> {
    mesh_parser::parse(source)
        .errors
        .iter()
        .map(|error| {
            let span = error.span;
            (error.code.as_str(), &source[span.start_byte..span.end_byte])
        })
        .collect()
}

#[test]
fn elements_nest_up_to_the_limit() {
    let result = mesh_parser::parse(&elements(MAX_NESTING_DEPTH));
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert!(result.ast.is_some());
}

#[test]
fn an_element_past_the_limit_is_reported_at_its_tag_name() {
    let source = elements(MAX_NESTING_DEPTH + 1);
    let parsed = mesh_parser::parse(&source);
    assert_eq!(parsed.ast, None);
    assert_eq!(errors(&source), [("nesting-too-deep", "a")]);
    // The innermost `<a>` is the one too deep.
    assert_eq!(parsed.errors[0].span.start_byte, 3 * MAX_NESTING_DEPTH + 1);
    assert_eq!(
        parsed.errors[0].message,
        "this is nested more than 128 levels deep, the deepest MESH supports"
    );
}

#[test]
fn every_expression_shape_nests_up_to_the_limit() {
    for (name, nest) in shapes() {
        let source = expression(MAX_NESTING_DEPTH, nest);
        let result = mesh_parser::parse(&source);
        assert!(result.errors.is_empty(), "{name}: {:?}", result.errors);
        assert!(result.ast.is_some(), "{name}");
    }
}

#[test]
fn every_expression_shape_past_the_limit_is_reported() {
    for (name, nest) in shapes() {
        let source = expression(MAX_NESTING_DEPTH + 1, nest);
        let found = errors(&source);
        assert_eq!(found.len(), 1, "{name}: {found:?}");
        assert_eq!(found[0].0, "nesting-too-deep", "{name}");
    }
}

#[test]
fn elements_and_expressions_count_together() {
    let depth = |source: &str| errors(source).first().map(|(code, _)| *code);
    let nest = |elements: usize, parentheses: usize| {
        format!(
            "{}<b x={{{}y{}}} />{}",
            "<a>".repeat(elements),
            "(".repeat(parentheses),
            ")".repeat(parentheses),
            "</a>".repeat(elements)
        )
    };
    // `elements` + `<b>` + the outer expression + one per parenthesis.
    assert_eq!(depth(&nest(100, MAX_NESTING_DEPTH - 102)), None);
    assert_eq!(
        depth(&nest(100, MAX_NESTING_DEPTH - 101)),
        Some("nesting-too-deep")
    );
}

#[test]
fn only_the_first_place_too_deep_is_reported() {
    let deep = |name: &str| {
        format!(
            "<{name}>{}{}</{name}>",
            "<a>".repeat(MAX_NESTING_DEPTH),
            "</a>".repeat(MAX_NESTING_DEPTH)
        )
    };
    let source = format!("<root>{}{}</root>", deep("p"), deep("q"));
    assert_eq!(errors(&source), [("nesting-too-deep", "a")]);
    let error = &mesh_parser::parse(&source).errors[0];
    assert!(error.span.start_byte < source.find("<q>").unwrap());
}

/// Far past the limit, a valid file is rejected without the recursion
/// that would overflow the stack (this runs on a test thread, with a
/// smaller stack than a program's main thread).
#[test]
fn rejects_very_deep_files_without_overflowing_the_stack() {
    let sources = [
        elements(100_000),
        format!("<a x={{{}y}} />", "!".repeat(100_000)),
        format!("<a x={{y{}}} />", ".a".repeat(100_000)),
        format!("<a x={{y{}}} />", " + y".repeat(100_000)),
    ];
    for source in &sources {
        let found = errors(source);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, "nesting-too-deep");
    }
}
