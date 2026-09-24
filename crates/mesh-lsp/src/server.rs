//! The message loop. It owns all state, and it's the only code that
//! publishes anything.
//!
//! State is kept as immutable snapshots (outline D10): each document's
//! text and version, the manifest snapshot (D9) and the configuration
//! (D6), each with a generation. A compile runs on the worker thread for
//! one snapshot identity, and its result is published only if that
//! identity is still current. Anything else is dropped, so an older
//! result never replaces a newer one.

use crate::config::Config;
use crate::convert::{self, Text};
use crate::worker::{self, Done, Identity, Job, Model, Worker};
use crate::{uri, Options, Outcome};
use crossbeam_channel::{after, never, select, Receiver, Sender};
use lsp_server::{ErrorCode, Message, Notification, Request, RequestId, Response};
use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DocumentChanges, MessageType, OneOf,
    OptionalVersionedTextDocumentIdentifier, PublishDiagnosticsParams, TextDocumentEdit, TextEdit,
    Uri, WorkspaceEdit,
};
use mesh_compiler::{ColumnUnit, SourceMap};
use mesh_manifest::Manifest;
use mesh_syntax::Diagnostic;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
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
}

/// An open document.
struct Document {
    uri: Uri,
    version: i32,
    text: Arc<str>,
    generation: u64,
    /// The component its template is, if the configuration names one the
    /// manifest doesn't declare. Reported on the manifest (D9).
    missing: Option<String>,
    /// The canonical state of the latest snapshot compiled and published.
    compiled: Option<Compiled>,
}

/// A snapshot's canonical state: what the compiler said about its text.
struct Compiled {
    identity: Identity,
    version: i32,
    text: Arc<str>,
    map: SourceMap,
    diagnostics: Vec<Diagnostic>,
}

/// One version of the manifest's text, loaded (D9).
struct ManifestSnapshot {
    uri: String,
    text: String,
    map: SourceMap,
    loaded: Result<Arc<Manifest>, Vec<Diagnostic>>,
}

#[derive(Default)]
struct ManifestState {
    generation: u64,
    /// Where the configuration says the manifest is.
    path: Option<PathBuf>,
    /// The client's URI and text while the manifest is open in it.
    open: Option<(String, String)>,
    /// `None` without a configured, readable manifest.
    snapshot: Option<Arc<ManifestSnapshot>>,
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
                Ok(done) => server.finish(done).map(|()| Flow::Continue),
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
            // Every request this server answers is answered at once, so
            // there is nothing to cancel.
            "$/cancelRequest" | "textDocument/didSave" | "$/setTrace" => Ok(()),
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

    // --- The manifest (D9) ----------------------------------------------

    fn is_manifest(&self, document: &str) -> bool {
        self.manifest.path.is_some() && uri::to_path(document) == self.manifest.path
    }

    /// Starts a new manifest generation from the open buffer or the disk,
    /// then recompiles every open document against it.
    fn reload_manifest(&mut self) -> Sent {
        self.manifest.generation += 1;
        self.manifest.snapshot = None;
        if let Some(path) = self.manifest.path.clone() {
            let (document_uri, text) = match &self.manifest.open {
                Some((open_uri, text)) => (open_uri.clone(), Ok(text.clone())),
                None => (uri::from_path(&path), fs::read_to_string(&path)),
            };
            match text {
                Ok(text) => {
                    let loaded = mesh_manifest::load(&text).map(Arc::new);
                    self.manifest.snapshot = Some(Arc::new(ManifestSnapshot {
                        uri: document_uri,
                        map: SourceMap::new(&text),
                        text,
                        loaded,
                    }));
                }
                Err(err) => self.log(
                    MessageType::WARNING,
                    format!(
                        "couldn't read the manifest {}: {err}; checking without a model",
                        path.display()
                    ),
                )?,
            }
        }
        let uris: Vec<String> = self.documents.keys().cloned().collect();
        for document in uris {
            self.compile(&document)?;
        }
        self.publish_manifest()
    }

