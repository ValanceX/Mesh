//! Snippets in the editor (Bet 5): go to the declaration of a snippet's name, and signature help in a
//! render tag's arguments.

mod common;

use common::Workspace;
use mesh_compiler::{ColumnUnit, SourceMap};
use mesh_lsp::testing::{test_options, Client};
use serde_json::{json, Value};

const MANIFEST: &str = r#"{
  "version": 1,
  "types": { "Person": { "kind": "record", "fields": { "name": { "type": { "kind": "string" }, "required": true } } } },
  "components": {
    "page": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "text": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "data-list": { "props": {
        "items": { "type": { "kind": "list", "element": { "kind": "named", "name": "Person" } }, "required": true },
        "row": { "type": { "kind": "snippet", "params": [ { "kind": "named", "name": "Person" } ] }, "required": true } },
      "events": {}, "commands": {}, "scope": {} },
    "users-page": { "props": {}, "events": {}, "commands": {}, "scope": {
        "people": { "kind": "list", "element": { "kind": "named", "name": "Person" } },
        "row": { "kind": "snippet", "params": [ { "kind": "named", "name": "Person" }, { "kind": "number" } ] },
        "footer": { "kind": "optional", "type": { "kind": "snippet", "params": [] } } } }
  }
}"#;

fn server() -> (Workspace, Client, String) {
    let workspace = Workspace::empty();
    workspace.write("components.json", MANIFEST);
    let settings =
        json!({ "model": "components.json", "components": { "page.mprx": "users-page" } });
    let (client, _) = Client::initialized_with(test_options(), workspace.init(settings, json!({})));
    let uri = workspace.uri("page.mprx");

    (workspace, client, uri)
}

fn position(source: &str, offset: usize) -> (u32, u32) {
    let at = SourceMap::new(source).line_column(source, offset, ColumnUnit::Utf16);

    (at.line as u32, at.column as u32)
}

/// Opens `marked` without its `|` and asks `ask` at the `|`.
fn at_cursor(
    client: &mut Client,
    uri: &str,
    marked: &str,
    ask: impl Fn(&mut Client, u32, u32) -> Value,
) -> (String, Value) {
    let offset = marked.find('|').expect("a cursor");
    let source = marked.replacen('|', "", 1);

    client.open(uri, 1, &source);
    client.next_publication(uri);
    let (line, character) = position(&source, offset);
    let answer = ask(client, line, character);

    (source, answer)
}

fn signature(client: &mut Client, uri: &str, marked: &str) -> Value {
    let uri = uri.to_string();

    at_cursor(
        client,
        &uri.clone(),
        marked,
        move |client, line, character| client.signature_help(&uri, line, character),
    )
    .1
}

