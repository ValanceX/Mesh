//! A test client for the server, over an in-memory connection. Not part
//! of the supported API: it exists so `mesh-lsp`'s tests and `mesh-cli`'s
//! agreement test share one harness.
//!
//! Every wait has a timeout, so a hung server fails a test instead of
//! hanging CI.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crate::{run, Options, Outcome};
use lsp_server::{Connection, Message, Notification, Request, RequestId, Response};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::path::Path;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How long any single wait lasts before a test fails.
pub const WAIT: Duration = Duration::from_secs(20);

/// The `file:` URI of `path`, as the server writes it.
pub fn file_uri(path: &Path) -> String {
    crate::uri::from_path(path)
}

/// One `textDocument/publishDiagnostics`.
#[derive(Debug, Clone, PartialEq)]
pub struct Publication {
    pub version: Option<i32>,
    pub diagnostics: Vec<lsp_types::Diagnostic>,
}

/// A client driving one server thread.
pub struct Client {
    connection: Connection,
    server: Option<JoinHandle<Outcome>>,
    next_id: i32,
    /// Messages received but not yet asked for.
    inbox: VecDeque<Message>,
    /// The answer to `workspace/configuration` (the `"mesh"` section).
    pub configuration: Value,
    /// Every `client/registerCapability` the server sent, in order.
    pub registrations: Vec<Value>,
    logs: Vec<String>,
}

impl Client {
    /// Starts a server with `options` on its own thread.
    pub fn start(options: Options) -> Client {
        let (server_side, client_side) = Connection::memory();
        let server = std::thread::spawn(move || run(server_side, options));
        Client {
            connection: client_side,
            server: Some(server),
            next_id: 0,
            inbox: VecDeque::new(),
            configuration: Value::Null,
            registrations: Vec::new(),
            logs: Vec::new(),
        }
    }

    /// Starts a server with test options (no debounce) and initializes it
    /// with `params`, then sends `initialized`. Returns the result.
    pub fn initialized(params: Value) -> (Client, Value) {
        Client::initialized_with(test_options(), params)
    }

    pub fn initialized_with(options: Options, params: Value) -> (Client, Value) {
        let mut client = Client::start(options);
        let response = client.request("initialize", params);
        let result = response.response_result.expect("initialize succeeds");
        client.notify("initialized", json!({}));
        (client, result)
    }

    pub fn notify(&self, method: &str, params: Value) {
        self.connection
            .sender
            .send(Notification::new(method.to_string(), params).into())
            .expect("the server is listening");
    }

    /// Sends a request and waits for its response.
    pub fn request(&mut self, method: &str, params: Value) -> Response {
        let id = self.send(method, params);
        self.response(&id, WAIT)
            .unwrap_or_else(|| panic!("no response to {method} within {WAIT:?}"))
    }

    /// Sends a request without waiting for its response; read it later
    /// with [`Client::response`].
    pub fn send(&mut self, method: &str, params: Value) -> RequestId {
        self.next_id += 1;
        let id = RequestId::from(self.next_id);
        self.connection
            .sender
            .send(Request::new(id.clone(), method.to_string(), params).into())
            .expect("the server is listening");
        id
    }

    /// The response to request `id`, if it comes within `within`.
    pub fn response(&mut self, id: &RequestId, within: Duration) -> Option<Response> {
        let is_it = |message: &Message| matches!(message, Message::Response(r) if &r.id == id);
        if let Some(index) = self.inbox.iter().position(is_it) {
            match self.inbox.remove(index) {
                Some(Message::Response(response)) => return Some(response),
                _ => unreachable!("the position found a response"),
            }
        }
        let deadline = Instant::now() + within;
        loop {
            match self.receive(deadline) {
                Some(Message::Response(response)) if &response.id == id => return Some(response),
                Some(other) => self.inbox.push_back(other),
                None => return None,
            }
        }
    }

    /// The result of `textDocument/hover` at `line`/`character` (in the
    /// negotiated unit), or `Value::Null`.
    pub fn hover(&mut self, uri: &str, line: u32, character: u32) -> Value {
        self.position_request("textDocument/hover", uri, line, character)
    }

    /// The result of `textDocument/definition`, or `Value::Null`.
    pub fn definition(&mut self, uri: &str, line: u32, character: u32) -> Value {
        self.position_request("textDocument/definition", uri, line, character)
    }

    /// The result of `textDocument/completion`.
    pub fn completion(&mut self, uri: &str, line: u32, character: u32) -> Value {
        self.position_request("textDocument/completion", uri, line, character)
    }

    fn position_request(&mut self, method: &str, uri: &str, line: u32, character: u32) -> Value {
        let response = self.request(method, position_params(uri, line, character));
        response
            .response_result
            .unwrap_or_else(|error| panic!("{method} failed: {error:?}"))
    }

    pub fn open(&self, uri: &str, version: i32, text: &str) {
        self.notify(
            "textDocument/didOpen",
            json!({ "textDocument": {
                "uri": uri, "languageId": "mprx", "version": version, "text": text
            } }),
        );
    }

