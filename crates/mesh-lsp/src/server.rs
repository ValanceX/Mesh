//! The message loop. It owns all state, and it's the only code that
//! publishes anything.
//!
//! State is kept as immutable snapshots (outline D10): each document's
//! text and version, the manifest snapshot (D9) and the configuration
//! (D6), each with a generation. A compile runs on the worker thread for
//! one snapshot identity, and its result is published only if that
//! identity is still current. Anything else is dropped, so an older
//! result never replaces a newer one.

mod complete;
mod documents;
mod manifest;
mod navigate;
mod requests;

use crate::config::Config;
use crate::convert::{self, Text};
use crate::worker::{self, Finished, Worker};
use crate::{uri, Options, Outcome};
use crossbeam_channel::{after, never, select, Receiver, Sender};
use documents::Document;
use lsp_server::{ErrorCode, Message, Notification, Request, RequestId, Response};
use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, CompletionParams,
    DocumentChanges, GotoDefinitionParams, HoverParams, MessageType, OneOf,
    OptionalVersionedTextDocumentIdentifier, TextDocumentEdit, TextEdit, WorkspaceEdit,
};
use manifest::ManifestState;
use mesh_compiler::{ColumnUnit, SourceMap};
use requests::{Held, Query};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;
use std::time::Instant;

/// The client has gone: a send failed, so nothing more can be said.
pub(crate) struct Disconnected;

type Sent = Result<(), Disconnected>;

/// What the loop does after a message.
enum Flow {
    Continue,
    Exit,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Protocol {
    /// Before `initialize`.
    Uninitialized,
    Running,
    /// After `shutdown`.
    ShutDown,
}

/// A request the server sent the client, awaiting its response.
enum Outgoing {
    Configuration,
    Registration,
}

/// What the client said it supports.
#[derive(Default)]
struct Client {
    /// `workspace/configuration` requests.
    configuration: bool,
    /// Registering `workspace/didChangeWatchedFiles` dynamically.
    watch: bool,
    /// Markdown in hovers.
    markdown: bool,
}

struct Server {
    sender: Sender<Message>,
    options: Options,
    protocol: Protocol,
    unit: ColumnUnit,
    client: Client,
    root: Option<PathBuf>,
    config: Config,
    config_generation: u64,
    manifest: ManifestState,
    documents: BTreeMap<String, Document>,
    next_generation: u64,
    /// Documents whose compile waits out the debounce, and until when.
    due: BTreeMap<String, Instant>,
    worker: Worker,
    next_request: i32,
    outgoing: HashMap<RequestId, Outgoing>,
    /// The manifest URI and diagnostics last published on it.
    manifest_published: Option<(String, Vec<lsp_types::Diagnostic>)>,
    watcher: Option<String>,
    /// Documents already told they have no configured component.
    unconfigured_logged: BTreeSet<String>,
    /// Logs held back until `initialized`.
    pending_logs: Vec<(MessageType, String)>,
    /// Requests waiting for their snapshot to be compiled.
    held: Vec<Held>,
    /// Completions the compile thread is working on.
    completing: Vec<RequestId>,
}

pub(crate) fn run(connection: lsp_server::Connection, options: Options) -> Outcome {
    let worker = match worker::spawn(options.compile_gate.clone()) {
        Ok(worker) => worker,
        Err(err) => return Outcome::Failed(format!("couldn't start the compile thread: {err}")),
    };
    let results = worker.results.clone();
    let receiver: Receiver<Message> = connection.receiver.clone();
    let mut server = Server {
        sender: connection.sender.clone(),
        options,
        protocol: Protocol::Uninitialized,
        unit: ColumnUnit::Utf16,
        client: Client::default(),
        root: None,
        config: Config::default(),
        config_generation: 0,
        manifest: ManifestState::default(),
        documents: BTreeMap::new(),
        next_generation: 0,
        due: BTreeMap::new(),
        worker,
        next_request: 0,
        outgoing: HashMap::new(),
        manifest_published: None,
        watcher: None,
        unconfigured_logged: BTreeSet::new(),
        pending_logs: Vec::new(),
        held: Vec::new(),
        completing: Vec::new(),
    };
    drop(connection);

    loop {
        let timer = match server.due.values().min() {
            Some(deadline) => after(deadline.saturating_duration_since(Instant::now())),
            None => never(),
        };
        let step = select! {
            recv(receiver) -> message => match message {
                Ok(message) => server.handle(message),
                // The transport has stopped: stdin closed, or a message
                // it couldn't read.
                Err(_) => return Outcome::Disconnected,
            },
            recv(results) -> done => match done {
                Ok(Finished::Compiled(done)) => server.finish(done).map(|()| Flow::Continue),
                Ok(Finished::Completed(done)) => {
                    server.finish_completion(done).map(|()| Flow::Continue)
                }
                Err(_) => Ok(Flow::Continue),
            },
            recv(timer) -> _ => server.compile_due().map(|()| Flow::Continue),
        };
        match step {
            Ok(Flow::Continue) => {}
            Ok(Flow::Exit) => {
                return Outcome::Exited {
                    after_shutdown: server.protocol == Protocol::ShutDown,
                }
            }
            Err(Disconnected) => return Outcome::Disconnected,
        }
    }
}

impl Server {
    // --- Sending --------------------------------------------------------

