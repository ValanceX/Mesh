//! Blocks (`{#if}`, `{:else if}`, `{:else}`, `{#each}`): control flow is syntax, so a manifest declares nothing for it, the reserved elements it replaces are errors that
//! name the block to write, and each mistake in a block names the block.

use mesh_compiler::{compile_with, CompileOptions};

// No `mesh-if`, `mesh-each`, `mesh-switch`, `mesh-case`, `mesh-default` or `mesh-fragment`: blocks need none of them declared.
const MODEL: &str = r#"{
  "version": 1, "types": {},
  "components": {
    "panel": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "note":  { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "mesh-slot": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "view": { "props": {}, "events": {}, "commands": {},
      "scope": { "flag": { "kind": "boolean" }, "count": { "kind": "number" },
                 "rows": { "kind": "list", "element": { "kind": "record", "fields": {
                   "id": { "type": { "kind": "number" }, "required": true },
                   "name": { "type": { "kind": "string" }, "required": true } } } } } }
  }
}"#;

fn diagnostics(source: &str) -> Vec<(String, String)> {
    let manifest = mesh_manifest::load(MODEL).unwrap();
    let result = compile_with(
        source,
        &CompileOptions::with_template(manifest.template("view").unwrap()),
    );

    result
        .diagnostics
        .iter()
        .map(|d| (d.code.as_str().to_string(), d.message.clone()))
        .collect()
}

fn codes(source: &str) -> Vec<String> {
    diagnostics(source)
        .into_iter()
        .map(|(code, _)| code)
        .collect()
}

#[test]
fn well_formed_blocks_are_clean_and_need_no_declaration() {
    for source in [
        "<panel>{#if flag}<note>a</note>{/if}</panel>",
        "<panel>{#if flag}<note>a</note>{:else}<note>b</note>{/if}</panel>",
        "<panel>{#if flag}<note>a</note>{:else if count > 1}<note>b</note>{:else if count > 0}<note>c</note>{:else}<note>d</note>{/if}</panel>",
        "<panel>{#each rows as row (row.id)}<note>{row.name}</note>{/each}</panel>",
        // A branch or body that is not exactly one element is wrapped in a fragment, a lone block or slot included.
        "<panel>{#if flag}text {count}<note>a</note>{/if}</panel>",
        "<panel>{#if flag}{#if !flag}<note>a</note>{/if}{/if}</panel>",
        "<panel>{#if flag}<mesh-slot />{:else}<note>b</note>{/if}</panel>",
        "<panel>{#each rows as row (row.id)}{#if flag}<note>{row.name}</note>{/if}{/each}</panel>",
        "<panel>{#if flag}{/if}</panel>",
    ] {
        assert_eq!(codes(source), Vec::<String>::new(), "{source}");
    }
}

#[test]
fn a_manifest_that_declares_the_constructs_is_used_as_declared() {
    let model = MODEL.replace(
        "\"view\"",
        "\"mesh-if\": { \"props\": { \"when\": { \"type\": { \"kind\": \"boolean\" }, \"required\": true } }, \"events\": {}, \"commands\": {}, \"scope\": {} }, \"view\"",
    );
    let manifest = mesh_manifest::load(&model).unwrap();
    let result = compile_with(
        "<panel>{#if flag}<note>a</note>{/if}</panel>",
        &CompileOptions::with_template(manifest.template("view").unwrap()),
    );

    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
}

