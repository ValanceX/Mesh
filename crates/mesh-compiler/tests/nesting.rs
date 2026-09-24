//! The whole pipeline fits in a small stack at the nesting limit.
//!
//! `mesh-parser` rejects a file nested more than `MAX_NESTING_DEPTH`
//! levels deep, so every recursive walk after it (lowering, validation,
//! analysis, rendering, and the tree types' derived `Clone`, `PartialEq`
//! and `Drop`) is bounded. These tests pin that bound to something an
//! embedder can rely on: a file nested exactly to the limit compiles,
//! against a manifest, on a thread with a 2 MiB stack, the Rust default
//! for spawned threads. Tests run unoptimized, which uses more stack per
//! level than a release build.

use mesh_parser::MAX_NESTING_DEPTH;

const STACK: usize = 2 * 1024 * 1024;

/// `a`'s prop `x` accepts anything, its event `e` has a payload, and its
/// template has `y` (anything) in scope and a command `f`.
const MANIFEST: &str = r#"{ "version": 1, "types": {}, "components": {
    "a": {
        "props": { "x": { "type": { "kind": "any" }, "required": false } },
        "events": { "e": { "payload": { "kind": "any" } } },
        "commands": { "f": { "parameters": [ { "name": "v", "type": { "kind": "any" } } ] } },
        "scope": { "y": { "kind": "any" } }
    }
} }"#;

/// Files nested exactly `MAX_NESTING_DEPTH` levels deep, one per way of
/// nesting. An expression in `<a x={...} />` starts 2 levels deep, and
/// each of these nests `p + 1` expressions, so `p` is the limit minus 2.
fn sources() -> Vec<String> {
    let p = MAX_NESTING_DEPTH - 2;
    let attribute = |expression: String| format!("<a x={{{expression}}} />");
    vec![
        format!(
            "{}{}",
            "<a>".repeat(MAX_NESTING_DEPTH),
            "</a>".repeat(MAX_NESTING_DEPTH)
        ),
        attribute(format!("{}y{}", "(".repeat(p), ")".repeat(p))),
        attribute(format!("{}y", "!".repeat(p))),
        attribute(format!("{}y", "-".repeat(p))),
        attribute(format!("y{}", ".a".repeat(p))),
        attribute(format!("{}y{}", "[".repeat(p), "]".repeat(p))),
        attribute(format!("{}y{}", "{k: ".repeat(p), "}".repeat(p))),
        attribute(format!("y{}", " + y".repeat(p))),
        attribute(format!("y{}", " == y".repeat(p))),
        attribute(format!("{}y", "y ? y : ".repeat(p))),
        // Commands are allowed only as a handler, so every inner one is
        // reported, and a mistake is reported at every level.
        format!("<a on.e={{{}$event{}}} />", "f(".repeat(p), ")".repeat(p)),
        attribute(format!("{}usr{}", "[".repeat(p), "]".repeat(p))),
        // Elements and expressions together.
        format!(
            "{}<a x={{{}y}} />{}",
            "<a>".repeat(MAX_NESTING_DEPTH / 2),
            "!".repeat(MAX_NESTING_DEPTH / 2 - 3),
            "</a>".repeat(MAX_NESTING_DEPTH / 2)
        ),
    ]
}

/// Compiles, renders and copies `source` the way a program would, and
/// returns whether it produced IR and every diagnostic's code.
fn exercise(source: &str) -> (bool, Vec<&'static str>) {
    let manifest = mesh_manifest::load(MANIFEST).expect("a valid manifest");
    let template = manifest.template("a").expect("declared");
    let options = mesh_compiler::CompileOptions::with_template(template);
    let result = mesh_compiler::compile_with(source, &options);
    for diagnostic in &result.diagnostics {
        mesh_compiler::render_diagnostic(source, "deep.mprx", diagnostic);
    }
    mesh_compiler::render_json(source, "deep.mprx", &result.diagnostics);
    let copy = result.clone();
    assert_eq!(copy, result);
    assert!(!format!("{:?}", result.ir).is_empty());
    let codes = result
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect();
    (result.ir.is_some(), codes)
}

/// Runs `test` on a thread with a [`STACK`]-byte stack.
fn on_small_stack<T: Send + 'static>(test: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(STACK)
        .spawn(test)
        .expect("spawns a thread")
        .join()
        .expect("the thread doesn't panic or overflow")
}

#[test]
fn a_file_nested_to_the_limit_compiles_on_a_small_stack() {
    for (index, source) in sources().into_iter().enumerate() {
        let (has_ir, codes) = on_small_stack(move || exercise(&source));
        assert!(has_ir, "source {index}: no IR");
        assert!(
            !codes.contains(&"nesting-too-deep"),
            "source {index}: {codes:?}"
        );
    }
}

#[test]
fn a_file_nested_past_the_limit_compiles_to_one_error() {
    let source = format!(
        "{}{}",
        "<a>".repeat(MAX_NESTING_DEPTH + 1),
        "</a>".repeat(MAX_NESTING_DEPTH + 1)
    );
    let (has_ir, codes) = on_small_stack(move || exercise(&source));
    assert!(!has_ir);
    assert_eq!(codes, ["nesting-too-deep"]);
}

/// A list type nested `levels` deep around `kind`: `list<list<...>>`.
fn deep_list(levels: usize, kind: &str) -> String {
    format!(
        r#"{}{{ "kind": "{kind}" }}{}"#,
        r#"{ "kind": "list", "element": "#.repeat(levels),
        " }".repeat(levels)
    )
}

#[test]
fn a_manifest_type_nested_to_the_limit_is_checked_on_a_small_stack() {
    // The document, `types` and each list are one level of JSON each,
    // and the innermost type is one more.
    let levels = mesh_manifest::MAX_NESTING_DEPTH - 3;
    let manifest = format!(
        r#"{{ "version": 1, "types": {{ "Strings": {strings}, "Numbers": {numbers} }},
        "components": {{ "a": {{
            "props": {{ "x": {{ "type": {{ "kind": "named", "name": "Strings" }}, "required": true }} }},
            "events": {{}}, "commands": {{}},
            "scope": {{ "s": {{ "kind": "named", "name": "Strings" }}, "n": {{ "kind": "named", "name": "Numbers" }} }}
        }} }} }}"#,
        strings = deep_list(levels, "string"),
        numbers = deep_list(levels, "number"),
    );
    let codes = on_small_stack(move || {
        let manifest =
            mesh_manifest::load(&manifest).expect("a manifest nested to the limit loads");
        let template = manifest.template("a").expect("declared");
        let options = mesh_compiler::CompileOptions::with_template(template);
        let source = "<a x={s}><a x={n} /><a x={s == n ? s : s} /></a>";
        let result = mesh_compiler::compile_with(source, &options);
        for diagnostic in &result.diagnostics {
            mesh_compiler::render_diagnostic(source, "deep.mprx", diagnostic);
        }
        result
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<Vec<_>>()
    });
    assert_eq!(codes, ["type-mismatch", "no-common-type"]);
}
