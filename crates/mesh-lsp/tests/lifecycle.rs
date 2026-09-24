//! Document snapshots and publication (outline D10): what each event
//! publishes, and the staleness rule, that a result computed for an older
//! snapshot is never published over a newer one.

mod common;

use common::{codes, Workspace};
use crossbeam_channel::{unbounded, Sender};
use mesh_lsp::testing::{test_options, Client};
use mesh_lsp::Options;
use serde_json::json;
use std::time::Duration;

const QUIET: Duration = Duration::from_millis(300);

/// A workspace where `page.mprx` is the template of `users-page`, so a
/// misspelled scope name is an `unknown-reference`.
fn configured(options: Options) -> (Workspace, Client, String) {
    let workspace = Workspace::with_example_manifest();
    let settings = json!({
        "model": "components.json",
        "components": { "page.mprx": "users-page" }
    });
    let (client, _) = Client::initialized_with(options, workspace.init(settings, json!({})));
    let uri = workspace.uri("page.mprx");
    (workspace, client, uri)
}

fn gated() -> (Options, Sender<()>) {
    let (release, gate) = unbounded();
    let options = Options {
        compile_gate: Some(gate),
        ..test_options()
    };
    (options, release)
}

const GOOD: &str = r#"<page title="Users" />"#;
const BAD: &str = r#"<page title={usr.name} />"#;

#[test]
fn did_open_publishes_that_version() {
    let (_workspace, mut client, uri) = configured(test_options());
    client.open(&uri, 7, BAD);
    let publication = client.next_publication(&uri);
    assert_eq!(publication.version, Some(7));
    assert_eq!(codes(&publication.diagnostics), ["unknown-reference"]);
}

#[test]
fn a_change_publishes_the_new_version() {
    let (_workspace, mut client, uri) = configured(test_options());
    client.open(&uri, 1, BAD);
    client.next_publication(&uri);
    client.change(&uri, 2, GOOD);
    let publication = client.next_publication(&uri);
    assert_eq!(publication.version, Some(2));
    assert!(publication.diagnostics.is_empty());
}

#[test]
fn a_burst_of_changes_publishes_the_last() {
    let options = Options {
        debounce: Duration::from_millis(100),
        ..test_options()
    };
    let (_workspace, mut client, uri) = configured(options);
    client.open(&uri, 1, GOOD);
    assert_eq!(client.next_publication(&uri).version, Some(1));
    for version in 2..=10 {
        let text = if version % 2 == 0 { BAD } else { GOOD };
        client.change(&uri, version, text);
    }
    let publication = client.next_publication(&uri);
    assert_eq!(publication.version, Some(10));
    assert_eq!(codes(&publication.diagnostics), ["unknown-reference"]);
    client.no_publication(&uri, QUIET);
}

#[test]
fn an_older_result_never_replaces_a_newer_one() {
    let (options, release) = gated();
    let (_workspace, mut client, uri) = configured(options);
    client.open(&uri, 1, GOOD);
    release.send(()).unwrap();
    assert_eq!(client.next_publication(&uri).version, Some(1));

    // Version 2's compile is held while version 3 arrives. Then both are
    // released: version 2's result is stale and must be dropped.
    client.change(&uri, 2, BAD);
    client.change(&uri, 3, GOOD);
    client.no_publication(&uri, QUIET);
    release.send(()).unwrap();
    release.send(()).unwrap();
    let publication = client.next_publication(&uri);
    assert_eq!(publication.version, Some(3));
    assert!(publication.diagnostics.is_empty());
    client.no_publication(&uri, QUIET);
}

#[test]
fn a_result_for_a_closed_document_is_dropped() {
    let (options, release) = gated();
    let (_workspace, mut client, uri) = configured(options);
    client.open(&uri, 1, BAD);
    client.close(&uri);
    let cleared = client.next_publication(&uri);
    assert_eq!(cleared.version, None);
    assert!(cleared.diagnostics.is_empty());
    release.send(()).unwrap();
    client.no_publication(&uri, QUIET);
}

#[test]
fn did_close_clears_diagnostics() {
    let (_workspace, mut client, uri) = configured(test_options());
    client.open(&uri, 1, BAD);
    assert_eq!(client.next_publication(&uri).diagnostics.len(), 1);
    client.close(&uri);
    let publication = client.next_publication(&uri);
    assert_eq!(publication.version, None);
    assert!(publication.diagnostics.is_empty());
}

#[test]
fn a_lower_version_is_ignored() {
    let (_workspace, mut client, uri) = configured(test_options());
    client.open(&uri, 5, GOOD);
    client.next_publication(&uri);
    client.change(&uri, 3, BAD);
    client.no_publication(&uri, QUIET);
    let logs = client.logs(Duration::ZERO);
    assert!(
        logs.iter().any(|log| log.contains("older than 5")),
        "{logs:#?}"
    );
}

#[test]
fn a_syntax_error_publishes_only_syntax_errors() {
    // With a model, a broken file still reports exactly what mesh check
    // does: its syntax errors, and no analysis (invariant I3).
    let (_workspace, mut client, uri) = configured(test_options());
    client.open(&uri, 1, "<page title={usr.} />");
    let publication = client.next_publication(&uri);
    assert!(!publication.diagnostics.is_empty());
    assert!(codes(&publication.diagnostics)
        .iter()
        .all(|code| code == "syntax-error"));
}
