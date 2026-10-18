//! Nesting limits (decision U12): a render nested too deeply, by composites
//! or by elements, is a diagnostic and never an internal error, and a
//! compiled template reads at the documented depth.

mod common;

use common::{codes, compile_with, snapshot};
use mesh_runtime::{render, Program};

const COMPONENT: &str = r#"{ "props": {}, "events": {}, "commands": {}, "scope": {} }"#;

/// A model with `box` and the chain `c0` to `c{depth}`.
fn model(depth: usize) -> String {
    let mut components = vec![format!(r#""box": {COMPONENT}"#)];
    components.extend((0..=depth).map(|i| format!(r#""c{i}": {COMPONENT}"#)));
    format!(
        r#"{{ "version": 1, "types": {{}}, "components": {{ {} }} }}"#,
        components.join(", ")
    )
}

/// `depth + 1` templates: `c{i}` is `per_level` boxes around `<c{i+1} />`,
/// and the last is `per_level` boxes around text.
fn chain(model: &str, depth: usize, per_level: usize) -> Vec<String> {
    (0..=depth)
        .map(|i| {
            let inner = if i < depth {
                format!("<c{} />", i + 1)
            } else {
                "end".to_string()
            };
            let source = format!(
                "{}{inner}{}",
                "<box>".repeat(per_level),
                "</box>".repeat(per_level)
            );
            compile_with(model, &format!("c{i}"), &source)
        })
        .collect()
}

/// Renders on a thread with a large stack: a debug build's frames are big
/// enough that the default test stack overflows well before the cap, and
/// the cap is a limit on the render, not a promise about any one stack.
fn rendered(depth: usize, per_level: usize) -> Result<(), Vec<&'static str>> {
    std::thread::Builder::new()
        .stack_size(512 << 20)
        .spawn(move || render_chain(depth, per_level))
        .unwrap()
        .join()
        .unwrap()
}

fn render_chain(depth: usize, per_level: usize) -> Result<(), Vec<&'static str>> {
    let model = model(depth);
    let templates = chain(&model, depth, per_level);
    let texts: Vec<&str> = templates.iter().map(String::as_str).collect();
    render(
        &Program {
            root: "c0",
            templates: &texts,
        },
        &model,
        &snapshot("{}"),
    )
    .map(|_| ())
    .map_err(|diagnostics| codes(&diagnostics))
}

#[test]
fn nesting_within_the_cap_renders() {
    assert_eq!(rendered(100, 1), Ok(()));
    assert_eq!(rendered(20, 10), Ok(()));
}

#[test]
fn composites_nested_past_the_cap_are_a_diagnostic() {
    for depth in [600, 1_000] {
        assert_eq!(
            rendered(depth, 1),
            Err(vec!["runtime-nesting-too-deep"]),
            "{depth} levels"
        );
    }
}

#[test]
fn elements_inside_composites_count_too() {
    // Ten elements a level is about 700 levels of nesting at 70 composites.
    assert_eq!(rendered(100, 10), Err(vec!["runtime-nesting-too-deep"]));
}

#[test]
fn a_template_at_the_documented_depth_reads_and_renders() {
    std::thread::Builder::new()
        .stack_size(512 << 20)
        .spawn(documented_depth)
        .unwrap()
        .join()
        .unwrap();
}

fn documented_depth() {
    // MPRX nests 128 levels (an element is one), and each is three levels of JSON.
    let model = model(0);
    let source = format!("{}end{}", "<box>".repeat(128), "</box>".repeat(128));
    let template = compile_with(&model, "c0", &source);
    let texts = [template.as_str()];
    let rendered = render(
        &Program {
            root: "c0",
            templates: &texts,
        },
        &model,
        &snapshot("{}"),
    );
    assert!(rendered.is_ok(), "{:?}", rendered.err().map(|d| codes(&d)));
}
