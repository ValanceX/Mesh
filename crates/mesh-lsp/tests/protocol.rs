//! The protocol: the handshake, capabilities, position encodings, and
//! every state and malformed message the crash-safety contract names
//! (outline D7). After each bad message, the server must still answer.

mod common;

use lsp_server::ErrorCode;
use mesh_lsp::testing::{test_options, Client};
use mesh_lsp::{Options, Outcome};
use serde_json::{json, Value};

fn init(capabilities: Value) -> Value {
    json!({ "processId": null, "rootUri": null, "capabilities": capabilities })
}

/// Asserts that `response` is an error with `code`.
fn assert_error(response: &lsp_server::Response, code: ErrorCode) {
    match &response.response_result {
        Err(error) => assert_eq!(error.code, code as i32, "{error:?}"),
        Ok(result) => panic!("expected {code:?}, got {result:?}"),
    }
}

/// A request the server answers with no side effects, to show it's alive.
fn still_answers(client: &mut Client) {
    let response = client.request(
        "textDocument/codeAction",
        json!({
            "textDocument": { "uri": "file:///nowhere.mprx" },
            "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } },
            "context": { "diagnostics": [] }
        }),
    );
    assert_eq!(response.response_result.ok(), Some(json!([])));
}

#[test]
fn initialize_advertises_the_capabilities() {
    let (client, result) = Client::initialized(init(json!({})));
    let capabilities = &result["capabilities"];
    assert_eq!(capabilities["positionEncoding"], "utf-16");
    assert_eq!(capabilities["textDocumentSync"]["openClose"], true);
    assert_eq!(capabilities["textDocumentSync"]["change"], 1);
    assert_eq!(
        capabilities["codeActionProvider"]["codeActionKinds"],
        json!(["quickfix"])
    );
    assert_eq!(result["serverInfo"]["name"], "mesh-lsp");
    assert_eq!(
        client.exit(true),
        Outcome::Exited {
            after_shutdown: true
        }
    );
}

#[test]
fn negotiates_utf8_when_offered() {
    let offer = json!({ "general": { "positionEncodings": ["utf-16", "utf-8"] } });
    let (_client, result) = Client::initialized(init(offer));
    assert_eq!(result["capabilities"]["positionEncoding"], "utf-8");
}

#[test]
fn negotiates_utf32_as_chars() {
    let offer = json!({ "general": { "positionEncodings": ["utf-32", "utf-16"] } });
    let (_client, result) = Client::initialized(init(offer));
    assert_eq!(result["capabilities"]["positionEncoding"], "utf-32");
}

#[test]
fn defaults_to_utf16() {
    let offer = json!({ "general": { "positionEncodings": ["something-else"] } });
    let (_client, result) = Client::initialized(init(offer));
    assert_eq!(result["capabilities"]["positionEncoding"], "utf-16");
}

#[test]
fn shutdown_then_exit_is_status_0() {
    let (client, _) = Client::initialized(init(json!({})));
    let outcome = client.exit(true);
    assert_eq!(outcome.exit_code(), 0);
}

#[test]
fn exit_without_shutdown_is_status_1() {
    let (client, _) = Client::initialized(init(json!({})));
    let outcome = client.exit(false);
    assert_eq!(
        outcome,
        Outcome::Exited {
            after_shutdown: false
        }
    );
    assert_eq!(outcome.exit_code(), 1);
}

#[test]
fn a_request_before_initialize_is_not_initialized() {
    let mut client = Client::start(test_options());
    let response = client.request("textDocument/codeAction", json!({}));
    assert_error(&response, ErrorCode::ServerNotInitialized);
    // A notification before initialize is dropped, not fatal.
    client.notify("textDocument/didOpen", json!({}));
    let response = client.request("initialize", init(json!({})));
    assert!(response.response_result.is_ok());
    client.notify("initialized", json!({}));
    still_answers(&mut client);
}

#[test]
fn a_second_initialize_is_invalid() {
    let (mut client, _) = Client::initialized(init(json!({})));
    let response = client.request("initialize", init(json!({})));
    assert_error(&response, ErrorCode::InvalidRequest);
    still_answers(&mut client);
}

#[test]
fn a_request_after_shutdown_is_invalid() {
    let (mut client, _) = Client::initialized(init(json!({})));
    assert!(client
        .request("shutdown", Value::Null)
        .response_result
        .is_ok());
    let response = client.request("textDocument/codeAction", json!({}));
    assert_error(&response, ErrorCode::InvalidRequest);
    client.notify("exit", Value::Null);
    assert_eq!(client.join().exit_code(), 0);
}

#[test]
fn an_unknown_request_is_method_not_found() {
    let (mut client, _) = Client::initialized(init(json!({})));
    let response = client.request("textDocument/formatting", json!({}));
    assert_error(&response, ErrorCode::MethodNotFound);
    still_answers(&mut client);
}

#[test]
fn bad_params_are_invalid_params() {
    let (mut client, _) = Client::initialized(init(json!({})));
    for params in [
        json!({}),
        json!(null),
        json!([1, 2]),
        json!({ "textDocument": 3 }),
    ] {
        let response = client.request("textDocument/codeAction", params);
        assert_error(&response, ErrorCode::InvalidParams);
    }
    still_answers(&mut client);
}

#[test]
fn unknown_and_malformed_notifications_are_ignored() {
    let (mut client, _) = Client::initialized(init(json!({})));
    client.notify("mesh/somethingNew", json!({ "x": 1 }));
    client.notify("textDocument/didOpen", json!({ "textDocument": 7 }));
    client.notify("textDocument/didChange", json!(null));
    client.notify("textDocument/didClose", json!("x"));
    client.notify("workspace/didChangeWatchedFiles", json!(5));
    client.notify("workspace/didChangeConfiguration", json!([]));
    client.notify("$/cancelRequest", json!({ "id": 99 }));
    still_answers(&mut client);
}

#[test]
fn documents_that_arent_open_are_ignored() {
    let (mut client, _) = Client::initialized(init(json!({})));
    client.change("file:///never-opened.mprx", 2, "<a />");
    client.close("file:///never-opened.mprx");
    still_answers(&mut client);
}

#[test]
fn a_panicking_handler_is_an_internal_error_and_the_server_keeps_running() {
    let options = Options {
        test_hooks: true,
        ..test_options()
    };
    let (mut client, _) = Client::initialized_with(options, init(json!({})));
    let response = client.request("mesh/panicForTest", Value::Null);
    assert_error(&response, ErrorCode::InternalError);
    still_answers(&mut client);
    // Without the test hooks, the request doesn't exist.
    let (mut client, _) = Client::initialized(init(json!({})));
    let response = client.request("mesh/panicForTest", Value::Null);
    assert_error(&response, ErrorCode::MethodNotFound);
}

#[test]
fn positions_outside_a_document_are_clamped() {
    let (mut client, _) = Client::initialized(init(json!({})));
    let uri = "file:///p.mprx";
    client.open(uri, 1, "<a>😀</a>");
    client.next_publication(uri);
    for (line, character) in [(0, 99), (99, 0), (0, 5), (4294967295u32, 4294967295u32)] {
        let response = client.request(
            "textDocument/codeAction",
            json!({
                "textDocument": { "uri": uri },
                "range": {
                    "start": { "line": line, "character": character },
                    "end": { "line": line, "character": character }
                },
                "context": { "diagnostics": [] }
            }),
        );
        assert_eq!(response.response_result.ok(), Some(json!([])));
    }
}
