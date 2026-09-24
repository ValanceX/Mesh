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
use mesh_compiler::{CompileResult, SourceMap};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;

/// An open document.
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
    /// The component the text was checked as, and the manifest snapshot
    /// that declares it; `None` when it was checked without a model.
    pub(super) template: Option<(String, Arc<ManifestSnapshot>)>,
}

impl Server {
    pub(super) fn did_open(&mut self, params: DidOpenTextDocumentParams) -> Sent {
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
                failed: None,
            },
        );
        self.compile(&key)?;
        self.publish_manifest()
    }

    pub(super) fn did_change(&mut self, params: DidChangeTextDocumentParams) -> Sent {
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
        self.release(&key)?;
        self.publish_manifest()
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
        let result = match done.result {
            Ok(result) => result,
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
