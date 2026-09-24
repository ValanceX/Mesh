//! The manifest's snapshots and what depends on them (outline D9).

use super::{Sent, Server};
use crate::convert::Text;
use crate::uri;
use lsp_types::MessageType;
use mesh_compiler::SourceMap;
use mesh_manifest::Manifest;
use mesh_syntax::Diagnostic;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

/// One version of the manifest's text, loaded (D9).
pub(super) struct ManifestSnapshot {
    pub(super) uri: String,
    pub(super) text: String,
    pub(super) map: SourceMap,
    pub(super) loaded: Result<Arc<Manifest>, Vec<Diagnostic>>,
}

#[derive(Default)]
pub(super) struct ManifestState {
    pub(super) generation: u64,
    /// Where the configuration says the manifest is.
    pub(super) path: Option<PathBuf>,
    /// The client's URI and text while the manifest is open in it.
    pub(super) open: Option<(String, String)>,
    /// `None` without a configured, readable manifest.
    pub(super) snapshot: Option<Arc<ManifestSnapshot>>,
}

impl Server {
    pub(super) fn is_manifest(&self, document: &str) -> bool {
        self.manifest.path.is_some() && uri::to_path(document) == self.manifest.path
    }

    /// Starts a new manifest generation from the open buffer or the disk,
    /// then recompiles every open document against it.
    pub(super) fn reload_manifest(&mut self) -> Sent {
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
    pub(super) fn publish_manifest(&mut self) -> Sent {
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
}
