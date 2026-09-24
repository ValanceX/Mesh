//! `recover`: what the editor may still use of a file with syntax errors
//! (the recovery spec, `docs/superpowers/specs/2026-09-24-mesh-v0.3-editor-recovery.md`).
//!
//! One test per observed shape, S1–S18. Each expectation is a compact
//! summary of the parts: an element part as `tag [attributes and on.events]
//! (children)`, with whitespace-only text left out, and an expression part
//! as `expr: <its source text>`. A change in Tree-sitter's recovery that
//! moves a shape fails its test.

use mesh_parser::{recover, Recovered};
use mesh_syntax::{Child, Element};

fn summary(source: &str) -> Vec<String> {
    let Recovered {
        elements,
        expressions,
        ..
    } = recover(source);
    let mut out: Vec<String> = elements.iter().map(element).collect();
    out.extend(expressions.iter().map(|part| {
        format!(
            "expr: {}",
            &source[part.span.start_byte..part.span.end_byte]
        )
    }));
    out
}

fn element(element: &Element) -> String {
    let mut names: Vec<String> = element
        .attributes
        .iter()
        .map(|attribute| attribute.name.clone())
        .collect();
    names.extend(
        element
            .event_bindings
            .iter()
            .map(|binding| format!("on.{}", binding.name)),
    );
    let children: Vec<String> = element
        .children
        .iter()
        .filter_map(|child| match child {
            Child::Text(text) if text.value.trim().is_empty() => None,
            Child::Text(text) => Some(format!("{:?}", text.value.trim())),
            Child::Expression(_) => Some("{…}".to_string()),
            Child::Element(child) => Some(self::element(child)),
        })
        .collect();
    let mut out = element.name.clone();
    if !names.is_empty() {
        out.push_str(&format!(" [{}]", names.join(", ")));
    }
    if !children.is_empty() {
        out.push_str(&format!(" ({})", children.join(", ")));
    }
    out
}

/// `inner` as a child of a page, on its own line.
fn in_page(inner: &str) -> String {
    format!("<page title=\"U\">\n  {inner}\n</page>")
}

fn assert_parts(source: &str, expected: &[&str]) {
    assert_eq!(summary(source), expected, "recovering {source:?}");
}

#[test]
fn a_file_that_parses_is_one_whole_element() {
    let source = in_page("<text>{user.name}</text>\n  <avatar src={user.avatar} alt=\"x\" />");
    assert_parts(&source, &["page [title] (text ({…}), avatar [src, alt])"]);
}

#[test]
fn s1_a_dot_after_a_reference() {
    assert_parts(
        &in_page("<text>{user.}</text>"),
        &["page [title] (text)", "expr: user"],
    );
}

#[test]
fn s2_a_broken_attribute_is_dropped_and_its_neighbours_kept() {
    assert_parts(
        &in_page("<avatar src={user.} alt=\"x\" />"),
        &["page [title] (avatar [alt])", "expr: user"],
    );
}

#[test]
fn s3_the_clean_expression_before_the_dot() {
    assert_parts(
        &in_page("<text>{!user.}</text>"),
        &["page [title] (text)", "expr: !user"],
    );
    assert_parts(
        &in_page("<text>{user.avatar.}</text>"),
        &["page [title] (text)", "expr: user.avatar"],
    );
    // The block's expression is the invocation, which isn't clean.
    assert_parts(
        &in_page("<text>{selectUser(user.)}</text>"),
        &["page [title] (text)"],
    );
}

#[test]
fn s4_a_conditional_with_a_broken_branch_is_not_a_part() {
    assert_parts(
        &in_page("<text>{compact ? user. : \"x\"}</text>"),
        &["page [title] (text)"],
    );
}

#[test]
fn s5_an_unfinished_tag_is_still_an_element() {
    assert_parts(&in_page("<avatar "), &["page [title] (avatar)"]);
}

