//! Open documents: their snapshots, compiles and publications (outline
//! D10).

use super::manifest::ManifestSnapshot;
use super::{apply_changes, Disconnected, Sent, Server};
use crate::convert::Text;
use crate::uri;
use crate::worker::{Done, Identity, Job, Model};
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    MessageType, PublishDiagnosticsParams, Uri,
};
use mesh_compiler::editor::Recovery;
use mesh_compiler::{CompileResult, SourceMap};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;

/// Any open text, whatever it is: the client's buffer, kept so that what
/// it is can be decided again when the configuration changes (outline
/// D9). Documents and the manifest's open text are derived from these.
pub(super) struct Buffer {
    pub(super) uri: Uri,
    pub(super) language_id: String,
    pub(super) version: i32,
    pub(super) text: String,
}

/// What an open buffer is.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    /// The configured manifest, whatever its `languageId`.
    Manifest,
    /// An MPRX document: `languageId` `mprx`.
    Document,
    /// Anything else: tracked, never compiled or published on.
    Ignored,
}

/// The `languageId` of an MPRX document.
pub(super) const MPRX: &str = "mprx";

/// An open MPRX document.
pub(super) struct Document {
    pub(super) uri: Uri,
    pub(super) version: i32,
    pub(super) text: Arc<str>,
    pub(super) generation: u64,
    /// The component its template is, if the configuration names one the
    /// manifest doesn't declare. Reported on the manifest (D9).
    pub(super) missing: Option<String>,
    /// The canonical state of the latest snapshot compiled and published.
    pub(super) compiled: Option<Compiled>,
    /// The snapshot whose compile panicked, if the latest one did.
    pub(super) failed: Option<Identity>,
}

/// A snapshot's canonical state: what the compiler said about its text.
pub(super) struct Compiled {
    pub(super) identity: Identity,
    pub(super) version: i32,
    pub(super) text: Arc<str>,
    pub(super) map: SourceMap,
    /// Exactly what `compile_with` (or `compile`) returned.
    pub(super) result: Arc<CompileResult>,
    /// Editor-only recovery, when `result` has no IR (outline D3). Only
    /// hover and definition read it; nothing publishes it.
    pub(super) recovery: Option<Arc<Recovery>>,
    /// The component the text was checked as, and the manifest snapshot
    /// that declares it; `None` when it was checked without a model.
    pub(super) template: Option<(String, Arc<ManifestSnapshot>)>,
}

impl Server {
    pub(super) fn did_open(&mut self, params: DidOpenTextDocumentParams) -> Sent {
        let document = params.text_document;
        let key = document.uri.as_str().to_string();
        self.buffers.insert(
            key.clone(),
            Buffer {
                uri: document.uri,
                language_id: document.language_id,
                version: document.version,
                text: document.text,
            },
        );
        match self.kind(&key) {
            Kind::Manifest => {
                // Reopened while it was a document: it isn't one now.
                self.forget_document(&key)?;
                self.reload_manifest()
            }
            Kind::Document => {
                // A second `didOpen` starts over from its text.
                self.documents.remove(&key);
                self.track_document(&key);
                self.compile(&key)?;
                self.publish_manifest()
            }
            Kind::Ignored => self.forget_document(&key),
        }
    }

    pub(super) fn did_change(&mut self, params: DidChangeTextDocumentParams) -> Sent {
        let key = params.text_document.uri.as_str().to_string();
        let version = params.text_document.version;

        let Some(buffer) = self.buffers.get_mut(&key) else {
            return self.log(
                MessageType::WARNING,
                format!("didChange for {key}, which isn't open; ignoring it"),
            );
        };
        if version < buffer.version {
            let current = buffer.version;
            return self.log(
                MessageType::WARNING,
                format!("didChange for {key} version {version}, older than {current}; ignoring it"),
            );
        }
        buffer.text = apply_changes(&buffer.text, &params.content_changes, self.unit);
        buffer.version = version;

        if self.is_manifest(&key) {
            return self.reload_manifest();
        }
        let Some(buffer) = self.buffers.get(&key) else {
            return Ok(());
        };
        let text = Arc::from(buffer.text.as_str());
        let Some(document) = self.documents.get_mut(&key) else {
            // An ignored buffer: nothing to check.
            return Ok(());
        };
        self.next_generation += 1;
        document.text = text;
        document.version = version;
        document.generation = self.next_generation;
        // Requests made against the replaced snapshot can't be answered.
        self.release(&key)?;

        if self.options.debounce.is_zero() {
            self.compile(&key)
        } else {
            self.due.insert(key, Instant::now() + self.options.debounce);
            Ok(())
        }
    }

