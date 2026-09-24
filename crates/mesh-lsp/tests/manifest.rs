//! The manifest's lifecycle (outline D9): loading it, re-checking every
//! dependent document when it changes, and publishing its own
//! diagnostics. Every document is always checked against one coherent
//! manifest snapshot.

mod common;

use common::{codes, Workspace};
use crossbeam_channel::unbounded;
use mesh_lsp::testing::{test_options, Client};
use mesh_lsp::Options;
use serde_json::{json, Value};
use std::time::Duration;

const QUIET: Duration = Duration::from_millis(300);

/// Uses `user` from the scope, which the example declares for
/// `users-page`: clean against the example manifest.
const PAGE: &str = r#"<page title={user.name} />"#;

fn settings() -> Value {
    json!({ "model": "components.json", "components": { "page.mprx": "users-page" } })
}

/// The example manifest with `users-page`'s scope name `user` renamed to
/// `person`, so `PAGE` has an unknown reference against it.
fn renamed_scope(manifest: &str) -> String {
    manifest.replacen(
        r#""user": { "kind": "named", "name": "User" }"#,
        r#""person": { "kind": "named", "name": "User" }"#,
        1,
    )
}

fn start(workspace: &Workspace, capabilities: Value) -> (Client, String, String) {
    let (client, _) = Client::initialized(workspace.init(settings(), capabilities));
    (
        client,
        workspace.uri("page.mprx"),
        workspace.uri("components.json"),
    )
}

#[test]
fn renaming_the_scope_name_breaks_the_page() {
    // Guards the helper: it must really change the manifest.
    let manifest = common::example("components.json");
    assert_ne!(renamed_scope(&manifest), manifest);
}

