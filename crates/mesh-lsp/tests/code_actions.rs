//! Quick fixes: one per suggestion a canonical diagnostic carries, and
//! only for the snapshot the client currently has.

mod common;

use common::Workspace;
use crossbeam_channel::unbounded;
use mesh_lsp::testing::{test_options, Client};
use mesh_lsp::Options;
use serde_json::{json, Value};
use std::time::Duration;

const BAD: &str = r#"<page title={usr.name} />"#;

fn settings() -> Value {
    json!({ "model": "components.json", "components": { "page.mprx": "users-page" } })
}

fn range(start: u32, end: u32) -> Value {
    json!({ "start": { "line": 0, "character": start }, "end": { "line": 0, "character": end } })
}

fn code_actions(client: &mut Client, uri: &str, range: Value) -> Vec<Value> {
    let response = client.request(
        "textDocument/codeAction",
        json!({
            "textDocument": { "uri": uri },
            "range": range,
            "context": { "diagnostics": [] }
        }),
    );
    match response.response_result {
        Ok(Value::Array(actions)) => actions,
        other => panic!("unexpected response {other:?}"),
    }
}

#[test]
fn a_suggestion_is_a_quick_fix() {
    let workspace = Workspace::with_example_manifest();
    let (mut client, _) = Client::initialized(workspace.init(settings(), json!({})));
    let uri = workspace.uri("page.mprx");
    client.open(&uri, 4, BAD);
    client.next_publication(&uri);

    // `usr` is at columns 13 to 16.
    let actions = code_actions(&mut client, &uri, range(14, 14));
    assert_eq!(actions.len(), 1, "{actions:#?}");
    let action = &actions[0];
    assert_eq!(action["kind"], "quickfix");
    assert_eq!(action["title"], "Replace with \"user\"");
    assert_eq!(action["isPreferred"], true);
    assert_eq!(action["diagnostics"][0]["code"], "unknown-reference");
    let edit = &action["edit"]["documentChanges"][0];
    assert_eq!(edit["textDocument"]["uri"], uri.as_str());
    assert_eq!(edit["textDocument"]["version"], 4);
    assert_eq!(edit["edits"][0]["newText"], "user");
    assert_eq!(edit["edits"][0]["range"], range(13, 16));
}

#[test]
fn no_actions_outside_a_diagnostic() {
    let workspace = Workspace::with_example_manifest();
    let (mut client, _) = Client::initialized(workspace.init(settings(), json!({})));
    let uri = workspace.uri("page.mprx");
    client.open(&uri, 1, BAD);
    client.next_publication(&uri);
    assert!(code_actions(&mut client, &uri, range(0, 3)).is_empty());
}

#[test]
fn a_diagnostic_without_suggestions_has_no_action() {
    let workspace = Workspace::with_example_manifest();
    let (mut client, _) = Client::initialized(workspace.init(settings(), json!({})));
    let uri = workspace.uri("page.mprx");
    // Nothing in scope is close to `zzzzzz`.
    client.open(&uri, 1, r#"<page title={zzzzzz} />"#);
    assert_eq!(client.next_publication(&uri).diagnostics.len(), 1);
    assert!(code_actions(&mut client, &uri, range(13, 19)).is_empty());
}

#[test]
fn no_actions_while_a_newer_version_is_pending() {
    let (release, gate) = unbounded();
    let options = Options {
        compile_gate: Some(gate),
        ..test_options()
    };
    let workspace = Workspace::with_example_manifest();
    let (mut client, _) = Client::initialized_with(options, workspace.init(settings(), json!({})));
    let uri = workspace.uri("page.mprx");
    client.open(&uri, 1, BAD);
    release.send(()).unwrap();
    client.next_publication(&uri);
    assert_eq!(code_actions(&mut client, &uri, range(14, 14)).len(), 1);

    // Version 2 is held: its text moved `usr`, so version 1's edit would
    // land in the wrong place.
    client.change(&uri, 2, &format!("  {BAD}"));
    assert!(code_actions(&mut client, &uri, range(14, 14)).is_empty());
    release.send(()).unwrap();
    assert_eq!(client.next_publication(&uri).version, Some(2));
    let actions = code_actions(&mut client, &uri, range(16, 16));
    assert_eq!(actions.len(), 1);
    assert_eq!(
        actions[0]["edit"]["documentChanges"][0]["edits"][0]["range"],
        range(15, 18)
    );
    client.no_publication(&uri, Duration::from_millis(100));
}