#[test]
fn s6_an_unfinished_tag_keeps_its_attributes() {
    assert_parts(
        &in_page("<avatar alt=\"x\" "),
        &["page [title] (avatar [alt])"],
    );
}

#[test]
fn s7_a_partial_tag_name_before_a_complete_element() {
    assert_parts(
        &in_page("<av\n  <text>hi</text>"),
        &["page [title] (av, text (\"hi\"))"],
    );
}

#[test]
fn s8_s9_loose_tokens_are_not_an_element() {
    assert_parts(&in_page("<avatar s"), &["page [title]"]);
    assert_parts(&in_page("<avatar src="), &["page [title]"]);
}

#[test]
fn s10_an_error_inside_a_tag_is_left_out() {
    assert_parts(
        &in_page("<avatar alt=\"x\" s />"),
        &["page [title] (avatar [alt])"],
    );
}

#[test]
fn s11_s12_an_unfinished_event_binding_is_left_out() {
    assert_parts(
        &in_page("<button on. >x</button>"),
        &["page [title] (button (\"x\"))"],
    );
    assert_parts(
        &in_page("<button on.cl >x</button>"),
        &["page [title] (button (\"x\"))"],
    );
}

#[test]
fn s13_a_missing_expression_is_not_a_part() {
    assert_parts(
        &in_page("<button on.click={} >x</button>"),
        &["page [title] (button (\"x\"))"],
    );
    assert_parts(&in_page("<text>{ }</text>"), &["page [title] (text)"]);
    assert_parts(
        &in_page("<text>{user.name + }</text>"),
        &["page [title] (text)"],
    );
}

#[test]
fn s14_an_unfinished_invocation_leaves_its_name() {
    assert_parts(
        &in_page("<button on.click={selectUser(} >x</button>"),
        &["page [title] (button (\"x\"))", "expr: selectUser"],
    );
}

#[test]
fn s15_a_lone_less_than() {
    assert_parts(&in_page("<avatar />\n  <"), &["page [title] (avatar)"]);
}

#[test]
fn s16_an_unclosed_block_leaves_a_forest() {
    let source =
        "<page title=\"U\">\n  <avatar alt=\"x\" src={user.avatar} />\n  <text>{user.\n</page>";
    assert_parts(source, &["avatar [alt, src]"]);
    let source = "<page title=\"U\">\n  <text>{user.name}</text>\n  <button on.click={selectUser($event)}>x</button>\n  <text>{user.nam\n</page>";
    assert_parts(source, &["text ({…})", "button [on.click] (\"x\")"]);
}

#[test]
fn s17_an_unclosed_string_leaves_nothing() {
    let source = "<page title=\"U\">\n  <avatar alt=\"x />\n  <text>{user.name}</text>\n</page>";
    assert_parts(source, &[]);
}

#[test]
fn s18_a_lone_less_than_as_the_whole_file() {
    assert_parts("<", &[]);
    assert_parts("", &[]);
    assert_parts("   \n", &[]);
}

#[test]
fn a_file_nested_too_deep_recovers_nothing() {
    let depth = mesh_parser::MAX_NESTING_DEPTH + 10;
    let source = format!("{}{{user.}}{}", "<a>".repeat(depth), "</a>".repeat(depth));
    assert_parts(&source, &[]);
}

#[test]
fn every_prefix_of_every_fixture_recovers_without_panicking() {
    let examples = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut files = vec![
        examples.join("users-page.mprx"),
        examples.join("user-card.mprx"),
    ];
    for dir in [
        "fixtures/pass",
        "fixtures/fail",
        "fixtures/check/pass",
        "fixtures/check/fail",
    ] {
        for entry in std::fs::read_dir(examples.join(dir)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "mprx") {
                files.push(path);
            }
        }
    }
    for file in files {
        let source = std::fs::read_to_string(&file).unwrap();
        for (end, _) in source.char_indices() {
            recover(&source[..end]);
        }
        recover(&source);
    }
}