    /// Publishes the manifest's diagnostics if they changed: its load
    /// errors while it's broken, or else one `manifest-missing-component`
    /// per component an open document is configured as and the manifest
    /// doesn't declare.
    fn publish_manifest(&mut self) -> Sent {
        let current = self.manifest.snapshot.as_ref().map(|snapshot| {
            let text = Text {
                source: &snapshot.text,
                map: &snapshot.map,
                unit: self.unit,
            };
            let diagnostics = match &snapshot.loaded {
                Err(diagnostics) => text.diagnostics(diagnostics),
                Ok(manifest) => {
                    let missing: BTreeSet<&str> = self
                        .documents
                        .values()
                        .filter_map(|document| document.missing.as_deref())
                        .collect();
                    let diagnostics: Vec<Diagnostic> = missing
                        .into_iter()
                        .filter_map(|component| manifest.template(component).err())
                        .collect();
                    text.diagnostics(&diagnostics)
                }
            };
            (snapshot.uri.clone(), diagnostics)
        });
        if current == self.manifest_published {
            return Ok(());
        }
        if let Some((old_uri, _)) = &self.manifest_published {
            if current.as_ref().map(|(uri, _)| uri) != Some(old_uri) {
                self.publish(old_uri, Vec::new(), None)?;
            }
        }
        if let Some((manifest_uri, diagnostics)) = &current {
            self.publish(manifest_uri, diagnostics.clone(), None)?;
        }
        self.manifest_published = current;
        Ok(())
    }

    // --- Documents (D10) ------------------------------------------------

    fn did_open(&mut self, params: DidOpenTextDocumentParams) -> Sent {
        let document = params.text_document;
        let key = document.uri.as_str().to_string();
        if self.is_manifest(&key) {
            self.manifest.open = Some((key, document.text));
            return self.reload_manifest();
        }
        self.next_generation += 1;
        self.documents.insert(
            key.clone(),
            Document {
                uri: document.uri,
                version: document.version,
                text: Arc::from(document.text),
                generation: self.next_generation,
                missing: None,
                compiled: None,
            },
        );
        self.compile(&key)?;
        self.publish_manifest()
    }

    fn did_change(&mut self, params: DidChangeTextDocumentParams) -> Sent {
        let key = params.text_document.uri.as_str().to_string();
        let version = params.text_document.version;

        if self.is_manifest(&key) {
            if let Some((_, text)) = &mut self.manifest.open {
                *text = apply_changes(text, &params.content_changes, self.unit);
                return self.reload_manifest();
            }
            return Ok(());
        }

        let Some(document) = self.documents.get_mut(&key) else {
            return self.log(
                MessageType::WARNING,
                format!("didChange for {key}, which isn't open; ignoring it"),
            );
        };
        if version < document.version {
            let current = document.version;
            return self.log(
                MessageType::WARNING,
                format!("didChange for {key} version {version}, older than {current}; ignoring it"),
            );
        }
        let text = apply_changes(&document.text, &params.content_changes, self.unit);
        self.next_generation += 1;
        document.text = Arc::from(text);
        document.version = version;
        document.generation = self.next_generation;

        if self.options.debounce.is_zero() {
            self.compile(&key)
        } else {
            self.due.insert(key, Instant::now() + self.options.debounce);
            Ok(())
        }
    }

    fn did_close(&mut self, params: DidCloseTextDocumentParams) -> Sent {
        let key = params.text_document.uri.as_str().to_string();
        if self.is_manifest(&key) {
            // Back to the file on disk.
            self.manifest.open = None;
            return self.reload_manifest();
        }
        self.due.remove(&key);
        if let Some(document) = self.documents.remove(&key) {
            self.publish(document.uri.as_str(), Vec::new(), None)?;
        }
        self.unconfigured_logged.remove(&key);
        self.publish_manifest()
    }

    /// Compiles every document whose debounce has run out.
    fn compile_due(&mut self) -> Sent {
        let now = Instant::now();
        let ready: Vec<String> = self
            .due
            .iter()
            .filter(|(_, deadline)| **deadline <= now)
            .map(|(key, _)| key.clone())
            .collect();
        for key in ready {
            self.compile(&key)?;
        }
        Ok(())
    }