    fn send(&self, message: Message) -> Sent {
        self.sender.send(message).map_err(|_| Disconnected)
    }

    fn respond(&self, id: RequestId, result: Value) -> Sent {
        self.send(Response::new_ok(id, result).into())
    }

    fn respond_error(&self, id: RequestId, code: ErrorCode, message: String) -> Sent {
        self.send(Response::new_err(id, code as i32, message).into())
    }

    fn notify(&self, method: &str, params: impl serde::Serialize) -> Sent {
        self.send(Notification::new(method.to_string(), params).into())
    }

    fn request_client(&mut self, method: &str, params: Value, purpose: Outgoing) -> Sent {
        self.next_request += 1;
        let id = RequestId::from(format!("mesh-{}", self.next_request));
        self.outgoing.insert(id.clone(), purpose);
        self.send(Request::new(id, method.to_string(), params).into())
    }

    /// Logs through `window/logMessage`, or holds the message back until
    /// `initialized`, before which the client may not be listening.
    fn log(&mut self, kind: MessageType, message: String) -> Sent {
        if self.protocol == Protocol::Uninitialized {
            self.pending_logs.push((kind, message));
            return Ok(());
        }
        self.notify(
            "window/logMessage",
            json!({ "type": kind, "message": message }),
        )
    }

    // --- Dispatch -------------------------------------------------------

    fn handle(&mut self, message: Message) -> Result<Flow, Disconnected> {
        match message {
            Message::Request(request) => self.request(request),
            Message::Notification(notification) => self.notification(notification),
            Message::Response(response) => self.response(response).map(|()| Flow::Continue),
        }
    }

    /// Answers `request`. A handler that panics is answered with
    /// `InternalError`, and the server keeps running (outline D7). That's a
    /// backstop: any such panic is a bug.
    fn request(&mut self, request: Request) -> Result<Flow, Disconnected> {
        let id = request.id.clone();
        let method = request.method.clone();
        match panic::catch_unwind(AssertUnwindSafe(|| self.dispatch_request(request))) {
            Ok(flow) => flow,
            Err(payload) => {
                let message = worker::panic_message(payload.as_ref());
                self.log(
                    MessageType::ERROR,
                    format!("mesh-lsp: internal error answering {method}: {message}"),
                )?;
                self.respond_error(
                    id,
                    ErrorCode::InternalError,
                    format!("internal error: {message}"),
                )?;
                Ok(Flow::Continue)
            }
        }
    }