    pub(super) fn did_close(&mut self, params: DidCloseTextDocumentParams) -> Sent {
        let key = params.text_document.uri.as_str().to_string();
        self.buffers.remove(&key);
        if self.is_manifest(&key) {
            // Back to the file on disk.
            return self.reload_manifest();
        }
        if self.documents.contains_key(&key) {
            self.forget_document(&key)?;
            return self.publish_manifest();
        }
        Ok(())
    }

    /// What the buffer at `key` is, under the current configuration.
    pub(super) fn kind(&self, key: &str) -> Kind {
        if self.is_manifest(key) {
            Kind::Manifest
        } else if self
            .buffers
            .get(key)
            .is_some_and(|buffer| buffer.language_id == MPRX)
        {
            Kind::Document
        } else {
            Kind::Ignored
        }
    }

    /// Decides again what every open buffer is, after the manifest's
    /// path may have changed: a buffer that stops being a document has
    /// its diagnostics cleared, and one that becomes a document is
    /// tracked. The caller recompiles.
    pub(super) fn reclassify(&mut self) -> Sent {
        let keys: Vec<String> = self.buffers.keys().cloned().collect();
        for key in keys {
            match self.kind(&key) {
                Kind::Document => self.track_document(&key),
                Kind::Manifest | Kind::Ignored => self.forget_document(&key)?,
            }
        }
        Ok(())
    }

    /// Starts tracking the buffer at `key` as a document, with a new
    /// snapshot of its text. Does nothing if it is tracked already.
    fn track_document(&mut self, key: &str) {
        if self.documents.contains_key(key) {
            return;
        }
        let Some(buffer) = self.buffers.get(key) else {
            return;
        };
        self.next_generation += 1;
        let document = Document {
            uri: buffer.uri.clone(),
            version: buffer.version,
            text: Arc::from(buffer.text.as_str()),
            generation: self.next_generation,
            missing: None,
            compiled: None,
            failed: None,
        };
        self.documents.insert(key.to_string(), document);
    }

    /// Stops tracking `key` as a document, if it is one: its diagnostics
    /// are cleared and its held requests answered.
    fn forget_document(&mut self, key: &str) -> Sent {
        self.due.remove(key);
        self.unconfigured_logged.remove(key);
        if let Some(document) = self.documents.remove(key) {
            self.publish(document.uri.as_str(), Vec::new(), None)?;
        }
        self.release(key)
    }

    /// Compiles every document whose debounce has run out.
    pub(super) fn compile_due(&mut self) -> Sent {
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

    pub(super) fn identity(&self, document: &Document) -> Identity {
        Identity {
            document: document.generation,
            manifest: self.manifest.generation,
            config: self.config_generation,
        }
    }

    /// Sends the document's current snapshot to the compile thread.
    pub(super) fn compile(&mut self, key: &str) -> Sent {
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
        // A manifest or configuration change starts a new snapshot here.
        self.release(key)
    }

    /// What the document at `key` is checked against, and the component
    /// it's configured as if the manifest doesn't declare it (D6, D9).
    pub(super) fn model_for(&mut self, key: &str) -> Result<(Model, Option<String>), Disconnected> {
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
    pub(super) fn finish(&mut self, done: Done) -> Sent {
        let Some(document) = self.documents.get(&done.uri) else {
            return Ok(());
        };
        if self.identity(document) != done.identity || self.due.contains_key(&done.uri) {
            return Ok(());
        }
        let (result, recovery) = match done.result {
            Ok(compiled) => (compiled.canonical, compiled.recovery),
            Err(message) => {
                if let Some(document) = self.documents.get_mut(&done.uri) {
                    document.failed = Some(done.identity);
                }
                self.log(
                    MessageType::ERROR,
                    format!(
                        "mesh-lsp: internal error checking {} version {}: {message}",
                        done.uri, done.version
                    ),
                )?;
                return self.release(&done.uri);
            }
        };
        let map = SourceMap::new(&done.text);
        let published = Text {
            source: &done.text,
            map: &map,
            unit: self.unit,
        }
        .diagnostics(&result.diagnostics);
        let uri = document.uri.as_str().to_string();
        // The identity matched, so the manifest snapshot in force is the
        // one this compile used.
        let template = match done.model {
            Model::Template { component, .. } => self
                .manifest
                .snapshot
                .clone()
                .map(|snapshot| (component, snapshot)),
            Model::None => None,
        };
        if let Some(document) = self.documents.get_mut(&done.uri) {
            document.compiled = Some(Compiled {
                identity: done.identity,
                version: done.version,
                text: done.text,
                map,
                result,
                recovery,
                template,
            });
        }
        self.publish(&uri, published, Some(done.version))?;
        self.release(&done.uri)
    }

    pub(super) fn publish(
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
}
