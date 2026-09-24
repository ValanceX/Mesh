//! Completion (outline D4), presented: each candidate the compiler
//! derived becomes one LSP item, in the compiler's order. Nothing here
//! chooses what is offered.

use super::requests::Query;
use super::{Sent, Server};
use crate::convert::Text;
use crate::worker::{CompleteDone, CompleteJob};
use lsp_server::{ErrorCode, RequestId};
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionItemLabelDetails, CompletionList,
    CompletionTextEdit, MessageType, TextEdit,
};
use mesh_compiler::editor::{Candidate, CandidateKind};
use serde_json::{json, Value};

impl Server {
    /// Sends a completion for `document`'s current, compiled snapshot to
    /// the compile thread; [`Server::finish_completion`] answers it.
    pub(super) fn start_completion(&mut self, id: RequestId, document: &str, query: Query) -> Sent {
        let Query::Completion(position) = query else {
            return self.respond(id, Value::Null);
        };
        let Some(current) = self.documents.get(document) else {
            return self.respond(id, Value::Null);
        };
        let identity = self.identity(current);
        let job = current.compiled.as_ref().and_then(|compiled| {
            let (component, snapshot) = compiled.template.as_ref()?;
            let manifest = snapshot.loaded.as_ref().ok()?;
            let text = Text {
                source: &compiled.text,
                map: &compiled.map,
                unit: self.unit,
            };
            Some(CompleteJob {
                id: id.clone(),
                uri: document.to_string(),
                identity,
                text: compiled.text.clone(),
                offset: text.offset(position),
                manifest: manifest.clone(),
                component: component.clone(),
                canonical: compiled.result.clone(),
            })
        });
        // No model: nothing to offer (D4).
        let Some(job) = job else {
            return self.respond(id, json!([]));
        };
        self.completing.push(id.clone());
        if !self.worker.complete(job) {
            self.completing.retain(|pending| pending != &id);
            self.respond(id, json!([]))?;
            return self.log(
                MessageType::ERROR,
                "mesh-lsp: the compile thread has stopped".to_string(),
            );
        }
        Ok(())
    }

    /// Answers a completion from the compile thread, if its snapshot is
    /// still current; otherwise `ContentModified`.
    pub(super) fn finish_completion(&mut self, done: CompleteDone) -> Sent {
        let Some(index) = self.completing.iter().position(|id| id == &done.id) else {
            // Cancelled, or answered at shutdown.
            return Ok(());
        };
        self.completing.remove(index);
        let current = self
            .documents
            .get(&done.uri)
            .filter(|document| self.identity(document) == done.identity)
            .filter(|_| !self.due.contains_key(&done.uri))
            .and_then(|document| document.compiled.as_ref())
            .filter(|compiled| compiled.identity == done.identity);
        let Some(compiled) = current else {
            return self.respond_error(
                done.id,
                ErrorCode::ContentModified,
                "the document changed before completion was ready".to_string(),
            );
        };
        let candidates = match done.candidates {
            Ok(candidates) => candidates,
            Err(message) => {
                self.log(
                    MessageType::ERROR,
                    format!(
                        "mesh-lsp: internal error completing in {}: {message}",
                        done.uri
                    ),
                )?;
                return self.respond(done.id, json!([]));
            }
        };
        let text = Text {
            source: &compiled.text,
            map: &compiled.map,
            unit: self.unit,
        };
        let items: Vec<CompletionItem> = candidates
            .iter()
            .enumerate()
            .map(|(index, candidate)| item(index, candidate, text))
            .collect();
        let list = CompletionList {
            is_incomplete: false,
            items,
        };
        self.respond(done.id, json!(list))
    }
}

fn item(index: usize, candidate: &Candidate, text: Text<'_>) -> CompletionItem {
    let kind = match candidate.kind {
        CandidateKind::Component => CompletionItemKind::CLASS,
        CandidateKind::Prop => CompletionItemKind::PROPERTY,
        CandidateKind::Event => CompletionItemKind::EVENT,
        CandidateKind::ScopeName => CompletionItemKind::VARIABLE,
        CandidateKind::Member => CompletionItemKind::FIELD,
        CandidateKind::Command => CompletionItemKind::FUNCTION,
        // `CandidateKind` is non-exhaustive; nothing else exists today.
        _ => CompletionItemKind::TEXT,
    };
    CompletionItem {
        label: candidate.label.clone(),
        kind: Some(kind),
        detail: candidate.detail.clone(),
        label_details: candidate.required.then(|| CompletionItemLabelDetails {
            detail: None,
            description: Some("required".to_string()),
        }),
        sort_text: Some(format!("{index:05}")),
        filter_text: Some(candidate.label.clone()),
        text_edit: Some(CompletionTextEdit::Edit(TextEdit {
            range: text.range(candidate.replace),
            new_text: candidate.label.clone(),
        })),
        ..CompletionItem::default()
    }
}
