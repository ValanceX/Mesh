//! Blocks in the AST: `{#if}` with its branches, and `{#each}` with its binding and key.

use mesh_parser::parse;
use mesh_syntax::{Block, Child, Expression};

fn block(source: &str) -> Block {
    let ast = parse(source).ast.expect("parses");
    let found = ast.children.into_iter().find_map(|child| match child {
        Child::Block(block) => Some(*block),
        _ => None,
    });

    found.expect("a block among the children")
}

#[test]
fn a_conditional_has_its_branches_in_order_and_an_optional_else() {
    let source = "<a>{#if x}<b />{:else if y}text{:else}<c />{/if}</a>";
    let Block::If(found) = block(source) else {
        panic!("a conditional")
    };

    assert_eq!(found.branches.len(), 2);
    assert_eq!(
        &source[found.branches[0].keyword_span.start_byte..found.branches[0].keyword_span.end_byte],
        "{#if"
    );
    assert_eq!(
        &source[found.branches[1].keyword_span.start_byte..found.branches[1].keyword_span.end_byte],
        "{:else if"
    );
    assert!(
        matches!(&found.branches[1].condition, Expression::Reference(reference) if reference.name == "y")
    );
    assert_eq!(found.branches[1].children.len(), 1);
    let otherwise = found.otherwise.expect("an else");
    assert_eq!(
        &source[otherwise.keyword_span.start_byte..otherwise.keyword_span.end_byte],
        "{:else}"
    );
    assert_eq!(
        &source[found.span.start_byte..found.span.end_byte],
        "{#if x}<b />{:else if y}text{:else}<c />{/if}"
    );
}

#[test]
fn a_repeat_has_its_list_its_binding_and_its_key() {
    let source = "<a>{#each rows as row (row.id)}<b />{/each}</a>";
    let Block::Each(found) = block(source) else {
        panic!("a repeat")
    };

    assert!(matches!(&found.items, Expression::Reference(reference) if reference.name == "rows"));
    assert_eq!(found.binding, "row");
    assert_eq!(
        &source[found.binding_span.start_byte..found.binding_span.end_byte],
        "row"
    );
    assert!(matches!(found.key, Some(Expression::MemberAccess(_))));
    assert_eq!(found.children.len(), 1);
}

#[test]
fn a_repeat_without_a_key_still_parses_and_the_lowering_asks_for_it() {
    let Block::Each(found) = block("<a>{#each rows as row}<b />{/each}</a>") else {
        panic!("a repeat")
    };

    assert!(found.key.is_none());
}

#[test]
fn the_word_between_the_list_and_the_name_must_be_as() {
    let result = parse("<a>{#each rows of row (row.id)}<b />{/each}</a>");

    assert!(result.ast.is_none());
    assert_eq!(result.errors.len(), 1);
    assert!(
        result.errors[0].message.contains("expected `as`"),
        "{}",
        result.errors[0].message
    );
}

#[test]
fn blocks_nest_and_hold_text_and_expressions() {
    let source = "<a>{#each rows as row (row.id)}{#if row.on}hello {row.name}{/if}{/each}</a>";
    let Block::Each(outer) = block(source) else {
        panic!("a repeat")
    };

    assert!(outer
        .children
        .iter()
        .any(|child| matches!(child, Child::Block(inner) if matches!(**inner, Block::If(_)))));
}
