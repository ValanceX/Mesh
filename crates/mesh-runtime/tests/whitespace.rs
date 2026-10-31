//! The space between two interpolations (docs/explanation/specs/2026-10-16-mprx-whitespace-between-interpolations.md, W-1 to W-3, W-9): a space written on the line is content; a line break
//! and the indentation after it are formatting, and so is whitespace next to anything but an interpolation.

mod common;

use common::view;
use mesh_runtime::{Node, TreeChild};

/// What the first `<text>` of a page draws: its text children, each as one string, in order.
fn texts(body: &str) -> Vec<String> {
    let render = view(&format!("<page title={{name}}>\n  {body}\n</page>")).unwrap();
    let root: &Node = &render.tree().root;
    let TreeChild::Node(first) = &root.children[0] else {
        panic!("an element first");
    };

    first
        .children
        .iter()
        .map(|child| match child {
            TreeChild::Text { text, .. } => text.clone(),
            TreeChild::Node(node) => format!("<{}>", node.component),
        })
        .collect()
}

#[test]
fn a_space_between_two_interpolations_is_part_of_the_text() {
    assert_eq!(
        texts("<text>{name} {name}</text>"),
        vec![format!("{n} {n}", n = rendered_name())]
    );
}

#[test]
fn two_spaces_are_kept_as_written() {
    assert_eq!(
        texts("<text>{name}  {name}</text>"),
        vec![format!("{n}  {n}", n = rendered_name())]
    );
}

#[test]
fn a_space_in_a_longer_run_is_one_run_with_the_text_around_it() {
    assert_eq!(
        texts("<text>#{count} {name} !</text>"),
        vec![format!("#{} {} !", rendered_count(), rendered_name())]
    );
}

#[test]
fn a_line_break_between_two_interpolations_is_formatting() {
    assert_eq!(
        texts("<text>{name}\n    {name}</text>"),
        vec![format!("{n}{n}", n = rendered_name())]
    );
}

#[test]
fn a_space_next_to_an_element_or_an_edge_is_still_dropped() {
    assert_eq!(texts("<text> {name} </text>"), vec![rendered_name()]);
    assert_eq!(
        texts("<text>{name} <probe /></text>"),
        vec![rendered_name(), "<probe>".to_string()]
    );
    assert_eq!(
        texts("<text><probe /> {name}</text>"),
        vec!["<probe>".to_string(), rendered_name()]
    );
}

fn rendered_name() -> String {
    let render = view("<page title={name}><text>{name}</text></page>").unwrap();
    let TreeChild::Node(first) = &render.tree().root.children[0] else {
        panic!("an element");
    };
    let TreeChild::Text { text, .. } = &first.children[0] else {
        panic!("text");
    };

    text.clone()
}

fn rendered_count() -> String {
    let render = view("<page title={name}><text>{count}</text></page>").unwrap();
    let TreeChild::Node(first) = &render.tree().root.children[0] else {
        panic!("an element");
    };
    let TreeChild::Text { text, .. } = &first.children[0] else {
        panic!("text");
    };

    text.clone()
}