#[test]
fn the_manifest_is_loaded_at_initialized() {
    let workspace = Workspace::with_example_manifest();
    let (mut client, page, _) = start(&workspace, json!({}));
    client.open(&page, 1, r#"<page title={usr.name} />"#);
    let publication = client.next_publication(&page);
    assert_eq!(codes(&publication.diagnostics), ["unknown-reference"]);
}

#[test]
fn editing_the_open_manifest_rechecks_dependents() {
    let workspace = Workspace::with_example_manifest();
    let (mut client, page, manifest) = start(&workspace, json!({}));
    client.open(&page, 1, PAGE);
    assert!(client.next_publication(&page).diagnostics.is_empty());

    let text = common::example("components.json");
    client.open(&manifest, 1, &text);
    client.change(&manifest, 2, &renamed_scope(&text));
    // Opening the manifest re-checks the page (still clean); the edit
    // re-checks it against the new text.
    let mut last = client.next_publication(&page);
    while let Some(next) = client.try_publication(&page, QUIET) {
        last = next;
    }
    assert_eq!(codes(&last.diagnostics), ["unknown-reference"]);
    assert_eq!(last.version, Some(1));
}

#[test]
fn closing_the_manifest_reverts_to_disk() {
    let workspace = Workspace::with_example_manifest();
    let (mut client, page, manifest) = start(&workspace, json!({}));
    client.open(&page, 1, PAGE);
    client.next_publication(&page);
    let text = common::example("components.json");
    client.open(&manifest, 1, &renamed_scope(&text));
    assert_eq!(
        codes(&client.next_publication(&page).diagnostics),
        ["unknown-reference"]
    );
    // The unsaved edit is discarded: disk still has the original.
    client.close(&manifest);
    assert!(client.next_publication(&page).diagnostics.is_empty());
}

#[test]
fn a_watched_change_on_disk_rechecks_dependents() {
    let workspace = Workspace::with_example_manifest();
    let capabilities =
        json!({ "workspace": { "didChangeWatchedFiles": { "dynamicRegistration": true } } });
    let (mut client, page, manifest) = start(&workspace, capabilities);
    client.open(&page, 1, PAGE);
    assert!(client.next_publication(&page).diagnostics.is_empty());
    // The server asked to watch the manifest's path.
    let registration = client
        .registrations
        .first()
        .cloned()
        .expect("the server registered a watcher");
    let pattern = registration["registrations"][0]["registerOptions"]["watchers"][0]["globPattern"]
        .as_str()
        .expect("a glob pattern")
        .to_string();
    assert!(pattern.ends_with("components.json"), "{pattern}");

    workspace.write(
        "components.json",
        &renamed_scope(&common::example("components.json")),
    );
    client.notify(
        "workspace/didChangeWatchedFiles",
        json!({ "changes": [{ "uri": manifest, "type": 2 }] }),
    );
    assert_eq!(
        codes(&client.next_publication(&page).diagnostics),
        ["unknown-reference"]
    );
}

#[test]
fn breaking_the_manifest_moves_dependents_to_modelless_checking() {
    let workspace = Workspace::with_example_manifest();
    let (mut client, page, manifest) = start(&workspace, json!({}));
    client.open(&page, 1, r#"<page title={usr.name} />"#);
    assert_eq!(client.next_publication(&page).diagnostics.len(), 1);

    let text = common::example("components.json");
    client.open(
        &manifest,
        1,
        &text.replacen("\"version\": 1", "\"version\": 2", 1),
    );
    // The manifest's own error is published on it...
    let published = client.latest_publication(&manifest, QUIET);
    assert_eq!(
        codes(&published.diagnostics),
        ["manifest-unsupported-version"]
    );
    // ...and the page is checked without a model: no unknown reference.
    assert!(client.next_publication(&page).diagnostics.is_empty());
}

#[test]
fn fixing_the_manifest_restores_model_diagnostics() {
    let workspace = Workspace::with_example_manifest();
    let (mut client, page, manifest) = start(&workspace, json!({}));
    client.open(&page, 1, r#"<page title={usr.name} />"#);
    client.next_publication(&page);
    let text = common::example("components.json");
    client.open(&manifest, 1, "{ not json");
    assert!(client
        .latest_publication(&page, QUIET)
        .diagnostics
        .is_empty());
    assert_eq!(
        codes(&client.latest_publication(&manifest, QUIET).diagnostics),
        ["manifest-syntax-error"]
    );
    client.change(&manifest, 2, &text);
    assert!(client
        .latest_publication(&manifest, QUIET)
        .diagnostics
        .is_empty());
    assert_eq!(
        codes(&client.latest_publication(&page, QUIET).diagnostics),
        ["unknown-reference"]
    );
}

#[test]
fn a_missing_component_is_reported_on_the_manifest() {
    let workspace = Workspace::with_example_manifest();
    let settings = json!({
        "model": "components.json",
        "components": { "page.mprx": "no-such-component" }
    });
    let (mut client, _) = Client::initialized(workspace.init(settings, json!({})));
    let page = workspace.uri("page.mprx");
    let manifest = workspace.uri("components.json");
    client.open(&page, 1, r#"<page title={usr.name} />"#);
    // The page is checked without a model...
    assert!(client.next_publication(&page).diagnostics.is_empty());
    // ...and the manifest says what's missing.
    let published = client.latest_publication(&manifest, QUIET);
    assert_eq!(
        codes(&published.diagnostics),
        ["manifest-missing-component"]
    );
    assert!(published.diagnostics[0]
        .message
        .contains("no-such-component"));
    // Closing the only document that needs it clears it.
    client.close(&page);
    assert!(client
        .latest_publication(&manifest, QUIET)
        .diagnostics
        .is_empty());
}

#[test]
fn an_unreadable_manifest_is_logged_and_ignored() {
    let workspace = Workspace::empty();
    let (mut client, page, _) = start(&workspace, json!({}));
    let logs = client.logs(Duration::from_millis(100));
    assert!(
        logs.iter()
            .any(|log| log.contains("couldn't read the manifest")),
        "{logs:#?}"
    );
    client.open(&page, 1, r#"<page title={usr.name} />"#);
    assert!(client.next_publication(&page).diagnostics.is_empty());
}

#[test]
fn a_manifest_change_during_a_held_compile_drops_the_old_result() {
    let (release, gate) = unbounded();
    let options = Options {
        compile_gate: Some(gate),
        ..test_options()
    };
    let workspace = Workspace::with_example_manifest();
    let (mut client, _) = Client::initialized_with(options, workspace.init(settings(), json!({})));
    let page = workspace.uri("page.mprx");
    let manifest = workspace.uri("components.json");

    // Held: the page's compile against the original manifest.
    client.open(&page, 1, PAGE);
    // Then the manifest changes, which queues a compile against it.
    let text = common::example("components.json");
    client.open(&manifest, 1, &renamed_scope(&text));
    client.no_publication(&page, QUIET);
    release.send(()).unwrap();
    release.send(()).unwrap();
    // Only the compile against the current manifest is published.
    let publication = client.next_publication(&page);
    assert_eq!(codes(&publication.diagnostics), ["unknown-reference"]);
    client.no_publication(&page, QUIET);
}

/// A valid manifest is published clean when it's loaded, which clears
/// anything an earlier session left on it.
#[test]
fn a_valid_manifest_is_published_clean_when_loaded() {
    let workspace = Workspace::with_example_manifest();
    let (mut client, _, manifest) = start(&workspace, json!({}));
    let publication = client.next_publication(&manifest);
    assert_eq!(publication.version, None);
    assert!(publication.diagnostics.is_empty());
}