#[test]
fn the_reserved_elements_blocks_replaced_are_errors_that_name_the_block() {
    let retired = [
        ("<panel><mesh-if when={flag}><note>a</note></mesh-if></panel>", "mesh-if", "{#if condition}"),
        ("<panel><mesh-each items={rows} as=\"row\" key={row.id}><note>a</note></mesh-each></panel>", "mesh-each", "{#each items as item (key)}"),
        ("<panel><mesh-switch><mesh-case when={flag}><note>a</note></mesh-case></mesh-switch></panel>", "mesh-switch", "{:else if b}"),
        ("<panel><mesh-switch><mesh-case when={flag}><note>a</note></mesh-case><mesh-default><note>b</note></mesh-default></mesh-switch></panel>", "mesh-case", "{#if a}"),
        ("<panel><mesh-switch><mesh-case when={flag}><note>a</note></mesh-case><mesh-default><note>b</note></mesh-default></mesh-switch></panel>", "mesh-default", "{:else}"),
    ];

    for (source, tag, block) in retired {
        let found = diagnostics(source);
        let message = found
            .iter()
            .find(|(code, message)| {
                code == "retired-element" && message.contains(&format!("<{tag}>"))
            })
            .unwrap_or_else(|| panic!("{tag}: {found:?}"));

        assert!(message.1.contains(block), "{tag}: {}", message.1);
        // Reported once, as a retired element: not also as an unknown component, nor a malformed switch.
        assert!(
            !found
                .iter()
                .any(|(code, _)| code == "unknown-component" || code == "invalid-switch"),
            "{tag}: {found:?}"
        );
    }
}

#[test]
fn the_retired_multi_way_code_stays_in_the_catalogue_and_nothing_reports_it() {
    assert!(mesh_syntax::DiagnosticCode::ALL.contains(&mesh_syntax::DiagnosticCode::INVALID_SWITCH));
    for source in [
        "<panel>{#if flag}<note>a</note>{:else if !flag}<note>b</note>{:else}<note>c</note>{/if}</panel>",
        "<panel><mesh-switch></mesh-switch></panel>",
        "<panel><mesh-case when={flag}><note>a</note></mesh-case></panel>",
    ] {
        assert!(!codes(source).contains(&"invalid-switch".to_string()), "{source}");
    }
}

#[test]
fn a_mistake_in_a_block_names_the_block() {
    let wrong = [
        (
            "<panel>{#if count}<note>a</note>{/if}</panel>",
            "type-mismatch",
            "the condition of `{#if}`",
        ),
        (
            "<panel>{#if flag}<note>a</note>{:else if count}<note>b</note>{/if}</panel>",
            "type-mismatch",
            "the condition of `{:else if}`",
        ),
        (
            "<panel>{#each count as row (row)}<note>a</note>{/each}</panel>",
            "type-mismatch",
            "the list of `{#each}`",
        ),
        (
            "<panel>{#each rows as row (flag)}<note>a</note>{/each}</panel>",
            "type-mismatch",
            "the key of `{#each}`",
        ),
        (
            "<panel>{#each rows as row (row.name2)}<note>a</note>{/each}</panel>",
            "unknown-member",
            "no member",
        ),
    ];

    for (source, code, words) in wrong {
        let found = diagnostics(source);

        assert!(
            found.iter().any(|(c, m)| c == code && m.contains(words)),
            "{source}: {found:?}"
        );
        assert!(
            !found.iter().any(|(_, m)| m.contains("mesh-if")
                || m.contains("mesh-each")
                || m.contains("mesh-case")),
            "{source}: {found:?}"
        );
    }
}

#[test]
fn a_repeat_without_a_key_says_a_key_is_required() {
    let found = diagnostics("<panel>{#each rows as row}<note>a</note>{/each}</panel>");

    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].0, "missing-repeat-key");
    assert!(found[0].1.contains("needs a key"), "{}", found[0].1);
}

#[test]
fn the_name_a_repeat_binds_is_readable_in_the_key_and_the_body_and_not_in_the_list() {
    assert_eq!(
        codes("<panel>{#each rows as row (row.id)}<note>{row.name}</note>{/each}</panel>"),
        Vec::<String>::new()
    );
    // Not in the list it iterates, and not after the block.
    assert!(
        codes("<panel>{#each row as row (row.id)}<note>a</note>{/each}</panel>")
            .contains(&"unknown-reference".to_string())
    );
    assert!(codes(
        "<panel>{#each rows as row (row.id)}<note>a</note>{/each}<note>{row.name}</note></panel>"
    )
    .contains(&"unknown-reference".to_string()));
    // The binding is not a read of state: the keyword and the name are not references.
    let manifest = mesh_manifest::load(MODEL).unwrap();
    let result = compile_with(
        "<panel>{#each rows as row (row.id)}<note>{row.name}</note>{/each}</panel>",
        &CompileOptions::with_template(manifest.template("view").unwrap()),
    );
    let analysis = result.analysis.unwrap();

    assert!(
        analysis.resolutions().iter().all(|resolution| !matches!(
            &resolution.target,
            mesh_analysis::Target::Component(name) if name.starts_with("mesh-")
        )),
        "a block resolves no component"
    );
    assert!(analysis
        .resolutions()
        .iter()
        .any(|resolution| matches!(&resolution.target, mesh_analysis::Target::Binding { name, .. } if name == "row")));
}