fn labels(help: &Value) -> Vec<String> {
    help["signatures"][0]["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|parameter| parameter["label"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn a_declared_snippet_shows_its_parameters_with_the_types_its_renders_give_them() {
    let (_workspace, mut client, uri) = server();
    let help = signature(
        &mut client,
        &uri,
        r#"<page>{#snippet pair(a, b)}<text>{a}{b}</text>{/snippet}{@render pair("x", |1)}</page>"#,
    );

    assert_eq!(help["signatures"][0]["label"], "pair(a: string, b: number)");
    assert_eq!(labels(&help), ["a: string", "b: number"]);
    assert_eq!(help["activeParameter"], 1);
    assert_eq!(help["activeSignature"], 0);
}

#[test]
fn the_active_parameter_follows_the_commas_and_ignores_the_commas_of_nested_values() {
    let (_workspace, mut client, uri) = server();
    let source = |before: &str| format!("<page>{{@render row({before}|)}}</page>");

    assert_eq!(
        signature(&mut client, &uri, &source(""))["activeParameter"],
        0
    );
    assert_eq!(
        signature(&mut client, &uri, &source("people[0], "))["activeParameter"],
        1
    );
    // Commas inside a list or a call are the argument's own.
    assert_eq!(
        signature(&mut client, &uri, &source("[1, 2], "))["activeParameter"],
        1
    );
    assert_eq!(
        signature(&mut client, &uri, &source("[1, 2]"))["activeParameter"],
        0
    );
    // More arguments than parameters: nothing is highlighted, the label stays.
    let past = signature(&mut client, &uri, &source("a, b, "));
    assert_eq!(past["activeParameter"], 2);
    assert_eq!(labels(&past).len(), 2);
}

#[test]
fn a_snippet_of_the_scope_shows_the_types_of_its_parameters_and_an_optional_one_says_so() {
    let (_workspace, mut client, uri) = server();
    let row = signature(&mut client, &uri, "<page>{@render row(|)}</page>");

    assert_eq!(row["signatures"][0]["label"], "row(Person, number)");
    assert_eq!(labels(&row), ["Person", "number"]);
    assert_eq!(row["signatures"][0]["documentation"], Value::Null);

    let footer = signature(&mut client, &uri, "<page>{@render footer(|)}</page>");
    assert_eq!(footer["signatures"][0]["label"], "footer()");
    assert!(footer["signatures"][0]["documentation"]
        .as_str()
        .unwrap()
        .contains("{#if has(footer)}"));
}

#[test]
fn a_call_that_is_being_typed_still_has_a_signature() {
    let (_workspace, mut client, uri) = server();
    // The call and the tag are open; the editor has closed neither.
    let help = signature(&mut client, &uri, "<page>{@render row(people, |");
    assert_eq!(help["signatures"][0]["label"], "row(Person, number)");
    assert_eq!(help["activeParameter"], 1);
}

#[test]
fn no_signature_outside_a_render_tags_own_arguments() {
    let (_workspace, mut client, uri) = server();
    for marked in [
        "<page><text>|</text></page>",
        "<page>{@render row|(people, 1)}</page>",
        "<page>{@render row(has(|), 1)}</page>",
        "<page>{@render row([|], 1)}</page>",
        "<page>{@render nothing(|)}</page>",
        "<page>{@render people(|)}</page>",
    ] {
        assert_eq!(
            signature(&mut client, &uri, marked),
            Value::Null,
            "{marked}"
        );
    }
}

fn definition(client: &mut Client, uri: &str, marked: &str) -> (String, Value) {
    let uri = uri.to_string();

    at_cursor(
        client,
        &uri.clone(),
        marked,
        move |client, line, character| client.definition(&uri, line, character),
    )
}

fn span_of(source: &str, range: &Value) -> String {
    let map = SourceMap::new(source);
    let offset = |at: &Value| {
        map.offset(
            source,
            mesh_compiler::LineColumn {
                line: at["line"].as_u64().unwrap() as usize,
                column: at["character"].as_u64().unwrap() as usize,
            },
            ColumnUnit::Utf16,
        )
    };

    source[offset(&range["start"])..offset(&range["end"])].to_string()
}

#[test]
fn the_name_in_a_render_goes_to_the_snippet_declared_in_the_template() {
    let (_workspace, mut client, uri) = server();
    let (source, found) = definition(
        &mut client,
        &uri,
        "<page>{#snippet pair(a)}<text>{a}</text>{/snippet}{@render pa|ir(1)}</page>",
    );

    assert_eq!(found["uri"], json!(uri));
    assert_eq!(span_of(&source, &found["range"]), "pair");
    assert!(
        found["range"]["start"]["character"].as_u64().unwrap() < 30,
        "the declaration, not the render"
    );
}

#[test]
fn a_local_snippet_passed_along_as_a_value_goes_to_its_declaration() {
    let (_workspace, mut client, uri) = server();
    let (source, found) = definition(
        &mut client,
        &uri,
        "<page>{#snippet fmt(p)}<text>{p.name}</text>{/snippet}<data-list items={people} row={fm|t} /></page>",
    );

    assert_eq!(span_of(&source, &found["range"]), "fmt");
    assert!(found["range"]["start"]["character"].as_u64().unwrap() < 20);
}

#[test]
fn a_snippet_of_the_scope_goes_to_the_manifest_and_a_snippet_given_to_a_component_to_its_prop() {
    let (workspace, mut client, uri) = server();
    let manifest = MANIFEST;
    let (_, scope) = definition(
        &mut client,
        &uri,
        "<page>{@render ro|w(people, 1)}</page>",
    );
    assert_eq!(scope["uri"], json!(workspace.uri("components.json")));
    let line = scope["range"]["start"]["line"].as_u64().unwrap() as usize;
    assert!(
        manifest.lines().nth(line).unwrap().contains("\"row\""),
        "line {line}"
    );

    let (_, passed) = definition(
        &mut client,
        &uri,
        "<page><data-list items={people}>{#snippet ro|w(p)}<text>{p.name}</text>{/snippet}</data-list></page>",
    );
    assert_eq!(passed["uri"], json!(workspace.uri("components.json")));
    let line = passed["range"]["start"]["line"].as_u64().unwrap() as usize;
    assert!(
        manifest.lines().nth(line).unwrap().contains("\"row\""),
        "line {line}"
    );
}
