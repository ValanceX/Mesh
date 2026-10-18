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

#[test]
fn a_snippet_has_its_name_its_parameters_and_its_body() {
    let source = "<a>{#snippet row(p, q)}<b />text{/snippet}</a>";
    let Block::Snippet(found) = block(source) else {
        panic!("a snippet")
    };

    assert_eq!(found.name, "row");
    assert_eq!(
        found
            .parameters
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect::<Vec<_>>(),
        ["p", "q"]
    );
    assert_eq!(
        &source[found.parameters[1].span.start_byte..found.parameters[1].span.end_byte],
        "q"
    );
    assert_eq!(
        &source[found.keyword_span.start_byte..found.keyword_span.end_byte],
        "{#snippet"
    );
    assert_eq!(found.children.len(), 2);
    assert_eq!(
        &source[found.span.start_byte..found.span.end_byte],
        "{#snippet row(p, q)}<b />text{/snippet}"
    );
}

#[test]
fn a_snippet_may_have_no_parameters_and_no_body() {
    let Block::Snippet(found) = block("<a>{#snippet empty()}{/snippet}</a>") else {
        panic!("a snippet")
    };

    assert!(found.parameters.is_empty());
    assert!(found.children.is_empty());
}

#[test]
fn a_render_tag_has_the_snippet_name_and_its_arguments() {
    let source = "<a>{@render row(item, 1 + 2)}</a>";
    let Block::Render(found) = block(source) else {
        panic!("a render tag")
    };

    assert_eq!(found.name, "row");
    assert_eq!(
        &source[found.name_span.start_byte..found.name_span.end_byte],
        "row"
    );
    assert_eq!(found.arguments.len(), 2);
    assert!(matches!(&found.arguments[0], Expression::Reference(r) if r.name == "item"));
    assert_eq!(
        &source[found.span.start_byte..found.span.end_byte],
        "{@render row(item, 1 + 2)}"
    );
}

#[test]
fn snippets_nest_in_blocks_and_in_each_other() {
    let source = "<a>{#snippet outer(x)}{#snippet inner(y)}{@render inner(x)}{/snippet}{@render inner(x)}{/snippet}</a>";
    let Block::Snippet(outer) = block(source) else {
        panic!("a snippet")
    };

    assert!(outer
        .children
        .iter()
        .any(|child| matches!(child, Child::Block(inner) if matches!(**inner, Block::Snippet(_)))));
    assert!(outer
        .children
        .iter()
        .any(|child| matches!(child, Child::Block(inner) if matches!(**inner, Block::Render(_)))));
}

#[test]
fn a_snippet_that_is_never_closed_is_reported_where_it_opens() {
    let source = "<a>{#snippet row(p)}<b /></a>";
    let result = parse(source);

    assert!(result.ast.is_none());
    assert!(
        result
            .errors
            .iter()
            .any(|error| error.message.contains("`{#snippet}` is never closed")),
        "{:?}",
        result.errors
    );
}

#[test]
fn malformed_snippets_and_render_tags_say_what_to_write() {
    for (source, expected) in [
        (
            "<a>{#snippet row}<b />{/snippet}</a>",
            "write `{#snippet name(parameters)}`",
        ),
        ("<a>{#snippet (p)}<b />{/snippet}</a>", "it has no name"),
        (
            "<a>{#snippet row(1)}<b />{/snippet}</a>",
            "each parameter is a name",
        ),
        ("<a>{@render row}</a>", "write `{@render name(arguments)}`"),
        ("<a>{@show row()}</a>", "unknown tag `{@show`"),
        (
            "<a>{/snippet}</a>",
            "`{/snippet}` with no open `{#snippet}`",
        ),
    ] {
        let result = parse(source);

        assert!(result.ast.is_none(), "{source}");
        assert!(
            result
                .errors
                .iter()
                .any(|error| error.message.contains(expected)),
            "{source}: {:?}",
            result.errors
        );
    }
}
