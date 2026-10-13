//! Lowering blocks: a block becomes the construct it stands for, and a branch or body is one element.

use mesh_semantic::{lower, Child, Construct, Element};

fn lowered(source: &str) -> (Element, Vec<String>) {
    let ast = mesh_parser::parse(source).ast.expect("parses");
    let result = lower(&ast);

    (
        result.ir.expect("lowers"),
        result
            .diagnostics
            .iter()
            .map(|d| d.code.as_str().to_string())
            .collect(),
    )
}

fn only(element: &Element) -> &Element {
    match element.children.as_slice() {
        [Child::Element(child)] => child,
        other => panic!("one element child, found {other:?}"),
    }
}

#[test]
fn an_if_with_no_other_branch_is_the_conditional() {
    let (root, diagnostics) = lowered("<a>{#if x}<b />{/if}</a>");
    let conditional = only(&root);

    assert!(diagnostics.is_empty());
    assert_eq!(
        (conditional.name.as_str(), conditional.construct),
        ("mesh-if", Some(Construct::If))
    );
    assert_eq!(conditional.attributes[0].name, "when");
    assert_eq!(
        only(conditional).name,
        "b",
        "exactly one element is that element"
    );
    assert_eq!(only(conditional).construct, None);
}

#[test]
fn a_chain_is_a_choice_with_a_case_for_each_branch_and_a_default() {
    let (root, _) = lowered("<a>{#if x}<b />{:else if y}<c />{:else}<d />{/if}</a>");
    let choice = only(&root);

    assert_eq!(
        (choice.name.as_str(), choice.construct),
        ("mesh-switch", Some(Construct::Switch))
    );
    let kinds: Vec<_> = choice
        .children
        .iter()
        .map(|child| match child {
            Child::Element(element) => (element.name.clone(), element.construct),
            other => panic!("{other:?}"),
        })
        .collect();

    assert_eq!(
        kinds,
        [
            (
                "mesh-case".to_string(),
                Some(Construct::Case { first: true })
            ),
            (
                "mesh-case".to_string(),
                Some(Construct::Case { first: false })
            ),
            ("mesh-default".to_string(), Some(Construct::Default)),
        ]
    );
}

#[test]
fn anything_but_exactly_one_element_is_wrapped_in_a_fragment() {
    for source in [
        "<a>{#if x}text{/if}</a>",
        "<a>{#if x}<b /><c />{/if}</a>",
        "<a>{#if x}{#if y}<b />{/if}{/if}</a>",
        "<a>{#if x}<mesh-slot />{/if}</a>",
        "<a>{#if x}{/if}</a>",
        "<a>{#if x}{y}{/if}</a>",
    ] {
        let (root, diagnostics) = lowered(source);
        let body = only(only(&root));

        assert!(diagnostics.is_empty(), "{source}: {diagnostics:?}");
        assert_eq!(
            (body.name.as_str(), body.construct),
            ("mesh-fragment", Some(Construct::Fragment)),
            "{source}"
        );
    }
}

#[test]
fn whitespace_around_the_one_element_is_not_content() {
    let (root, _) = lowered("<a>{#if x}\n  <b />\n{/if}</a>");

    assert_eq!(only(only(&root)).name, "b");
}

#[test]
fn a_repeat_has_its_list_its_name_and_its_key_as_attributes() {
    let (root, diagnostics) = lowered("<a>{#each rows as row (row.id)}<b />{/each}</a>");
    let repeat = only(&root);
    let names: Vec<&str> = repeat
        .attributes
        .iter()
        .map(|attribute| attribute.name.as_str())
        .collect();

    assert!(diagnostics.is_empty());
    assert_eq!(
        (repeat.name.as_str(), repeat.construct),
        ("mesh-each", Some(Construct::Each))
    );
    assert_eq!(names, ["items", "as", "key"]);
}

#[test]
fn a_repeat_without_a_key_is_reported_while_lowering() {
    let (_, diagnostics) = lowered("<a>{#each rows as row}<b />{/each}</a>");

    assert_eq!(diagnostics, ["missing-repeat-key"]);
}

#[test]
fn a_reserved_element_written_as_an_element_is_reported() {
    for (source, tag) in [
        ("<a><mesh-if when={x}><b /></mesh-if></a>", "mesh-if"),
        (
            "<a><mesh-each items={x} as=\"y\" key={y}><b /></mesh-each></a>",
            "mesh-each",
        ),
        (
            "<a><mesh-switch><mesh-default><b /></mesh-default></mesh-switch></a>",
            "mesh-switch",
        ),
    ] {
        let (root, diagnostics) = lowered(source);

        assert!(
            diagnostics.contains(&"retired-element".to_string()),
            "{tag}: {diagnostics:?}"
        );
        assert_eq!(
            only(&root).construct,
            None,
            "written, not lowered from a block"
        );
    }
}
