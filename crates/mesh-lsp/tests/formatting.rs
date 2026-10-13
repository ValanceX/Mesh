//! Format Document, Format Selection and indentation after Enter, over the protocol.

mod common;

use common::{example, Workspace};
use lsp_types::TextEdit;
use mesh_lsp::testing::{test_options, Client};
use serde_json::{json, Value};

fn server() -> (Client, String) {
    let workspace = Workspace::empty();
    workspace.write("components.json", &example("components.json"));
    let settings =
        json!({ "model": "components.json", "components": { "page.mprx": "users-page" } });
    let (client, _) = Client::initialized_with(test_options(), workspace.init(settings, json!({})));
    let uri = workspace.uri("page.mprx");
    // The workspace lives as long as the server needs its files.
    std::mem::forget(workspace);

    (client, uri)
}

fn edits(value: Value) -> Vec<TextEdit> {
    serde_json::from_value(value).expect("text edits")
}

/// Applies `edits` (LSP ranges, ascii text) to `source`.
fn apply(source: &str, edits: &[TextEdit]) -> String {
    let lines: Vec<usize> = std::iter::once(0)
        .chain(source.match_indices('\n').map(|(at, _)| at + 1))
        .collect();
    let offset = |line: u32, character: u32| lines[line as usize] + character as usize;
    let mut out = source.to_string();
    let mut sorted: Vec<&TextEdit> = edits.iter().collect();
    sorted.sort_by_key(|edit| (edit.range.start.line, edit.range.start.character));
    for edit in sorted.into_iter().rev() {
        let (start, end) = (
            offset(edit.range.start.line, edit.range.start.character),
            offset(edit.range.end.line, edit.range.end.character),
        );
        out.replace_range(start..end, &edit.new_text);
    }
    out
}

const MESSY: &str = "<page title=\"Users\">\n{#each [user] as person (person.name)}\n      <text\n   size=\"1\"\n  >{person.name}</text>\n   {/each}\n</page>\n";
const TIDY: &str = "<page title=\"Users\">\n    {#each [user] as person (person.name)}\n        <text\n            size=\"1\"\n        >{person.name}</text>\n    {/each}\n</page>\n";

fn options() -> Value {
    json!({ "tabSize": 4, "insertSpaces": true })
}

#[test]
fn format_document_indents_and_lays_out_tags() {
    let (mut client, uri) = server();
    client.open(&uri, 1, MESSY);
    client.next_publication(&uri);
    let response = client.request(
        "textDocument/formatting",
        json!({ "textDocument": { "uri": uri }, "options": options() }),
    );
    let formatted = apply(MESSY, &edits(response.response_result.unwrap()));
    assert_eq!(formatted, TIDY);
}

#[test]
fn format_document_of_a_broken_document_changes_nothing() {
    let (mut client, uri) = server();
    let broken = "<page>\n<a\n</page>\n";
    client.open(&uri, 1, broken);
    client.next_publication(&uri);
    let response = client.request(
        "textDocument/formatting",
        json!({ "textDocument": { "uri": uri }, "options": options() }),
    );
    assert_eq!(response.response_result.unwrap(), Value::Null);
}

#[test]
fn format_selection_touches_only_the_selected_lines() {
    let (mut client, uri) = server();
    client.open(&uri, 1, MESSY);
    client.next_publication(&uri);
    // Lines 1 to 2: the `{#each` line and the line of `<text`.
    let response = client.request(
        "textDocument/rangeFormatting",
        json!({
            "textDocument": { "uri": uri },
            "range": { "start": { "line": 1, "character": 0 }, "end": { "line": 2, "character": 3 } },
            "options": options()
        }),
    );
    let formatted = apply(MESSY, &edits(response.response_result.unwrap()));
    assert!(
        formatted.starts_with(
            "<page title=\"Users\">\n    {#each [user] as person (person.name)}\n        <text\n"
        ),
        "{formatted}"
    );
    // The closing `{/each}` was outside the selection.
    assert!(formatted.contains("\n   {/each}\n"), "{formatted}");
}

#[test]
fn enter_indents_the_new_line_by_scope() {
    let (mut client, uri) = server();
    let text =
        "<page title=\"Users\">\n{#each [user] as person (person.name)}\n\n{/each}\n</page>\n";
    client.open(&uri, 1, text);
    client.next_publication(&uri);
    // The cursor is on the empty line (line 2) after pressing Enter.
    let response = client.request(
        "textDocument/onTypeFormatting",
        json!({
            "textDocument": { "uri": uri },
            "position": { "line": 2, "character": 0 },
            "ch": "\n",
            "options": options()
        }),
    );
    let after = apply(text, &edits(response.response_result.unwrap()));
    assert_eq!(
        after,
        "<page title=\"Users\">\n{#each [user] as person (person.name)}\n        \n{/each}\n</page>\n"
    );
}
