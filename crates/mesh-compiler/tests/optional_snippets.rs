//! Optional snippets (decision U19): a snippet prop or scope name that may be absent is rendered only where
//! `has(name)` says it is there. The guard is the first branch of an `{#if}` (or `{:else if}`) whose condition
//! has `has(name)`, alone or `&&`-joined, however deep inside it, and not a snippet's body or an `{:else}`.

use mesh_compiler::{compile_with, CompileOptions};

const MODEL: &str = r#"{
  "version": 1, "types": {},
  "components": {
    "page": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "text": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "view": { "props": {}, "events": {}, "commands": {},
      "scope": {
        "flag": { "kind": "boolean" },
        "footer": { "kind": "optional", "type": { "kind": "snippet", "params": [] } },
        "row": { "kind": "optional", "type": { "kind": "snippet", "params": [ { "kind": "number" } ] } },
        "children": { "kind": "snippet", "params": [] },
        "rows": { "kind": "list", "element": { "kind": "number" } } } }
  }
}"#;

/// The (code, line:column) of each diagnostic of `source`, as the template of `view`.
fn found(source: &str) -> Vec<(String, String)> {
    let manifest = mesh_manifest::load(MODEL).unwrap();
    let result = compile_with(
        source,
        &CompileOptions::with_template(manifest.template("view").unwrap()),
    );

    result
        .diagnostics
        .iter()
        .map(|d| {
            let before = &source[..d.span.start_byte];
            let line = before.matches('\n').count() + 1;
            let column = before.len() - before.rfind('\n').map_or(0, |at| at + 1) + 1;

            (d.code.as_str().to_string(), format!("{line}:{column}"))
        })
        .collect()
}

fn absent(source: &str) -> Vec<String> {
    found(source)
        .into_iter()
        .filter(|(code, _)| code == "possibly-absent-snippet")
        .map(|(_, at)| at)
        .collect()
}

#[test]
fn an_unguarded_render_of_an_optional_snippet_is_possibly_absent() {
    assert_eq!(absent("<page>{@render footer()}</page>"), ["1:16"]);
    assert_eq!(
        absent("<page>\n  {@render footer()}\n  {@render row(1)}\n</page>"),
        ["2:12", "3:12"]
    );
}

#[test]
fn a_required_snippet_needs_no_guard() {
    assert!(found("<page>{@render children()}</page>").is_empty());
}

#[test]
fn the_first_branch_of_an_if_with_has_in_its_condition_is_guarded() {
    for source in [
        "<page>{#if has(footer)}{@render footer()}{/if}</page>",
        "<page>{#if has(footer) && flag}{@render footer()}{/if}</page>",
        "<page>{#if flag && has(footer)}{@render footer()}{/if}</page>",
        "<page>{#if has(footer)}<text>x</text>{@render footer()}{/if}</page>",
        "<page>{#if has(footer)}<page>{#each rows as r (r)}{@render footer()}{/each}</page>{/if}</page>",
        "<page>{#if has(footer)}{@render footer()}{:else}<text>none</text>{/if}</page>",
        "<page>{#if flag}<text>a</text>{:else if has(footer)}{@render footer()}{/if}</page>",
        "<page>{#if has(footer)}{#if flag}{@render footer()}{/if}{/if}</page>",
    ] {
        assert_eq!(absent(source), Vec::<String>::new(), "{source}");
    }
}

#[test]
fn nothing_else_vouches_for_it() {
    for source in [
        // The other branch, a different name, an `||`, a negation, a different test.
        "<page>{#if has(footer)}<text>x</text>{:else}{@render footer()}{/if}</page>",
        "<page>{#if has(row)}{@render footer()}{/if}</page>",
        "<page>{#if has(footer) || flag}{@render footer()}{/if}</page>",
        "<page>{#if !has(footer)}{@render footer()}{/if}</page>",
        "<page>{#if flag}{@render footer()}{/if}</page>",
        // Later siblings of the guard are outside it.
        "<page>{#if has(footer)}<text>x</text>{/if}{@render footer()}</page>",
    ] {
        // The one render of `footer`: its name is what is reported.
        let name = source.find("{@render footer").unwrap() + "{@render ".len();

        assert_eq!(absent(source), [format!("1:{}", name + 1)], "{source}");
    }
}

#[test]
fn a_snippet_declared_in_a_guard_is_rendered_where_its_holder_chooses_so_it_is_not_guarded() {
    assert_eq!(
        absent("<page>{#if has(footer)}{#snippet inner()}{@render footer()}{/snippet}{@render inner()}{/if}</page>"),
        ["1:51"]
    );
}

#[test]
fn passing_an_optional_snippet_along_needs_no_guard() {
    // Passed by name to a prop that is optional too; it is only rendering that needs the guard.
    let model = MODEL.replace(
        r#""view":"#,
        r#""shelf": { "props": { "footer": { "type": { "kind": "optional", "type": { "kind": "snippet", "params": [] } }, "required": false } }, "events": {}, "commands": {}, "scope": {} },
    "view":"#,
    );
    let manifest = mesh_manifest::load(&model).unwrap();
    let result = compile_with(
        "<page><shelf footer={footer} /></page>",
        &CompileOptions::with_template(manifest.template("view").unwrap()),
    );

    assert_eq!(
        result
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>(),
        Vec::<&str>::new()
    );
}

#[test]
fn the_message_names_the_snippet_and_says_what_to_write() {
    let manifest = mesh_manifest::load(MODEL).unwrap();
    let result = compile_with(
        "<page>{@render footer()}</page>",
        &CompileOptions::with_template(manifest.template("view").unwrap()),
    );
    let message = &result.diagnostics[0].message;

    assert!(message.contains(r#""footer" may be absent"#), "{message}");
    assert!(message.contains("{#if has(footer)}"), "{message}");
}