    pub fn change(&self, uri: &str, version: i32, text: &str) {
        self.notify(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": uri, "version": version },
                "contentChanges": [{ "text": text }]
            }),
        );
    }

    pub fn close(&self, uri: &str) {
        self.notify(
            "textDocument/didClose",
            json!({ "textDocument": { "uri": uri } }),
        );
    }

    /// The next diagnostics published for `uri`, waiting if need be.
    pub fn next_publication(&mut self, uri: &str) -> Publication {
        self.try_publication(uri, WAIT)
            .unwrap_or_else(|| panic!("no diagnostics published for {uri} within {WAIT:?}"))
    }

    /// The next diagnostics published for `uri` within `within`, if any.
    pub fn try_publication(&mut self, uri: &str, within: Duration) -> Option<Publication> {
        if let Some(index) = self
            .inbox
            .iter()
            .position(|message| is_publication(message, uri))
        {
            return self.inbox.remove(index).map(publication);
        }
        let deadline = Instant::now() + within;
        loop {
            match self.receive(deadline) {
                Some(message) if is_publication(&message, uri) => {
                    return Some(publication(message))
                }
                Some(other) => self.inbox.push_back(other),
                None => return None,
            }
        }
    }

    /// The last diagnostics published for `uri` once publications stop:
    /// waits for one, then takes any that follow within `settle`.
    pub fn latest_publication(&mut self, uri: &str, settle: Duration) -> Publication {
        let mut latest = self.next_publication(uri);
        while let Some(next) = self.try_publication(uri, settle) {
            latest = next;
        }
        latest
    }

    /// Asserts that nothing is published for `uri` within `within`.
    pub fn no_publication(&mut self, uri: &str, within: Duration) {
        if let Some(publication) = self.try_publication(uri, within) {
            panic!("unexpected publication for {uri}: {publication:#?}");
        }
    }

    /// Every `window/logMessage` received so far, and any that arrive
    /// within `within`.
    pub fn logs(&mut self, within: Duration) -> Vec<String> {
        let deadline = Instant::now() + within;
        while let Some(message) = self.receive(deadline) {
            self.inbox.push_back(message);
        }
        self.logs.clone()
    }

    /// Sends `shutdown` (if `shutdown`) and `exit`, and returns how the
    /// server ended.
    pub fn exit(mut self, shutdown: bool) -> Outcome {
        if shutdown {
            let response = self.request("shutdown", Value::Null);
            assert!(response.response_result.is_ok(), "{response:?}");
        }
        self.notify("exit", Value::Null);
        self.join()
    }

    /// Waits for the server thread to end, and returns how it ended.
    pub fn join(mut self) -> Outcome {
        let server = self.server.take().expect("the server thread exists");
        let deadline = Instant::now() + WAIT;
        while !server.is_finished() {
            assert!(
                Instant::now() < deadline,
                "the server didn't stop within {WAIT:?}"
            );
            // Keep draining, so the server never blocks sending to us.
            let _ = self
                .connection
                .receiver
                .recv_timeout(Duration::from_millis(10));
        }
        server.join().expect("the server thread doesn't panic")
    }

    /// The next message the tests should see. Answers the server's own
    /// requests, and records logs, on the way.
    fn receive(&mut self, deadline: Instant) -> Option<Message> {
        loop {
            let timeout = deadline.saturating_duration_since(Instant::now());
            let message = self.connection.receiver.recv_timeout(timeout).ok()?;
            match message {
                Message::Request(request) => self.answer(request),
                Message::Notification(notification)
                    if notification.method == "window/logMessage" =>
                {
                    let text = notification.params["message"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                    self.logs.push(text);
                }
                other => return Some(other),
            }
        }
    }

    fn answer(&mut self, request: Request) {
        let result = match request.method.as_str() {
            "workspace/configuration" => json!([self.configuration]),
            "client/registerCapability" => {
                self.registrations.push(request.params.clone());
                Value::Null
            }
            _ => Value::Null,
        };
        let _ = self
            .connection
            .sender
            .send(Response::new_ok(request.id, result).into());
    }
}

/// The parameters of a request at a position in a document.
pub fn position_params(uri: &str, line: u32, character: u32) -> Value {
    json!({
        "textDocument": { "uri": uri },
        "position": { "line": line, "character": character }
    })
}

/// Options for tests: compile on every change, with no debounce.
pub fn test_options() -> Options {
    Options {
        debounce: Duration::ZERO,
        ..Options::default()
    }
}

fn is_publication(message: &Message, uri: &str) -> bool {
    matches!(message, Message::Notification(notification)
        if notification.method == "textDocument/publishDiagnostics"
            && notification.params["uri"].as_str() == Some(uri))
}

fn publication(message: Message) -> Publication {
    let Message::Notification(notification) = message else {
        panic!("not a publication: {message:?}");
    };
    Publication {
        version: notification.params["version"]
            .as_i64()
            .map(|version| i32::try_from(version).expect("a version fits i32")),
        diagnostics: serde_json::from_value(notification.params["diagnostics"].clone())
            .expect("diagnostics decode"),
    }
}