    fn dispatch_request(&mut self, request: Request) -> Result<Flow, Disconnected> {
        let Request { id, method, params } = request;
        match (self.protocol, method.as_str()) {
            (Protocol::Uninitialized, "initialize") => self.initialize(id, &params)?,
            (Protocol::Uninitialized, _) => self.respond_error(
                id,
                ErrorCode::ServerNotInitialized,
                format!("{method} before initialize"),
            )?,
            (Protocol::ShutDown, _) => self.respond_error(
                id,
                ErrorCode::InvalidRequest,
                format!("{method} after shutdown"),
            )?,
            (Protocol::Running, "initialize") => self.respond_error(
                id,
                ErrorCode::InvalidRequest,
                "the server is already initialized".to_string(),
            )?,
            (Protocol::Running, "shutdown") => {
                self.drain_held()?;
                self.protocol = Protocol::ShutDown;
                self.respond(id, Value::Null)?;
            }
            (Protocol::Running, "textDocument/codeAction") => {
                match decode::<CodeActionParams>(&method, params) {
                    Ok(params) => {
                        let actions = self.code_actions(&params);
                        self.respond(id, json!(actions))?;
                    }
                    Err(message) => self.respond_error(id, ErrorCode::InvalidParams, message)?,
                }
            }
            (Protocol::Running, "textDocument/hover") => {
                match decode::<HoverParams>(&method, params) {
                    Ok(params) => {
                        let position = params.text_document_position_params;
                        let document = position.text_document.uri.as_str().to_string();
                        self.answer_or_hold(id, &document, Query::Hover(position.position))?;
                    }
                    Err(message) => self.respond_error(id, ErrorCode::InvalidParams, message)?,
                }
            }
            (Protocol::Running, "textDocument/definition") => {
                match decode::<GotoDefinitionParams>(&method, params) {
                    Ok(params) => {
                        let position = params.text_document_position_params;
                        let document = position.text_document.uri.as_str().to_string();
                        self.answer_or_hold(id, &document, Query::Definition(position.position))?;
                    }
                    Err(message) => self.respond_error(id, ErrorCode::InvalidParams, message)?,
                }
            }
            (Protocol::Running, "textDocument/completion") => {
                match decode::<CompletionParams>(&method, params) {
                    Ok(params) => {
                        let position = params.text_document_position;
                        let document = position.text_document.uri.as_str().to_string();
                        self.answer_or_hold(id, &document, Query::Completion(position.position))?;
                    }
                    Err(message) => self.respond_error(id, ErrorCode::InvalidParams, message)?,
                }
            }
            (Protocol::Running, "mesh/panicForTest") if self.options.test_hooks => {
                panic!("mesh/panicForTest asked for a panic")
            }
            (Protocol::Running, _) => self.respond_error(
                id,
                ErrorCode::MethodNotFound,
                format!("mesh-lsp doesn't handle {method}"),
            )?,
        }
        Ok(Flow::Continue)
    }

    fn notification(&mut self, notification: Notification) -> Result<Flow, Disconnected> {
        let method = notification.method.clone();
        if method == "exit" {
            return Ok(Flow::Exit);
        }
        if self.protocol != Protocol::Running {
            return Ok(Flow::Continue);
        }
        match panic::catch_unwind(AssertUnwindSafe(|| {
            self.dispatch_notification(notification)
        })) {
            Ok(result) => result?,
            Err(payload) => {
                let message = worker::panic_message(payload.as_ref());
                self.log(
                    MessageType::ERROR,
                    format!("mesh-lsp: internal error handling {method}: {message}"),
                )?;
            }
        }
        Ok(Flow::Continue)
    }

    fn dispatch_notification(&mut self, notification: Notification) -> Sent {
        let Notification { method, params } = notification;
        match method.as_str() {
            "initialized" => self.initialized(),
            "textDocument/didOpen" => match decode(&method, params) {
                Ok(params) => self.did_open(params),
                Err(message) => self.log(MessageType::WARNING, message),
            },
            "textDocument/didChange" => match decode(&method, params) {
                Ok(params) => self.did_change(params),
                Err(message) => self.log(MessageType::WARNING, message),
            },
            "textDocument/didClose" => match decode(&method, params) {
                Ok(params) => self.did_close(params),
                Err(message) => self.log(MessageType::WARNING, message),
            },
            "workspace/didChangeConfiguration" => self.did_change_configuration(&params),
            "workspace/didChangeWatchedFiles" => self.did_change_watched_files(&params),
            // Only a request waiting for its snapshot can be cancelled;
            // every other one is answered at once.
            "$/cancelRequest" => self.cancel(&params),
            "textDocument/didSave" | "$/setTrace" => Ok(()),
            _ => {
                eprintln!("mesh-lsp: ignoring the notification {method}");
                Ok(())
            }
        }
    }

