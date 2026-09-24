//! Requests that wait for their snapshot (outline D10, "Requests wait for
//! their snapshot").
//!
//! A request about a document is answered from the snapshot that is
//! current when it arrives. If that snapshot isn't compiled yet, the
//! request is held, and the document's debounce is cut short. When the
//! snapshot's result arrives, the request is answered from it. If a newer
//! snapshot replaces it first, the request is answered `ContentModified`,
//! as LSP 3.17 prescribes for a result the server knows is invalid.
//! Holding is bookkeeping: the loop never blocks.

use super::documents::Compiled;
use super::navigate::{self, Facts, Model};
use super::{Sent, Server};
use crate::convert::Text;
use crate::worker::Identity;
use lsp_server::{ErrorCode, RequestId};
use lsp_types::Position;
use serde_json::{json, Value};

/// A request that is answered from a document's compiled snapshot.
#[derive(Clone, Copy)]
pub(super) enum Query {
    Hover(Position),
    Definition(Position),
}

/// A request waiting for the snapshot it was made against.
pub(super) struct Held {
    id: RequestId,
    document: String,
    identity: Identity,
    query: Query,
}

impl Server {
    /// Answers `query` about `document` from its current snapshot, now if
    /// that snapshot is compiled, or once it is.
    pub(super) fn answer_or_hold(&mut self, id: RequestId, document: &str, query: Query) -> Sent {
        let Some(current) = self.documents.get(document) else {
            return self.respond(id, Value::Null);
        };
        let identity = self.identity(current);
        if current.failed == Some(identity) {
            return self.respond(id, Value::Null);
        }
        let ready = !self.due.contains_key(document)
            && current
                .compiled
                .as_ref()
                .is_some_and(|compiled| compiled.identity == identity);
        if ready {
            let answer = self.answer(document, query);
            return self.respond(id, answer);
        }
        self.held.push(Held {
            id,
            document: document.to_string(),
            identity,
            query,
        });
        if self.due.contains_key(document) {
            // Cut the debounce short: someone is waiting.
            self.compile(document)?;
        }
        Ok(())
    }

    /// Answers every request held on `document`: from its compiled state
    /// if they were made against it, `null` if its compile failed, and
    /// `ContentModified` if a newer snapshot has replaced theirs.
    pub(super) fn release(&mut self, document: &str) -> Sent {
        let current = self.documents.get(document).map(|document| {
            let identity = self.identity(document);
            let compiled = document
                .compiled
                .as_ref()
                .is_some_and(|compiled| compiled.identity == identity);
            (identity, compiled, document.failed == Some(identity))
        });
        let (waiting, others): (Vec<Held>, Vec<Held>) = std::mem::take(&mut self.held)
            .into_iter()
            .partition(|held| held.document == document);
        self.held = others;
        let mut still = Vec::new();
        for held in waiting {
            match current {
                Some((identity, compiled, failed)) if held.identity == identity => {
                    if compiled && !self.due.contains_key(document) {
                        let answer = self.answer(document, held.query);
                        self.respond(held.id, answer)?;
                    } else if failed {
                        self.respond(held.id, Value::Null)?;
                    } else {
                        still.push(held);
                    }
                }
                _ => self.respond_error(
                    held.id,
                    ErrorCode::ContentModified,
                    "the document changed before this request could be answered".to_string(),
                )?,
            }
        }
        self.held.extend(still);
        Ok(())
    }

    /// Answers a held request cancelled by `$/cancelRequest`.
    pub(super) fn cancel(&mut self, params: &Value) -> Sent {
        let id = match params.get("id") {
            Some(Value::Number(number)) => number
                .as_i64()
                .and_then(|n| i32::try_from(n).ok())
                .map(RequestId::from),
            Some(Value::String(string)) => Some(RequestId::from(string.clone())),
            _ => None,
        };
        let Some(id) = id else {
            return Ok(());
        };
        let Some(index) = self.held.iter().position(|held| held.id == id) else {
            return Ok(());
        };
        let held = self.held.remove(index);
        self.respond_error(
            held.id,
            ErrorCode::RequestCanceled,
            "the request was cancelled".to_string(),
        )
    }

    /// Answers every held request with `null`, before `shutdown`.
    pub(super) fn drain_held(&mut self) -> Sent {
        for held in std::mem::take(&mut self.held) {
            self.respond(held.id, Value::Null)?;
        }
        Ok(())
    }

    /// `query`'s answer from `document`'s compiled state. Only called
    /// when that state is the current snapshot's.
    fn answer(&self, document: &str, query: Query) -> Value {
        let Some(compiled) = self
            .documents
            .get(document)
            .and_then(|document| document.compiled.as_ref())
        else {
            return Value::Null;
        };
        match query {
            Query::Hover(position) => self
                .navigation(compiled, position)
                .and_then(|(facts, model, text, offset)| {
                    navigate::hover(facts, model, text, offset, self.client.markdown)
                })
                .map_or(Value::Null, |hover| json!(hover)),
            Query::Definition(position) => {
                let Some((_, snapshot)) = &compiled.template else {
                    return Value::Null;
                };
                let manifest_text = Text {
                    source: &snapshot.text,
                    map: &snapshot.map,
                    unit: self.unit,
                };
                self.navigation(compiled, position)
                    .and_then(|(facts, model, _, offset)| {
                        navigate::definition(facts, model, &snapshot.uri, manifest_text, offset)
                    })
                    .map_or(Value::Null, |location| json!(location))
            }
        }
    }

    /// What hover and definition read for `compiled` at `position`: its
    /// facts, its model, its text, and the position as a byte offset.
    fn navigation<'a>(
        &self,
        compiled: &'a Compiled,
        position: Position,
    ) -> Option<(Facts<'a>, Model<'a>, Text<'a>, usize)> {
        let (component, snapshot) = compiled.template.as_ref()?;
        let manifest = snapshot.loaded.as_ref().ok()?;
        // Canonical state when the compile has it; recovery only when it
        // has no IR (outline D3).
        let facts = match (&compiled.result.analysis, &compiled.recovery) {
            (Some(analysis), _) => Facts::Canonical(analysis),
            (None, Some(recovery)) if compiled.result.ir.is_none() => Facts::Recovered(recovery),
            _ => return None,
        };
        let text = Text {
            source: &compiled.text,
            map: &compiled.map,
            unit: self.unit,
        };
        let model = Model {
            component,
            manifest,
        };
        Some((facts, model, text, text.offset(position)))
    }
}
