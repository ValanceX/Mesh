//! Blocks in an editor: hover and go-to-definition on the name a repeat binds, and nothing offered for a block as a component.

mod common;

use common::{example, Workspace};
use mesh_compiler::{ColumnUnit, SourceMap};
use mesh_lsp::testing::{test_options, Client};
use serde_json::{json, Value};

const MANIFEST_SCOPE: &str = "users-page";

fn server() -> (Workspace, Client, String) {
    let workspace = Workspace::empty();
    workspace.write("components.json", &example("components.json"));
    let settings =
        json!({ "model": "components.json", "components": { "page.mprx": MANIFEST_SCOPE } });
    let (client, _) = Client::initialized_with(test_options(), workspace.init(settings, json!({})));
    let uri = workspace.uri("page.mprx");

    (workspace, client, uri)
}

fn position(source: &str, offset: usize) -> (u32, u32) {
    let at = SourceMap::new(source).line_column(source, offset, ColumnUnit::Utf16);

    (at.line as u32, at.column as u32)
}

const SOURCE: &str = "<page title=\"Users\">\n  {#each [user] as person (person.name)}\n    <text>{person.name}</text>\n  {/each}\n</page>\n";

fn hover_text(hover: &Value) -> String {
    hover["contents"]["value"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

#[test]
fn hovering_a_repeat_binding_shows_its_type_and_where_it_comes_from() {
    let (_workspace, mut client, uri) = server();
    client.open(&uri, 1, SOURCE);
    client.next_publication(&uri);

    // In the body, and at the declaration.
    for needle in ["person.name}</text>", "person (person"] {
        let offset = SOURCE.find(needle).unwrap() + 1;
        let (line, character) = position(SOURCE, offset);
        let text = hover_text(&client.hover(&uri, line, character));

        assert!(text.contains("person: User"), "{needle}: {text}");
        assert!(text.contains("Bound by `{#each}`"), "{needle}: {text}");
    }
}

#[test]
fn definition_of_a_repeat_binding_is_the_name_in_the_block_that_binds_it() {
    let (_workspace, mut client, uri) = server();
    client.open(&uri, 1, SOURCE);
    client.next_publication(&uri);

    let use_at = SOURCE.find("person.name}</text>").unwrap() + 1;
    let (line, character) = position(SOURCE, use_at);
    let location = client.definition(&uri, line, character);
    let declared = SOURCE.find("person (person").unwrap();
    let (declared_line, declared_character) = position(SOURCE, declared);

    assert_eq!(location["uri"], json!(uri));
    assert_eq!(
        location["range"]["start"],
        json!({ "line": declared_line, "character": declared_character })
    );
    assert_eq!(
        location["range"]["end"],
        json!({ "line": declared_line, "character": declared_character + "person".len() as u32 })
    );
}

#[test]
fn the_keyword_and_the_block_resolve_to_nothing() {
    let (_workspace, mut client, uri) = server();
    client.open(&uri, 1, SOURCE);
    client.next_publication(&uri);

    for needle in ["#each", " as "] {
        let offset = SOURCE.find(needle).unwrap() + 2;
        let (line, character) = position(SOURCE, offset);

        assert_eq!(client.hover(&uri, line, character), Value::Null, "{needle}");
        assert_eq!(
            client.definition(&uri, line, character),
            Value::Null,
            "{needle}"
        );
    }
}
