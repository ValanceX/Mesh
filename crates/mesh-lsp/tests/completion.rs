//! Completion (outline D4): the Definition of Done's cases, and the
//! contract: deterministic, duplicate-free, stably ordered, and degrading
//! to nothing rather than to anything wrong.

mod common;

use common::{codes, example, Workspace};
use crossbeam_channel::unbounded;
use lsp_server::ErrorCode;
use mesh_compiler::{ColumnUnit, CompileOptions, SourceMap};
use mesh_lsp::testing::{position_params, test_options, Client};
use mesh_lsp::Options;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::time::Duration;

/// A server where `page.mprx` is the template of `users-page` in
/// `manifest`, unless `configured` is false.
fn server(manifest: &str, configured: bool, options: Options) -> (Workspace, Client, String) {
    let workspace = Workspace::empty();
    workspace.write("components.json", manifest);
    let components = if configured {
        json!({ "page.mprx": "users-page" })
    } else {
        json!({})
    };
    let settings = json!({ "model": "components.json", "components": components });
    let (client, _) = Client::initialized_with(options, workspace.init(settings, json!({})));
    let uri = workspace.uri("page.mprx");
    (workspace, client, uri)
}

fn position(source: &str, offset: usize) -> (u32, u32) {
    let at = SourceMap::new(source).line_column(source, offset, ColumnUnit::Utf16);
    (at.line as u32, at.column as u32)
}

/// Opens `marked` without its `|` and completes at the `|`.
fn complete_at(client: &mut Client, uri: &str, marked: &str) -> Value {
    let offset = marked.find('|').unwrap();
    let source = marked.replacen('|', "", 1);
    client.open(uri, 1, &source);
    client.next_publication(uri);
    let (line, character) = position(&source, offset);
    client.completion(uri, line, character)
}

fn labels(list: &Value) -> Vec<String> {
    items(list)
        .iter()
        .map(|item| item["label"].as_str().unwrap().to_string())
        .collect()
}

fn items(list: &Value) -> Vec<Value> {
    match list {
        Value::Array(items) => items.clone(),
        list => list["items"].as_array().cloned().unwrap_or_default(),
    }
}

fn page(inner: &str) -> String {
    format!("<page title=\"Users\">\n  {inner}\n</page>")
}

fn users_page() -> (Workspace, Client, String) {
    server(&example("components.json"), true, test_options())
}

// --- The Definition of Done ------------------------------------------

#[test]
fn inside_an_opening_tag_the_props_required_first() {
    let (_workspace, mut client, uri) = users_page();
    let list = complete_at(&mut client, &uri, &page("<avatar |"));
    assert_eq!(labels(&list), ["alt", "src", "size"]);
    let items = items(&list);
    let required: Vec<&Value> = items
        .iter()
        .map(|item| &item["labelDetails"]["description"])
        .collect();
    assert_eq!(
        required,
        [&json!("required"), &json!("required"), &Value::Null]
    );
    assert_eq!(items[0]["kind"], json!(10)); // Property
    assert_eq!(items[1]["detail"], json!("string?"));
}

#[test]
fn after_a_dot_the_members_although_it_is_a_syntax_error() {
    for marked in [
        page("<text>{user.|}</text>"),
        "<page title=\"Users\">\n  <text>{user.|\n</page>".to_string(),
    ] {
        let (_workspace, mut client, uri) = users_page();
        let source = marked.replacen('|', "", 1);
        let list = complete_at(&mut client, &uri, &marked);
        assert_eq!(labels(&list), ["active", "avatar", "name"], "{marked}");
        // The published diagnostics are the canonical syntax errors only.
        let manifest = mesh_manifest::load(&example("components.json")).unwrap();
        let template = manifest.template("users-page").unwrap();
        let canonical =
            mesh_compiler::compile_with(&source, &CompileOptions::with_template(template));
        let expected: Vec<String> = canonical
            .diagnostics
            .iter()
            .map(|d| d.code.as_str().to_string())
            .collect();
        client.change(&uri, 2, &source);
        assert_eq!(codes(&client.next_publication(&uri).diagnostics), expected);
    }
}

