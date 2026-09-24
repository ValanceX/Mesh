//! The crash-safety cases of outline D7 that the other suites don't
//! reach (audit C10): positions inside a surrogate pair, a manifest that
//! isn't UTF-8, the nesting stress inputs of findings A1 and A2, and empty
//! and whitespace-only documents. Each has D7's outcome, and the server
//! keeps answering.

mod common;

use common::{codes, Workspace};
use mesh_lsp::testing::{position_params, Client};
use serde_json::{json, Value};
use std::time::Duration;

const QUIET: Duration = Duration::from_millis(300);

fn settings() -> Value {
    json!({ "model": "components.json", "components": { "page.mprx": "users-page" } })
}

fn configured() -> (Workspace, Client, String) {
    let workspace = Workspace::with_example_manifest();
    let (client, _) = Client::initialized(workspace.init(settings(), json!({})));
    let uri = workspace.uri("page.mprx");
    (workspace, client, uri)
}

/// Hover, definition and completion at `line`/`character` each answer
/// with a result, not an error.
fn all_answer(client: &mut Client, uri: &str, line: u32, character: u32) {
    for method in [
        "textDocument/hover",
        "textDocument/definition",
        "textDocument/completion",
    ] {
        let response = client.request(method, position_params(uri, line, character));
        assert!(
            response.response_result.is_ok(),
            "{method} at {line}:{character}: {response:?}"
        );
    }
}

#[test]
fn positions_inside_a_surrogate_pair_are_answered() {
    let (_workspace, mut client, uri) = configured();
    // In UTF-16, the emoji is columns 13 and 14: 14 is inside it.
    let text = r#"<page title="😀">{user.name}</page>"#;
    client.open(&uri, 1, text);
    assert!(client.next_publication(&uri).diagnostics.is_empty());
    all_answer(&mut client, &uri, 0, 14);
    // The same inside a broken file, where completion reads the tokens.
    client.change(&uri, 2, r#"<page title="😀">{user.}</page>"#);
    client.next_publication(&uri);
    all_answer(&mut client, &uri, 0, 14);
    // Hover right after the emoji still finds `user`.
    client.change(&uri, 3, text);
    client.next_publication(&uri);
    let hover = client.hover(&uri, 0, 19);
    assert!(hover["contents"].to_string().contains("User"), "{hover}");
}

#[test]
fn a_manifest_that_isnt_utf8_is_logged_and_unused() {
    let workspace = Workspace::empty();
    std::fs::write(workspace.path("components.json"), [0xff, 0xfe, b'{', b'}'])
        .expect("the file writes");
    let (mut client, _) = Client::initialized(workspace.init(settings(), json!({})));
    let logs = client.logs(QUIET);
    assert!(
        logs.iter()
            .any(|log| log.contains("couldn't read the manifest")),
        "{logs:#?}"
    );
    let uri = workspace.uri("page.mprx");
    client.open(&uri, 1, r#"<page title={usr.name} />"#);
    assert!(client.next_publication(&uri).diagnostics.is_empty());
    all_answer(&mut client, &uri, 0, 14);
}

/// Findings A1's shapes, far deeper than the stack could once take.
fn deep_documents() -> Vec<(&'static str, String)> {
    const DEEP: usize = 20_000;
    vec![
        (
            "elements",
            format!("{}{}", "<a>".repeat(DEEP), "</a>".repeat(DEEP)),
        ),
        (
            "arrays",
            format!("<a b={{{}{}}} />", "[".repeat(DEEP), "]".repeat(DEEP)),
        ),
        (
            "parentheses",
            format!("<a b={{{}x{}}} />", "(".repeat(DEEP), ")".repeat(DEEP)),
        ),
        ("unary", format!("<a b={{{}x}} />", "!".repeat(50_000))),
        (
            "members",
            format!("<a b={{user{}}} />", ".name".repeat(DEEP)),
        ),
        // Unclosed, as while typing: recovery and completion see it.
        ("unclosed", format!("<page>{}", "<a>{[(".repeat(DEEP))),
    ]
}

#[test]
fn deeply_nested_documents_are_diagnosed_not_crashed() {
    let (_workspace, mut client, uri) = configured();
    for (version, (shape, text)) in deep_documents().into_iter().enumerate() {
        let version = i32::try_from(version).expect("a small version");
        if version == 0 {
            client.open(&uri, version, &text);
        } else {
            client.change(&uri, version, &text);
        }
        let published = client.next_publication(&uri);
        assert_eq!(published.version, Some(version), "{shape}");
        let expected: Vec<String> = mesh_compiler::compile(&text)
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.to_string())
            .collect();
        if shape != "members" && shape != "unclosed" {
            assert!(
                expected.iter().any(|code| code == "nesting-too-deep"),
                "{shape}: {expected:?}"
            );
        }
        // The model can only add to what the model-less compile says.
        let published = codes(&published.diagnostics);
        for code in &expected {
            assert!(published.contains(code), "{shape}: {published:?}");
        }
        let last_line = u32::try_from(text.lines().count().saturating_sub(1)).expect("fits");
        for (line, character) in [(0, 0), (0, 3), (0, 40_000), (last_line, u32::MAX)] {
            all_answer(&mut client, &uri, line, character);
        }
    }
}

#[test]
fn a_deeply_nested_manifest_is_diagnosed_not_crashed() {
    let workspace = Workspace::empty();
    let deep = 50_000;
    let manifest = format!(
        r#"{{ "version": 1, "types": {{ "Deep": {}{{ "kind": "string" }}{} }} }}"#,
        r#"{ "kind": "list", "element": "#.repeat(deep),
        "}".repeat(deep)
    );
    workspace.write("components.json", &manifest);
    let (mut client, _) = Client::initialized(workspace.init(settings(), json!({})));
    let manifest_uri = workspace.uri("components.json");
    assert_eq!(
        codes(&client.latest_publication(&manifest_uri, QUIET).diagnostics),
        ["manifest-nesting-too-deep"]
    );
    // Open in the editor too, then edited.
    client.open_as(&manifest_uri, "json", 1, &manifest);
    // A leading line moves the error, so it's published again.
    client.change(&manifest_uri, 2, &format!("\n{manifest}"));
    let published = client.latest_publication(&manifest_uri, QUIET);
    assert_eq!(codes(&published.diagnostics), ["manifest-nesting-too-deep"]);
    assert_eq!(published.diagnostics[0].range.start.line, 1);
    // Documents are checked without a model meanwhile.
    let uri = workspace.uri("page.mprx");
    client.open(&uri, 1, r#"<page title={usr.name} />"#);
    assert!(client.next_publication(&uri).diagnostics.is_empty());
    all_answer(&mut client, &uri, 0, 14);
}

#[test]
fn empty_and_whitespace_only_documents_get_the_compilers_diagnostics() {
    let (_workspace, mut client, uri) = configured();
    for (version, text) in ["", " ", "\n\n", " \t\r\n  \n"].into_iter().enumerate() {
        let version = i32::try_from(version).expect("a small version");
        if version == 0 {
            client.open(&uri, version, text);
        } else {
            client.change(&uri, version, text);
        }
        let published = client.next_publication(&uri);
        let expected: Vec<String> = mesh_compiler::compile(text)
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.to_string())
            .collect();
        assert!(!expected.is_empty(), "{text:?}");
        assert_eq!(codes(&published.diagnostics), expected, "{text:?}");
        for (line, character) in [(0, 0), (1, 0), (3, 7)] {
            all_answer(&mut client, &uri, line, character);
        }
    }
}
