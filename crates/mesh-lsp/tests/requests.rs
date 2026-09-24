//! Requests wait for their snapshot (outline D10): a request about a
//! document is answered from the snapshot current when it arrives, once
//! that snapshot is compiled, and `ContentModified` if a newer one
//! replaces it first.

mod common;

use common::Workspace;
use crossbeam_channel::{unbounded, Sender};
use lsp_server::{ErrorCode, RequestId};
use mesh_lsp::testing::{position_params, test_options, Client};
use mesh_lsp::Options;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

const QUIET: Duration = Duration::from_millis(300);

/// `page.mprx` is the template of `users-page`.
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

const PLAIN: &str = r#"<page title="x" />"#;
/// `name` is at character 20 of line 0.
const MEMBER: &str = r#"<page title={user.name} />"#;

fn hover_at_name(client: &mut Client, uri: &str) -> RequestId {
    client.send("textDocument/hover", position_params(uri, 0, 20))
}

fn error_code(client: &mut Client, id: &RequestId) -> i32 {
    let response = client
        .response(id, Duration::from_secs(10))
        .expect("an answer");
    response.response_result.expect_err("an error").code
}

fn hover_value(result: &Value) -> &str {
    result["contents"]["value"]
        .as_str()
        .expect("a markup hover")
}

#[test]
fn a_hover_right_after_a_change_waits_for_the_compile() {
    // With a long debounce, the hover must cut it short.
    let options = Options {
        debounce: Duration::from_secs(30),
        ..test_options()
    };
    let (_workspace, mut client, uri) = configured(options);
    client.open(&uri, 1, PLAIN);
    client.next_publication(&uri);
    client.change(&uri, 2, MEMBER);
    let started = Instant::now();
    let id = hover_at_name(&mut client, &uri);
    let response = client
        .response(&id, Duration::from_secs(10))
        .expect("an answer");
    assert!(started.elapsed() < Duration::from_secs(10));
    assert_eq!(
        hover_value(&response.response_result.expect("a result")),
        "string"
    );
    // The cut-short compile published version 2.
    assert_eq!(client.next_publication(&uri).version, Some(2));
}

#[test]
fn a_held_request_is_content_modified_by_a_newer_change() {
    let (options, release) = gated();
    let (_workspace, mut client, uri) = configured(options);
    client.open(&uri, 1, PLAIN);
    release.send(()).unwrap();
    client.next_publication(&uri);
    client.change(&uri, 2, MEMBER);
    let id = hover_at_name(&mut client, &uri);
    assert!(client.response(&id, QUIET).is_none(), "held until compiled");
    client.change(&uri, 3, MEMBER);
    assert_eq!(
        error_code(&mut client, &id),
        ErrorCode::ContentModified as i32
    );
    release.send(()).unwrap();
    release.send(()).unwrap();
    // Version 3's answer still comes, for a new request.
    assert_eq!(client.latest_publication(&uri, QUIET).version, Some(3));
    let result = client.hover(&uri, 0, 20);
    assert_eq!(hover_value(&result), "string");
}

#[test]
fn a_held_request_can_be_cancelled() {
    let (options, _release) = gated();
    let (_workspace, mut client, uri) = configured(options);
    client.open(&uri, 1, MEMBER);
    let id = hover_at_name(&mut client, &uri);
    client.notify("$/cancelRequest", json!({ "id": number(&id) }));
    assert_eq!(
        error_code(&mut client, &id),
        ErrorCode::RequestCanceled as i32
    );
}

/// The request number the harness gave `id`.
fn number(id: &RequestId) -> i64 {
    serde_json::to_value(id)
        .expect("an id serializes")
        .as_i64()
        .expect("the harness numbers its requests")
}

#[test]
fn closing_a_document_answers_its_held_requests() {
    let (options, _release) = gated();
    let (_workspace, mut client, uri) = configured(options);
    client.open(&uri, 1, MEMBER);
    let id = hover_at_name(&mut client, &uri);
    client.close(&uri);
    assert_eq!(
        error_code(&mut client, &id),
        ErrorCode::ContentModified as i32
    );
}

#[test]
fn a_manifest_edit_answers_held_requests() {
    let (options, _release) = gated();
    let (workspace, mut client, uri) = configured(options);
    client.open(&uri, 1, MEMBER);
    let id = hover_at_name(&mut client, &uri);
    client.open(
        &workspace.uri("components.json"),
        1,
        &common::example("components.json"),
    );
    assert_eq!(
        error_code(&mut client, &id),
        ErrorCode::ContentModified as i32
    );
}

#[test]
fn shutdown_answers_held_requests_first() {
    let (options, _release) = gated();
    let (_workspace, mut client, uri) = configured(options);
    client.open(&uri, 1, MEMBER);
    let id = hover_at_name(&mut client, &uri);
    let shutdown = client.request("shutdown", Value::Null);
    assert!(shutdown.response_result.is_ok());
    let held = client
        .response(&id, QUIET)
        .expect("answered before shutdown");
    assert_eq!(held.response_result.expect("a result"), Value::Null);
}

#[test]
fn a_request_for_a_document_that_isnt_open_is_null() {
    let (workspace, mut client, _uri) = configured(test_options());
    let result = client.hover(&workspace.uri("other.mprx"), 0, 0);
    assert_eq!(result, Value::Null);
    let result = client.definition(&workspace.uri("other.mprx"), 0, 0);
    assert_eq!(result, Value::Null);
}