#[test]
fn after_a_less_than_every_component() {
    let (_workspace, mut client, uri) = users_page();
    let list = complete_at(&mut client, &uri, &page("<|"));
    let manifest = mesh_manifest::load(&example("components.json")).unwrap();
    let all: Vec<String> = manifest.components().keys().cloned().collect();
    assert_eq!(labels(&list), all);
}

#[test]
fn an_edit_replaces_the_whole_partial_name() {
    let (_workspace, mut client, uri) = users_page();
    let list = complete_at(&mut client, &uri, &page("<user-c|"));
    let item = items(&list)
        .into_iter()
        .find(|item| item["label"] == "user-card")
        .unwrap();
    let range = &item["textEdit"]["range"];
    // `user-c` on line 1, after two spaces and `<`.
    assert_eq!(range["start"], json!({ "line": 1, "character": 3 }));
    assert_eq!(range["end"], json!({ "line": 1, "character": 9 }));
    assert_eq!(item["textEdit"]["newText"], "user-card");
}

// --- The contract ------------------------------------------------------

#[test]
fn the_same_snapshot_gives_the_same_list_without_duplicates() {
    for (file, component) in [
        ("users-page.mprx", "users-page"),
        ("user-card.mprx", "user-card-example"),
    ] {
        let workspace = Workspace::empty();
        workspace.write("components.json", &example("components.json"));
        let settings = json!({ "model": "components.json", "components": { "t.mprx": component } });
        let (mut client, _) = Client::initialized(workspace.init(settings, json!({})));
        let uri = workspace.uri("t.mprx");
        let source = example(file);
        client.open(&uri, 1, &source);
        client.next_publication(&uri);
        for (offset, _) in source.char_indices() {
            let (line, character) = position(&source, offset);
            let first = client.completion(&uri, line, character);
            let second = client.completion(&uri, line, character);
            assert_eq!(first, second, "{file} at {offset}");
            let mut seen = BTreeSet::new();
            for item in items(&first) {
                let key = (item["label"].to_string(), item["kind"].to_string());
                assert!(seen.insert(key), "{file} at {offset}: a duplicate {item}");
            }
        }
    }
}

#[test]
fn everything_but_props_is_in_name_order() {
    let (_workspace, mut client, uri) = users_page();
    let list = complete_at(&mut client, &uri, &page("<button |"));
    assert_eq!(labels(&list), ["disabled", "on.click"]);
    let (_workspace, mut client, uri) = users_page();
    let list = complete_at(&mut client, &uri, &page("<text>{|}</text>"));
    assert_eq!(labels(&list), ["compact", "user"]);
    let (_workspace, mut client, uri) = users_page();
    let list = complete_at(&mut client, &uri, &page("<button on.click={|}>x</button>"));
    assert_eq!(labels(&list), ["selectUser"]);
}

#[test]
fn without_a_usable_model_nothing_is_offered() {
    // No component configured.
    let (_workspace, mut client, uri) = server(&example("components.json"), false, test_options());
    assert!(labels(&complete_at(&mut client, &uri, &page("<|"))).is_empty());
    // A manifest with errors.
    let (_workspace, mut client, uri) = server("{ \"version\": 1", true, test_options());
    assert!(labels(&complete_at(&mut client, &uri, &page("<|"))).is_empty());
    // An undeclared tag.
    let (_workspace, mut client, uri) = users_page();
    assert!(labels(&complete_at(&mut client, &uri, &page("<nope |"))).is_empty());
}

#[test]
fn no_members_without_a_type_or_through_an_optional() {
    let (_workspace, mut client, uri) = users_page();
    assert!(labels(&complete_at(
        &mut client,
        &uri,
        &page("<text>{usr.|}</text>")
    ))
    .is_empty());
    let (_workspace, mut client, uri) = users_page();
    assert!(labels(&complete_at(
        &mut client,
        &uri,
        &page("<text>{user.avatar.|}</text>")
    ))
    .is_empty());
}

