//! What each open buffer is (Pass 5, Decisions 1–3; audit findings C1–C3
//! and B4): the configured manifest, whatever its `languageId`; an MPRX
//! document, by `languageId` `mprx`; or nothing the server checks. What
//! a buffer is gets decided again whenever the configuration changes.

mod common;

use common::{codes, example, Workspace};
use mesh_lsp::testing::{Client, Publication};
use serde_json::{json, Value};
use std::time::Duration;

const QUIET: Duration = Duration::from_millis(300);

/// An unknown reference against the example manifest's `users-page`,
/// and clean without a model.
const BAD: &str = r#"<page title={usr.name} />"#;

/// Clean against the example manifest's `users-page`; an unknown
/// reference against [`renamed_scope`].
const PAGE: &str = r#"<page title={user.name} />"#;

/// A manifest with an error in it.
const BROKEN: &str = r#"{ "version": 1, "components": 3 }"#;

fn settings(model: &str) -> Value {
    json!({ "model": model, "components": { "page.mprx": "users-page" } })
}

fn push(client: &Client, settings: Value) {
    client.notify(
        "workspace/didChangeConfiguration",
        json!({ "settings": { "mesh": settings } }),
    );
}

/// The example manifest with `users-page`'s scope name `user` renamed.
fn renamed_scope() -> String {
    let manifest = example("components.json");
    let renamed = manifest.replacen(
        r#""user": { "kind": "named", "name": "User" }"#,
        r#""person": { "kind": "named", "name": "User" }"#,
        1,
    );
    assert_ne!(renamed, manifest);
    renamed
}

fn latest(client: &mut Client, uri: &str) -> Publication {
    client.latest_publication(uri, QUIET)
}

#[test]
fn switching_the_model_while_the_old_manifest_is_open_loads_the_new_one() {
    let workspace = Workspace::with_example_manifest();
    workspace.write("other.json", BROKEN);
    let (mut client, _) =
        Client::initialized(workspace.init(settings("components.json"), json!({})));
    let old = workspace.uri("components.json");
    let new = workspace.uri("other.json");
    let page = workspace.uri("page.mprx");
    client.open_as(&old, "json", 1, &example("components.json"));
    client.open(&page, 1, BAD);
    assert_eq!(
        codes(&latest(&mut client, &page).diagnostics),
        ["unknown-reference"]
    );

    push(&client, settings("other.json"));
    // The new manifest is read from disk, and its errors are published
    // on its own URI...
    let published = client.next_publication(&new);
    assert!(!published.diagnostics.is_empty());
    assert!(codes(&published.diagnostics)
        .iter()
        .all(|code| code.starts_with("manifest-")));
    // ...documents are checked without a model while it's broken...
    assert!(latest(&mut client, &page).diagnostics.is_empty());
    // ...and the old manifest is cleared.
    assert!(latest(&mut client, &old).diagnostics.is_empty());
}

#[test]
fn a_manifest_opened_before_it_is_configured_becomes_the_manifest() {
    let workspace = Workspace::with_example_manifest();
    let (mut client, _) = Client::initialized(workspace.init(json!(null), json!({})));
    let manifest = workspace.uri("components.json");
    let page = workspace.uri("page.mprx");
    // An unsaved edit: the buffer differs from the disk.
    client.open_as(&manifest, "json", 1, &renamed_scope());
    client.no_publication(&manifest, QUIET);

    push(&client, settings("components.json"));
    client.open(&page, 1, PAGE);
    // The buffer is the model, not the disk.
    assert_eq!(
        codes(&latest(&mut client, &page).diagnostics),
        ["unknown-reference"]
    );
    assert!(latest(&mut client, &manifest).diagnostics.is_empty());
}

#[test]
fn a_document_that_becomes_the_manifest_has_its_diagnostics_cleared() {
    let workspace = Workspace::with_example_manifest();
    let (mut client, _) = Client::initialized(workspace.init(json!(null), json!({})));
    let manifest = workspace.uri("components.json");
    // Sent as MPRX: until it's configured as the manifest, it's checked
    // as MPRX, and JSON isn't.
    client.open(&manifest, 1, &example("components.json"));
    assert_eq!(
        codes(&client.next_publication(&manifest).diagnostics),
        ["syntax-error"]
    );

    push(&client, settings("components.json"));
    assert!(latest(&mut client, &manifest).diagnostics.is_empty());
    // Edits now reach the manifest.
    client.change(&manifest, 2, BROKEN);
    let published = latest(&mut client, &manifest);
    assert!(codes(&published.diagnostics)
        .iter()
        .all(|code| code.starts_with("manifest-")));
    assert!(!published.diagnostics.is_empty());
}