    fn response(&mut self, response: Response) -> Sent {
        match self.outgoing.remove(&response.id) {
            Some(Outgoing::Configuration) => match response.response_result {
                // One item was asked for: section "mesh".
                Ok(result) => {
                    let settings = result.get(0).cloned().unwrap_or(Value::Null);
                    self.apply_settings(&settings)
                }
                Err(error) => self.log(
                    MessageType::WARNING,
                    format!("the client couldn't send the configuration: {}", error.message),
                ),
            },
            Some(Outgoing::Registration) => match response.response_result {
                Ok(_) => Ok(()),
                Err(error) => self.log(
                    MessageType::WARNING,
                    format!(
                        "the client couldn't watch the manifest: {}; changes on disk are seen on reopen",
                        error.message
                    ),
                ),
            },
            None => Ok(()),
        }
    }

    // --- Lifecycle ------------------------------------------------------

    fn initialize(&mut self, id: RequestId, params: &Value) -> Sent {
        let encodings: Vec<&str> = params
            .pointer("/capabilities/general/positionEncodings")
            .and_then(Value::as_array)
            .map(|encodings| encodings.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let encoding = ["utf-8", "utf-32"]
            .into_iter()
            .find(|preferred| encodings.contains(preferred))
            .unwrap_or("utf-16");
        self.unit = convert::unit(encoding);

        let flag = |pointer: &str| params.pointer(pointer).and_then(Value::as_bool) == Some(true);
        self.client = Client {
            configuration: flag("/capabilities/workspace/configuration"),
            watch: flag("/capabilities/workspace/didChangeWatchedFiles/dynamicRegistration"),
            markdown: params
                .pointer("/capabilities/textDocument/hover/contentFormat")
                .and_then(Value::as_array)
                .is_some_and(|formats| formats.iter().any(|format| format == "markdown")),
        };

        // Multi-root is a non-goal: the first folder, or `rootUri`.
        self.root = params
            .pointer("/workspaceFolders/0/uri")
            .or_else(|| params.get("rootUri"))
            .and_then(Value::as_str)
            .and_then(uri::to_path);

        let (config, warnings) =
            Config::parse(params.get("initializationOptions").unwrap_or(&Value::Null));
        self.config = config;
        for warning in warnings {
            self.log(MessageType::WARNING, warning)?;
        }

        self.respond(
            id,
            json!({
                "capabilities": {
                    "positionEncoding": encoding,
                    "textDocumentSync": { "openClose": true, "change": 1 },
                    "codeActionProvider": { "codeActionKinds": ["quickfix"] },
                    "hoverProvider": true,
                    "definitionProvider": true,
                    "completionProvider": { "triggerCharacters": ["<", "."], "resolveProvider": false },
                    "workspace": { "workspaceFolders": { "supported": false } }
                },
                "serverInfo": { "name": "mesh-lsp", "version": env!("CARGO_PKG_VERSION") }
            }),
        )?;
        self.protocol = Protocol::Running;
        Ok(())
    }

    /// Flushes held-back logs, then loads the manifest (D9).
    fn initialized(&mut self) -> Sent {
        for (kind, message) in std::mem::take(&mut self.pending_logs) {
            self.log(kind, message)?;
        }
        self.configure()
    }

    fn did_change_configuration(&mut self, params: &Value) -> Sent {
        if self.client.configuration {
            return self.request_client(
                "workspace/configuration",
                json!({ "items": [{ "section": "mesh" }] }),
                Outgoing::Configuration,
            );
        }
        match params.pointer("/settings/mesh") {
            Some(settings) => self.apply_settings(&settings.clone()),
            // Settings for other tools only: nothing of ours changed.
            None => Ok(()),
        }
    }

    fn apply_settings(&mut self, settings: &Value) -> Sent {
        let (config, warnings) = Config::parse(settings);
        for warning in warnings {
            self.log(MessageType::WARNING, warning)?;
        }
        self.config = config;
        self.config_generation += 1;
        self.unconfigured_logged.clear();
        self.configure()
    }

    /// Resolves the manifest's path from the configuration, watches it,
    /// and loads it, which recompiles every open document.
    fn configure(&mut self) -> Sent {
        let path = match (&self.config.model, &self.root) {
            (Some(model), Some(root)) => uri::resolve(root, model),
            (Some(model), None) => {
                let model = model.clone();
                self.log(
                    MessageType::WARNING,
                    format!(
                        "no workspace root, so \"mesh.model\" ({model}) can't be found; checking without a model"
                    ),
                )?;
                None
            }
            (None, _) => None,
        };
        self.manifest.path = path;
        self.watch_manifest()?;
        self.reload_manifest()
    }

    fn watch_manifest(&mut self) -> Sent {
        if !self.client.watch {
            return Ok(());
        }
        if let Some(id) = self.watcher.take() {
            self.request_client(
                "client/unregisterCapability",
                json!({ "unregisterations": [{ "id": id, "method": "workspace/didChangeWatchedFiles" }] }),
                Outgoing::Registration,
            )?;
        }
        let Some(path) = &self.manifest.path else {
            return Ok(());
        };
        let id = format!("mesh-manifest-{}", self.config_generation);
        let pattern = path.to_string_lossy().into_owned();
        self.request_client(
            "client/registerCapability",
            json!({ "registrations": [{
                "id": id,
                "method": "workspace/didChangeWatchedFiles",
                "registerOptions": { "watchers": [{ "globPattern": pattern }] }
            }] }),
            Outgoing::Registration,
        )?;
        self.watcher = Some(id);
        Ok(())
    }

    fn did_change_watched_files(&mut self, params: &Value) -> Sent {
        let Some(path) = &self.manifest.path else {
            return Ok(());
        };
        let touched = params
            .get("changes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|change| change.get("uri").and_then(Value::as_str))
            .any(|changed| uri::to_path(changed).as_ref() == Some(path));
        // While the manifest is open, its buffer is the truth, not disk.
        if touched && self.manifest.open.is_none() {
            return self.reload_manifest();
        }
        Ok(())
    }

    // --- Quick fixes ----------------------------------------------------

    /// One quick fix per suggestion of each diagnostic the range touches,
    /// from the current snapshot only. While a newer version is waiting to
    /// be compiled, there are none: edits computed for older text could
    /// land in the wrong place.
    fn code_actions(&self, params: &CodeActionParams) -> Vec<CodeActionOrCommand> {
        let key = params.text_document.uri.as_str();
        let Some(document) = self.documents.get(key) else {
            return Vec::new();
        };
        let Some(compiled) = &document.compiled else {
            return Vec::new();
        };
        if compiled.identity != self.identity(document) || self.due.contains_key(key) {
            return Vec::new();
        }
        let text = Text {
            source: &compiled.text,
            map: &compiled.map,
            unit: self.unit,
        };
        let wanted = text.span(params.range);
        let mut actions = Vec::new();
        for diagnostic in &compiled.result.diagnostics {
            let span = diagnostic.span;
            let touches = span.start_byte <= wanted.end_byte && wanted.start_byte <= span.end_byte;
            if !touches {
                continue;
            }
            let only = diagnostic.suggestions.len() == 1;
            for suggestion in &diagnostic.suggestions {
                let edit = TextDocumentEdit {
                    text_document: OptionalVersionedTextDocumentIdentifier {
                        uri: document.uri.clone(),
                        version: Some(compiled.version),
                    },
                    edits: vec![OneOf::Left(TextEdit {
                        range: text.range(suggestion.span),
                        new_text: suggestion.replacement.clone(),
                    })],
                };
                actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                    title: format!("Replace with \"{}\"", suggestion.replacement),
                    kind: Some(CodeActionKind::QUICKFIX),
                    diagnostics: Some(vec![text.diagnostic(diagnostic)]),
                    edit: Some(WorkspaceEdit {
                        changes: None,
                        document_changes: Some(DocumentChanges::Edits(vec![edit])),
                        change_annotations: None,
                    }),
                    command: None,
                    is_preferred: Some(only),
                    disabled: None,
                    data: None,
                }));
            }
        }
        actions
    }
}

/// Decodes a message's parameters, or says why they don't decode.
fn decode<T: DeserializeOwned>(method: &str, params: Value) -> Result<T, String> {
    serde_json::from_value(params).map_err(|err| format!("invalid parameters for {method}: {err}"))
}

/// `text` after the client's content changes, in order. The server asks
/// for whole documents, but a change with a range is applied too.
fn apply_changes(
    text: &str,
    changes: &[lsp_types::TextDocumentContentChangeEvent],
    unit: ColumnUnit,
) -> String {
    let mut text = text.to_string();
    for change in changes {
        match change.range {
            None => text = change.text.clone(),
            Some(range) => {
                let map = SourceMap::new(&text);
                let span = Text {
                    source: &text,
                    map: &map,
                    unit,
                }
                .span(range);
                text.replace_range(span.start_byte..span.end_byte, &change.text);
            }
        }
    }
    text
}