    fn identity(&self, document: &Document) -> Identity {
        Identity {
            document: document.generation,
            manifest: self.manifest.generation,
            config: self.config_generation,
        }
    }

    /// Sends the document's current snapshot to the compile thread.
    fn compile(&mut self, key: &str) -> Sent {
        self.due.remove(key);
        let (model, missing) = self.model_for(key)?;
        let Some(document) = self.documents.get_mut(key) else {
            return Ok(());
        };
        document.missing = missing;
        let Some(document) = self.documents.get(key) else {
            return Ok(());
        };
        let job = Job {
            uri: key.to_string(),
            identity: self.identity(document),
            version: document.version,
            text: Arc::clone(&document.text),
            model,
        };
        if !self.worker.send(job) {
            return self.log(
                MessageType::ERROR,
                "mesh-lsp: the compile thread has stopped".to_string(),
            );
        }
        Ok(())
    }

    /// What the document at `key` is checked against, and the component
    /// it's configured as if the manifest doesn't declare it (D6, D9).
    fn model_for(&mut self, key: &str) -> Result<(Model, Option<String>), Disconnected> {
        let Some(snapshot) = self.manifest.snapshot.clone() else {
            return Ok((Model::None, None));
        };
        let Ok(manifest) = &snapshot.loaded else {
            // A broken manifest: model-less checking (D9).
            return Ok((Model::None, None));
        };
        let component = self.root.as_ref().and_then(|root| {
            let path = uri::to_path(key)?;
            let document_key = uri::key(root, &path)?;
            self.config.components.get(&document_key).cloned()
        });
        let Some(component) = component else {
            if self.unconfigured_logged.insert(key.to_string()) {
                self.log(
                    MessageType::INFO,
                    format!(
                        "{key} has no component in \"mesh.components\", so it's checked without a model"
                    ),
                )?;
            }
            return Ok((Model::None, None));
        };
        if manifest.components().contains_key(&component) {
            Ok((
                Model::Template {
                    manifest: Arc::clone(manifest),
                    component,
                },
                None,
            ))
        } else {
            Ok((Model::None, Some(component)))
        }
    }

    /// Publishes a compile's result if its snapshot is still current;
    /// otherwise drops it (D10's staleness rule).
    fn finish(&mut self, done: Done) -> Sent {
        let Some(document) = self.documents.get(&done.uri) else {
            return Ok(());
        };
        if self.identity(document) != done.identity || self.due.contains_key(&done.uri) {
            return Ok(());
        }
        let diagnostics = match done.result {
            Ok(diagnostics) => diagnostics,
            Err(message) => {
                return self.log(
                    MessageType::ERROR,
                    format!(
                        "mesh-lsp: internal error checking {} version {}: {message}",
                        done.uri, done.version
                    ),
                );
            }
        };
        let map = SourceMap::new(&done.text);
        let published = Text {
            source: &done.text,
            map: &map,
            unit: self.unit,
        }
        .diagnostics(&diagnostics);
        let uri = document.uri.as_str().to_string();
        if let Some(document) = self.documents.get_mut(&done.uri) {
            document.compiled = Some(Compiled {
                identity: done.identity,
                version: done.version,
                text: done.text,
                map,
                diagnostics,
            });
        }
        self.publish(&uri, published, Some(done.version))
    }

    fn publish(
        &self,
        document: &str,
        diagnostics: Vec<lsp_types::Diagnostic>,
        version: Option<i32>,
    ) -> Sent {
        let Ok(uri) = Uri::from_str(document) else {
            eprintln!("mesh-lsp: can't publish to {document:?}, which isn't a URI");
            return Ok(());
        };
        self.notify(
            "textDocument/publishDiagnostics",
            PublishDiagnosticsParams {
                uri,
                diagnostics,
                version,
            },
        )
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
        for diagnostic in &compiled.diagnostics {
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