#[test]
fn changing_the_model_to_an_open_file_uses_its_buffer() {
    let workspace = Workspace::with_example_manifest();
    let (mut client, _) = Client::initialized(workspace.init(settings("missing.json"), json!({})));
    let manifest = workspace.uri("components.json");
    let page = workspace.uri("page.mprx");
    client.open_as(&manifest, "json", 1, &renamed_scope());
    client.open(&page, 1, PAGE);
    assert!(latest(&mut client, &page).diagnostics.is_empty());

    push(&client, settings("components.json"));
    assert_eq!(
        codes(&latest(&mut client, &page).diagnostics),
        ["unknown-reference"]
    );
}

#[test]
fn a_change_to_the_old_manifest_after_a_switch_is_kept() {
    let workspace = Workspace::with_example_manifest();
    let (mut client, _) =
        Client::initialized(workspace.init(settings("components.json"), json!({})));
    let manifest = workspace.uri("components.json");
    let page = workspace.uri("page.mprx");
    client.open_as(&manifest, "json", 1, &example("components.json"));
    client.open(&page, 1, PAGE);
    assert!(latest(&mut client, &page).diagnostics.is_empty());

    push(&client, settings("missing.json"));
    // Not the manifest now, but still open: the edit is tracked.
    client.change(&manifest, 2, &renamed_scope());
    push(&client, settings("components.json"));
    assert_eq!(
        codes(&latest(&mut client, &page).diagnostics),
        ["unknown-reference"]
    );
    let logs = client.logs(Duration::ZERO);
    assert!(
        !logs.iter().any(|log| log.contains("isn't open")),
        "{logs:#?}"
    );
}

#[test]
fn a_json_file_that_isnt_the_manifest_is_ignored() {
    let workspace = Workspace::with_example_manifest();
    let (mut client, _) =
        Client::initialized(workspace.init(settings("components.json"), json!({})));
    let other = workspace.uri("package.json");
    client.open_as(&other, "json", 1, r#"{ "name": "x" }"#);
    client.change(&other, 2, r#"{ "name": "y" }"#);
    client.no_publication(&other, QUIET);
    client.close(&other);
    client.no_publication(&other, QUIET);
}

#[test]
fn a_document_is_mprx_by_language_id_not_extension() {
    let workspace = Workspace::with_example_manifest();
    let (mut client, _) =
        Client::initialized(workspace.init(settings("components.json"), json!({})));
    let untitled = "untitled:Untitled-1";
    client.open_as(untitled, "mprx", 1, "<page");
    assert!(!client.next_publication(untitled).diagnostics.is_empty());
    let text = workspace.uri("notes.mprx");
    client.open_as(&text, "plaintext", 1, "<page");
    client.no_publication(&text, QUIET);
}

fn registered_pattern(client: &Client) -> Value {
    let registration = client
        .registrations
        .last()
        .cloned()
        .expect("the server registered a watcher");
    registration["registrations"][0]["registerOptions"]["watchers"][0]["globPattern"].clone()
}

#[test]
fn the_watcher_matches_the_file_name_anywhere_without_relative_patterns() {
    let workspace = Workspace::empty();
    workspace.write("dir [x]/m{1}.json", &example("components.json"));
    let capabilities =
        json!({ "workspace": { "didChangeWatchedFiles": { "dynamicRegistration": true } } });
    let (mut client, _) =
        Client::initialized(workspace.init(settings("dir [x]/m{1}.json"), capabilities));
    client.logs(QUIET);
    assert_eq!(registered_pattern(&client), json!("**/m[{]1[}].json"));
}

#[test]
fn the_watcher_is_relative_to_the_manifest_when_the_client_can() {
    let workspace = Workspace::empty();
    workspace.write("dir [x]/m{1}.json", &example("components.json"));
    let capabilities = json!({ "workspace": { "didChangeWatchedFiles": {
        "dynamicRegistration": true, "relativePatternSupport": true
    } } });
    let (mut client, _) =
        Client::initialized(workspace.init(settings("dir [x]/m{1}.json"), capabilities));
    client.logs(QUIET);
    let pattern = registered_pattern(&client);
    assert_eq!(pattern["pattern"], "m[{]1[}].json");
    assert_eq!(pattern["baseUri"], json!(workspace.uri("dir [x]")));

    // A change on disk still reloads it.
    let page = workspace.uri("page.mprx");
    client.open(&page, 1, PAGE);
    assert!(latest(&mut client, &page).diagnostics.is_empty());
    workspace.write("dir [x]/m{1}.json", &renamed_scope());
    client.notify(
        "workspace/didChangeWatchedFiles",
        json!({ "changes": [{ "uri": workspace.uri("dir [x]/m{1}.json"), "type": 2 }] }),
    );
    assert_eq!(
        codes(&latest(&mut client, &page).diagnostics),
        ["unknown-reference"]
    );
}