#[test]
fn a_completion_after_a_change_is_for_the_new_text() {
    let options = Options {
        debounce: Duration::from_secs(30),
        ..test_options()
    };
    let (_workspace, mut client, uri) = server(&example("components.json"), true, options);
    let before = page("<text></text>");
    client.open(&uri, 1, &before);
    client.next_publication(&uri);
    let marked = page("<text>{user.|}</text>");
    client.change(&uri, 2, &marked.replacen('|', "", 1));
    let (line, character) = position(&marked, marked.find('|').unwrap());
    let list = client.completion(&uri, line, character);
    assert_eq!(labels(&list), ["active", "avatar", "name"]);
}

#[test]
fn a_newer_change_makes_it_content_modified() {
    let (release, gate) = unbounded();
    let options = Options {
        compile_gate: Some(gate),
        ..test_options()
    };
    let (_workspace, mut client, uri) = server(&example("components.json"), true, options);
    let marked = page("<text>{user.|}</text>");
    let source = marked.replacen('|', "", 1);
    client.open(&uri, 1, &source);
    let (line, character) = position(&marked, marked.find('|').unwrap());
    let id = client.send(
        "textDocument/completion",
        position_params(&uri, line, character),
    );
    client.change(&uri, 2, &source);
    let response = client
        .response(&id, Duration::from_secs(10))
        .expect("an answer");
    assert_eq!(
        response.response_result.unwrap_err().code,
        ErrorCode::ContentModified as i32
    );
    drop(release);
}

// --- Blocks ----------------------------------------------------------------

#[test]
fn after_a_block_opener_the_names_a_value_may_be() {
    for opener in [
        "{#if |",
        "{#if user.active}<text>a</text>{:else if |",
        "{#each |",
    ] {
        let (_workspace, mut client, uri) = users_page();
        let marked = page(opener);
        let list = complete_at(&mut client, &uri, &marked);

        assert_eq!(labels(&list), ["compact", "user"], "{opener}");
    }
}

#[test]
fn a_block_is_not_offered_as_a_tag() {
    // A model written before blocks may still declare the reserved elements; they are not components to write.
    let manifest = example("components.json").replace(
        "\"users-page\"",
        "\"mesh-if\": { \"props\": { \"when\": { \"type\": { \"kind\": \"boolean\" }, \"required\": true } }, \"events\": {}, \"commands\": {}, \"scope\": {} }, \"users-page\"",
    );
    let (_workspace, mut client, uri) = server(&manifest, true, test_options());
    let list = complete_at(&mut client, &uri, &page("<|"));

    assert!(
        !labels(&list)
            .iter()
            .any(|label| label.starts_with("mesh-if")),
        "{:?}",
        labels(&list)
    );
}

// --- Snippets and enums (Bet 5, Bet 4) -------------------------------------

/// A manifest where `users-page` has a closed set, an optional one, a snippet and a list, and `badge` and `data-list` take them.
const SNIPPET_MANIFEST: &str = r#"{
  "version": 1,
  "types": { "Person": { "kind": "record", "fields": { "name": { "type": { "kind": "string" }, "required": true } } } },
  "components": {
    "page": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "text": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "badge": { "props": {
        "tone": { "type": { "kind": "enum", "values": ["info", "danger", "warn"] }, "required": true },
        "shade": { "type": { "kind": "optional", "type": { "kind": "enum", "values": ["light", "dark"] } }, "required": false },
        "label": { "type": { "kind": "string" }, "required": false } }, "events": {}, "commands": {}, "scope": {} },
    "data-list": { "props": {
        "items": { "type": { "kind": "list", "element": { "kind": "named", "name": "Person" } }, "required": true },
        "row": { "type": { "kind": "snippet", "params": [ { "kind": "named", "name": "Person" } ] }, "required": true } },
      "events": {}, "commands": {}, "scope": {} },
    "users-page": { "props": {}, "events": {}, "commands": {}, "scope": {
        "tone": { "kind": "enum", "values": ["info", "danger"] },
        "title": { "kind": "string" },
        "people": { "kind": "list", "element": { "kind": "named", "name": "Person" } },
        "row": { "kind": "snippet", "params": [ { "kind": "named", "name": "Person" } ] },
        "footer": { "kind": "optional", "type": { "kind": "snippet", "params": [] } } } }
  }
}"#;

