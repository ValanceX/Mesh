//! Configuration and explicit component identity (outline D6): only
//! `mesh.components` makes a document the template of a component. A file
//! name never does.

mod common;

use common::{codes, Workspace};
use mesh_lsp::testing::{test_options, Client};
use serde_json::json;
use std::time::Duration;

const BAD: &str = r#"<page title={usr.name} />"#;

fn settings() -> serde_json::Value {
    json!({ "model": "components.json", "components": { "users-page.mprx": "users-page" } })
}

#[test]
fn a_configured_document_is_checked_against_its_component() {
    let workspace = Workspace::with_example_manifest();
    let (mut client, _) = Client::initialized(workspace.init(settings(), json!({})));
    let uri = workspace.uri("users-page.mprx");
    client.open(&uri, 1, BAD);
    assert_eq!(
        codes(&client.next_publication(&uri).diagnostics),
        ["unknown-reference"]
    );
}

#[test]
fn an_unconfigured_document_is_checked_without_a_model() {
    let workspace = Workspace::with_example_manifest();
    let (mut client, _) = Client::initialized(workspace.init(settings(), json!({})));
    // Named like a component, but not in the map.
    let uri = workspace.uri("page.mprx");
    client.open(&uri, 1, BAD);
    assert!(client.next_publication(&uri).diagnostics.is_empty());
    let logs = client.logs(Duration::ZERO);
    assert!(
        logs.iter().any(|log| log.contains("has no component")),
        "{logs:#?}"
    );
}

#[test]
fn an_untitled_document_is_checked_without_a_model() {
    let workspace = Workspace::with_example_manifest();
    let (mut client, _) = Client::initialized(workspace.init(settings(), json!({})));
    let uri = "untitled:Untitled-1";
    client.open(uri, 1, BAD);
    assert!(client.next_publication(uri).diagnostics.is_empty());
    // Syntax is still checked.
    client.change(uri, 2, "<page");
    assert!(!client.next_publication(uri).diagnostics.is_empty());
}

#[test]
fn renaming_a_file_does_not_carry_its_component() {
    let workspace = Workspace::with_example_manifest();
    let (mut client, _) = Client::initialized(workspace.init(settings(), json!({})));
    let old = workspace.uri("users-page.mprx");
    client.open(&old, 1, BAD);
    assert_eq!(client.next_publication(&old).diagnostics.len(), 1);
    client.close(&old);
    client.next_publication(&old);
    let new = workspace.uri("renamed.mprx");
    client.open(&new, 1, BAD);
    assert!(client.next_publication(&new).diagnostics.is_empty());
}

#[test]
fn a_document_outside_the_root_is_checked_without_a_model() {
    let workspace = Workspace::with_example_manifest();
    let (mut client, _) = Client::initialized(workspace.init(settings(), json!({})));
    let uri = "file:///somewhere/else/users-page.mprx";
    client.open(uri, 1, BAD);
    assert!(client.next_publication(uri).diagnostics.is_empty());
}

#[test]
fn pushed_settings_are_applied() {
    let workspace = Workspace::with_example_manifest();
    let (mut client, _) = Client::initialized(workspace.init(json!(null), json!({})));
    let uri = workspace.uri("users-page.mprx");
    client.open(&uri, 1, BAD);
    assert!(client.next_publication(&uri).diagnostics.is_empty());
    client.notify(
        "workspace/didChangeConfiguration",
        json!({ "settings": { "mesh": settings() } }),
    );
    assert_eq!(
        codes(&client.next_publication(&uri).diagnostics),
        ["unknown-reference"]
    );
}

#[test]
fn pulled_settings_are_applied() {
    let workspace = Workspace::with_example_manifest();
    let capabilities = json!({ "workspace": { "configuration": true } });
    let (mut client, _) = Client::initialized(workspace.init(json!(null), capabilities));
    client.configuration = settings();
    let uri = workspace.uri("users-page.mprx");
    client.open(&uri, 1, BAD);
    assert!(client.next_publication(&uri).diagnostics.is_empty());
    // The notification's own settings are ignored: the server asks.
    client.notify(
        "workspace/didChangeConfiguration",
        json!({ "settings": null }),
    );
    assert_eq!(
        codes(&client.next_publication(&uri).diagnostics),
        ["unknown-reference"]
    );
}

#[test]
fn settings_of_the_wrong_type_are_logged_not_fatal() {
    let workspace = Workspace::with_example_manifest();
    let bad = json!({ "model": 3, "components": { "a.mprx": false } });
    let (mut client, _) = Client::initialized(workspace.init(bad, json!({})));
    let logs = client.logs(Duration::from_millis(100));
    assert!(
        logs.iter().any(|log| log.contains("mesh.model")),
        "{logs:#?}"
    );
    assert!(
        logs.iter().any(|log| log.contains("mesh.components")),
        "{logs:#?}"
    );
    let uri = workspace.uri("a.mprx");
    client.open(&uri, 1, BAD);
    assert!(client.next_publication(&uri).diagnostics.is_empty());
}

#[test]
fn without_a_root_the_model_is_logged_and_unused() {
    let workspace = Workspace::with_example_manifest();
    let (mut client, _) = Client::initialized_with(
        test_options(),
        json!({ "processId": null, "rootUri": null, "capabilities": {},
                "initializationOptions": settings() }),
    );
    let logs = client.logs(Duration::from_millis(100));
    assert!(
        logs.iter().any(|log| log.contains("no workspace root")),
        "{logs:#?}"
    );
    let uri = workspace.uri("users-page.mprx");
    client.open(&uri, 1, BAD);
    assert!(client.next_publication(&uri).diagnostics.is_empty());
}