#[test]
fn a_block_counts_as_three_levels_toward_the_nesting_limit() {
    // 128 levels is the limit: the root element is one, each nested element one, each block three.
    let deep = |blocks: usize| {
        format!(
            "<panel>{}<note>a</note>{}</panel>",
            "{#if flag}".repeat(blocks),
            "{/if}".repeat(blocks)
        )
    };

    assert!(!codes(&deep(40)).contains(&"nesting-too-deep".to_string()));
    assert!(codes(&deep(43)).contains(&"nesting-too-deep".to_string()));
}

#[test]
fn a_block_cannot_be_the_document() {
    let found = diagnostics("{#if flag}<note>a</note>{/if}");

    assert!(
        found
            .iter()
            .any(|(code, message)| code == "syntax-error"
                && message.contains("cannot be the document")),
        "{found:?}"
    );
}

#[test]
fn syntax_errors_in_blocks_keep_the_code_and_say_what_is_wrong() {
    let wrong = [
        ("<panel>{#if flag}<note>a</note></panel>", "`{#if}` is never closed"),
        ("<panel>{#each rows as row (row.id)}<note>a</note></panel>", "`{#each}` is never closed"),
        ("<panel><note>a</note>{/if}</panel>", "`{/if}` with no open `{#if}`"),
        ("<panel><note>a</note>{/each}</panel>", "`{/each}` with no open `{#each}`"),
        ("<panel><note>a</note>{:else}</panel>", "`{:else}` with no open `{#if}`"),
        ("<panel><note>a</note>{:else if flag}</panel>", "`{:else if}` with no open `{#if}`"),
        ("<panel>{#each rows as row (row.id)}<note>a</note>{:else}{/each}</panel>", "`{:else}` with no open `{#if}`"),
        ("<panel>{#if}<note>a</note>{/if}</panel>", "malformed `{#if}`"),
        ("<panel>{#each rows}<note>a</note>{/each}</panel>", "malformed `{#each}`"),
        ("<panel>{#each rows as (row.id)}<note>a</note>{/each}</panel>", "malformed `{#each}`"),
        ("<panel>{#if flag}<note>a</note>{:else}<note>b</note>{:else if flag}<note>c</note>{/if}</panel>", "after `{:else}`"),
        ("<panel>{#if flag}<note>a</note>{/each}</panel>", "closes a block that is not open"),
        ("<panel>{#when flag}<note>a</note>{/when}</panel>", "unknown block"),
    ];

    for (source, words) in wrong {
        let found = diagnostics(source);

        assert!(
            found
                .iter()
                .any(|(code, message)| code == "syntax-error" && message.contains(words)),
            "{source}: {found:?}"
        );
    }
}

#[test]
fn text_cannot_contain_the_block_delimiters() {
    // To show a delimiter, write a string expression.
    assert_eq!(
        codes("<panel><note>{\"{#if}\"}</note></panel>"),
        Vec::<String>::new()
    );
    for source in [
        "<panel><note>a {#if b</note></panel>",
        "<panel><note>a {/if} b</note></panel>",
        "<panel><note>a {: b</note></panel>",
    ] {
        assert!(!codes(source).is_empty(), "{source}");
    }
}

#[test]
fn a_case_without_a_condition_is_a_malformed_branch() {
    assert!(
        codes("<panel>{#if flag}<note>a</note>{:else if}<note>b</note>{/if}</panel>")
            .contains(&"syntax-error".to_string())
    );
}