fn snippet_page() -> (Workspace, Client, String) {
    server(SNIPPET_MANIFEST, true, test_options())
}

fn kinds(list: &Value) -> Vec<(String, u64)> {
    items(list)
        .iter()
        .map(|item| {
            (
                item["label"].as_str().unwrap().to_string(),
                item["kind"].as_u64().unwrap(),
            )
        })
        .collect()
}

// CompletionItemKind: Function 3, Variable 6, EnumMember 20.
const FUNCTION: u64 = 3;
const VARIABLE: u64 = 6;
const ENUM_MEMBER: u64 = 20;

#[test]
fn after_a_render_opener_the_snippets_in_reach_local_ones_first_then_the_scopes() {
    let (_workspace, mut client, uri) = snippet_page();
    let list = complete_at(
        &mut client,
        &uri,
        "<page>{#snippet greet(name)}<text>{name}</text>{/snippet}{#snippet other()}<text>x</text>{/snippet}{@render |}</page>",
    );

    assert_eq!(
        kinds(&list),
        [
            ("greet".to_string(), FUNCTION),
            ("other".to_string(), FUNCTION),
            ("footer".to_string(), FUNCTION),
            ("row".to_string(), FUNCTION)
        ]
    );
    let details: Vec<String> = items(&list)
        .iter()
        .map(|item| item["detail"].as_str().unwrap_or("").to_string())
        .collect();
    assert_eq!(
        details,
        [
            "greet(name)",
            "other()",
            "snippet<>?, may be absent: render it inside {#if has(footer)}",
            "snippet<Person>"
        ]
    );
}

#[test]
fn a_snippet_is_in_reach_of_the_children_of_the_element_it_is_in_and_of_nothing_outside() {
    let (_workspace, mut client, uri) = snippet_page();
    // `inner` is declared inside `<text>`: not in reach of a render beside it.
    let beside = complete_at(
        &mut client,
        &uri,
        "<page><text>{#snippet inner()}<text>x</text>{/snippet}</text>{@render |}</page>",
    );
    assert_eq!(labels(&beside), ["footer", "row"]);
    // A snippet in the body of another is in reach of that body, and of nothing outside it.
    let inside = complete_at(&mut client, &uri, "<page>{#snippet outer()}{#snippet inner()}<text>x</text>{/snippet}{@render |}{/snippet}</page>");
    assert_eq!(labels(&inside), ["outer", "inner", "footer", "row"]);
}

#[test]
fn a_render_completes_a_partial_name_and_replaces_it() {
    let (_workspace, mut client, uri) = snippet_page();
    let marked = "<page>{#snippet greet()}<text>x</text>{/snippet}{@render gr|}</page>";
    let list = complete_at(&mut client, &uri, marked);
    let source = marked.replacen('|', "", 1);
    let start = source.find("gr}").unwrap();
    let (line, character) = position(&source, start);
    let first = &items(&list)[0];

    assert_eq!(first["label"], "greet");
    assert_eq!(
        first["textEdit"]["range"]["start"],
        json!({ "line": line, "character": character })
    );
}

#[test]
fn in_a_snippets_body_its_parameters_are_offered_and_in_its_header_they_are_not() {
    let (_workspace, mut client, uri) = snippet_page();
    let body = complete_at(
        &mut client,
        &uri,
        "<page>{#snippet row2(person, index)}<text>{|}</text>{/snippet}</page>",
    );
    assert_eq!(
        kinds(&body)[..2],
        [
            ("person".to_string(), VARIABLE),
            ("index".to_string(), VARIABLE)
        ]
    );
    assert!(
        labels(&body).contains(&"title".to_string()),
        "the scope too: {:?}",
        labels(&body)
    );

    let outside = complete_at(
        &mut client,
        &uri,
        "<page>{#snippet row2(person)}<text>x</text>{/snippet}<text>{|}</text></page>",
    );
    assert!(
        !labels(&outside).contains(&"person".to_string()),
        "{:?}",
        labels(&outside)
    );
}

