//! Hover and definition on files with syntax errors, from editor recovery
//! state (outline D3), and what recovery must never do: publish anything,
//! or answer for a misparse.

mod common;

use common::{codes, example, Workspace};
use mesh_compiler::{ColumnUnit, CompileOptions, SourceMap};
use mesh_lsp::testing::Client;
use serde_json::{json, Value};

/// A server with `manifest`, where `page.mprx` is the template of
/// `users-page`; `source` is opened in it.
fn opened(manifest: &str, source: &str) -> (Workspace, Client, String) {
    let workspace = Workspace::empty();
    workspace.write("components.json", manifest);
    let settings = json!({
        "model": "components.json",
        "components": { "page.mprx": "users-page" }
    });
    let capabilities = json!({ "textDocument": { "hover": { "contentFormat": ["plaintext"] } } });
    let (mut client, _) = Client::initialized(workspace.init(settings, capabilities));
    let uri = workspace.uri("page.mprx");
    client.open(&uri, 1, source);
    // The published diagnostics are exactly the canonical ones: the syntax
    // errors, and nothing recovery found.
    let published = client.next_publication(&uri);
    let manifest = mesh_manifest::load(manifest).unwrap();
    let template = manifest.template("users-page").unwrap();
    let canonical = mesh_compiler::compile_with(source, &CompileOptions::with_template(template));
    assert!(canonical.ir.is_none(), "these tests are about broken files");
    let expected: Vec<String> = canonical
        .diagnostics
        .iter()
        .map(|d| d.code.as_str().to_string())
        .collect();
    assert_eq!(codes(&published.diagnostics), expected);
    (workspace, client, uri)
}

fn at(source: &str, needle: &str, nth: usize) -> (u32, u32) {
    let offset = source.find(needle).unwrap_or_else(|| panic!("{needle:?}")) + nth;
    let position = SourceMap::new(source).line_column(source, offset, ColumnUnit::Utf16);
    (position.line as u32, position.column as u32)
}

fn hover(client: &mut Client, uri: &str, source: &str, needle: &str, nth: usize) -> Value {
    let (line, character) = at(source, needle, nth);
    client.hover(uri, line, character)
}

fn value(hover: &Value) -> &str {
    hover["contents"]["value"].as_str().expect("a hover")
}

#[test]
fn hover_works_beside_a_syntax_error() {
    let source = example("users-page.mprx").replace("</page>", "  <text>{user.}</text>\n</page>");
    let (_workspace, mut client, uri) = opened(&example("components.json"), &source);
    let compact = hover(&mut client, &uri, &source, "compact ?", 1);
    assert!(value(&compact).starts_with("compact: boolean"));
    // The `user` before the stray dot is an expression part of its own.
    let user = hover(&mut client, &uri, &source, "user.}", 1);
    assert!(value(&user).starts_with("user: User"), "{user}");
}

#[test]
fn hover_works_before_an_unclosed_block() {
    // One unclosed `{` collapses the whole file into an ERROR root; the
    // avatar still survives, whole, as a part of the forest.
    let source = example("users-page.mprx").replace("</page>", "  <text>{user.\n</page>");
    let (_workspace, mut client, uri) = opened(&example("components.json"), &source);
    let compact = hover(&mut client, &uri, &source, "compact ?", 1);
    assert!(value(&compact).starts_with("compact: boolean"));
    let alt = hover(&mut client, &uri, &source, "alt=", 1);
    assert!(value(&alt).starts_with("alt: string"));
}

#[test]
fn definition_works_beside_a_syntax_error() {
    let source = example("users-page.mprx").replace("</page>", "  <text>{user.}</text>\n</page>");
    let manifest = example("components.json");
    let (workspace, mut client, uri) = opened(&manifest, &source);
    let (line, character) = at(&source, "<avatar", 2);
    let location = client.definition(&uri, line, character);
    assert_eq!(location["uri"], json!(workspace.uri("components.json")));
    let start = &location["range"]["start"];
    let offset = SourceMap::new(&manifest).offset(
        &manifest,
        mesh_compiler::LineColumn {
            line: start["line"].as_u64().unwrap() as usize,
            column: start["character"].as_u64().unwrap() as usize,
        },
        ColumnUnit::Utf16,
    );
    assert!(manifest[offset..].starts_with("\"avatar\""));
}

#[test]
fn misparses_are_not_answers() {
    // A manifest whose template scope has `page` and `text`, the names
    // Tree-sitter reads closing tags as after an unclosed block.
    let manifest = example("components.json").replace(
        r#""compact": { "kind": "boolean" }
      }"#,
        r#""compact": { "kind": "boolean" },
        "page": { "kind": "string" },
        "text": { "kind": "string" }
      }"#,
    );
    assert!(manifest.contains(r#""page": { "kind": "string" }"#));

    let s16 =
        "<page title=\"U\">\n  <avatar alt=\"x\" src={user.avatar} />\n  <text>{user.\n</page>";
    let (_workspace, mut client, uri) = opened(&manifest, s16);
    assert_eq!(hover(&mut client, &uri, s16, "</page", 3), Value::Null);
    let user = hover(&mut client, &uri, s16, "user.avatar", 1);
    assert!(
        value(&user).starts_with("user: User"),
        "the avatar still answers"
    );

    let s17 = "<page title=\"U\">\n  <avatar alt=\"x />\n  <text>{user.name}</text>\n</page>";
    let (_workspace, mut client, uri) = opened(&manifest, s17);
    assert_eq!(hover(&mut client, &uri, s17, "</text", 3), Value::Null);
    assert_eq!(hover(&mut client, &uri, s17, "</page", 3), Value::Null);
}

#[test]
fn no_recovery_without_a_model() {
    let workspace = Workspace::empty();
    workspace.write("components.json", &example("components.json"));
    let settings = json!({ "model": "components.json", "components": {} });
    let (mut client, _) = Client::initialized(workspace.init(settings, json!({})));
    let uri = workspace.uri("page.mprx");
    let source = example("users-page.mprx").replace("</page>", "  <text>{user.}</text>\n</page>");
    client.open(&uri, 1, &source);
    client.next_publication(&uri);
    let (line, character) = at(&source, "compact ?", 1);
    assert_eq!(client.hover(&uri, line, character), Value::Null);
}