#[test]
fn a_parameter_shadows_a_scope_name_of_the_same_name_and_the_item_of_an_each_is_one() {
    let (_workspace, mut client, uri) = snippet_page();
    let list = complete_at(&mut client, &uri, "<page>{#each people as title (title.name)}{#snippet s(row)}<text>{|}</text>{/snippet}{/each}</page>");
    let found = kinds(&list);

    assert_eq!(
        found[..2],
        [
            ("row".to_string(), VARIABLE),
            ("title".to_string(), VARIABLE)
        ]
    );
    assert_eq!(
        found.iter().filter(|(label, _)| label == "title").count(),
        1,
        "{found:?}"
    );
    assert_eq!(
        found.iter().filter(|(label, _)| label == "row").count(),
        1,
        "{found:?}"
    );
}

#[test]
fn a_string_attribute_of_a_closed_set_offers_its_members_and_replaces_the_text_inside_the_quotes() {
    let (_workspace, mut client, uri) = snippet_page();
    let empty = complete_at(&mut client, &uri, &page("<badge tone=\"|\" />"));
    assert_eq!(
        kinds(&empty),
        [
            ("info".to_string(), ENUM_MEMBER),
            ("danger".to_string(), ENUM_MEMBER),
            ("warn".to_string(), ENUM_MEMBER)
        ]
    );

    let marked = page("<badge tone=\"da|\" />");
    let partial = complete_at(&mut client, &uri, &marked);
    let source = marked.replacen('|', "", 1);
    let start = source.find("da\"").unwrap();
    let (line, character) = position(&source, start);

    assert_eq!(
        labels(&partial),
        ["info", "danger", "warn"],
        "the editor filters; the list is the set"
    );
    assert_eq!(
        items(&partial)[0]["textEdit"]["range"]["start"],
        json!({ "line": line, "character": character })
    );
    assert_eq!(
        items(&partial)[0]["textEdit"]["range"]["end"],
        json!({ "line": line, "character": character + 2 })
    );
}

#[test]
fn an_optional_closed_set_offers_its_members_and_a_string_prop_offers_nothing() {
    let (_workspace, mut client, uri) = snippet_page();
    assert_eq!(
        labels(&complete_at(
            &mut client,
            &uri,
            &page("<badge tone=\"info\" shade=\"|\" />")
        )),
        ["light", "dark"]
    );
    assert!(labels(&complete_at(
        &mut client,
        &uri,
        &page("<badge tone=\"info\" label=\"|\" />")
    ))
    .is_empty());
}

#[test]
fn a_string_written_as_an_expression_for_a_closed_set_offers_its_members_too() {
    let (_workspace, mut client, uri) = snippet_page();
    assert_eq!(
        labels(&complete_at(
            &mut client,
            &uri,
            &page("<badge tone={\"|\"} />")
        )),
        ["info", "danger", "warn"]
    );
}

#[test]
fn a_string_compared_with_a_closed_set_offers_its_members() {
    let (_workspace, mut client, uri) = snippet_page();
    assert_eq!(
        labels(&complete_at(
            &mut client,
            &uri,
            &page("<text>{tone == \"|\" ? title : title}</text>")
        )),
        ["info", "danger"]
    );
    assert_eq!(
        labels(&complete_at(
            &mut client,
            &uri,
            &page("<text>{\"|\" != tone ? title : title}</text>")
        )),
        ["info", "danger"]
    );
    // Not a closed set: nothing.
    assert!(labels(&complete_at(
        &mut client,
        &uri,
        &page("<text>{title == \"|\" ? title : title}</text>")
    ))
    .is_empty());
}
