//! The whole check `mesh check` performs, for any program that hosts
//! MESH: load a component model, then check a source against it.
//!
//! This module owns the check's rules. A host (the `mesh` CLI, the
//! WebAssembly build, or any later one) gathers the inputs and presents
//! the result, and decides nothing about the check itself:
//!
//! - a manifest with errors stops the check, and only its errors are
//!   reported, against the manifest;
//! - a manifest that doesn't declare the component stops the check with
//!   `manifest-missing-component`, against the manifest;
//! - otherwise the source is compiled, as the template of that component
//!   or without a model, and its diagnostics are reported against it.
//!
//! The check has two public phases, [`Model::load`] and [`source`],
//! because a host may need to do something between them: the CLI reads
//! the source file only once the model has loaded, so a broken manifest
//! is reported even when the file can't be read. [`run`] is exactly those
//! two phases, in that order, for a host that has every input up front.
//!
//! `mesh-lsp` deliberately isn't a host of this module. An editor needs a
//! template's own syntax errors while its manifest is broken, so the
//! server checks such a document without a model instead of stopping,
//! through [`compile_with`] directly.

use crate::{compile, compile_with, render_json, CompileOptions};
use mesh_syntax::{Diagnostic, Severity};

/// A manifest that loaded cleanly, and a component it declares: what a
/// source is checked against.
#[derive(Debug, Clone)]
pub struct Model {
    manifest: mesh_manifest::Manifest,
    component: String,
}

impl Model {
    /// Loads `manifest` and looks up `component` in it.
    ///
    /// On failure, the diagnostics are reported against the manifest:
    /// every error the manifest has, or, when it has none, the one
    /// `manifest-missing-component`.
    pub fn load(manifest: &str, component: &str) -> Result<Model, Vec<Diagnostic>> {
        let manifest = mesh_manifest::load(manifest)?;
        manifest
            .template(component)
            .map_err(|missing| vec![missing])?;
        Ok(Model {
            manifest,
            component: component.to_string(),
        })
    }

    /// The component whose template a source is checked as.
    pub fn component(&self) -> &str {
        &self.component
    }
}

/// Checks `source` as the template of `model`'s component, or with no
/// model. The diagnostics are reported against `source`, in the order the
/// compiler produced them.
pub fn source(source: &str, model: Option<&Model>) -> Vec<Diagnostic> {
    match model {
        None => compile(source).diagnostics,
        Some(model) => {
            let template = model
                .manifest
                .template(&model.component)
                .expect("Model::load checked that the manifest declares its component");
            compile_with(source, &CompileOptions::with_template(template)).diagnostics
        }
    }
}

/// Every input of one check, for a host that has them all up front.
///
/// Paths only identify documents in the diagnostics; nothing here reads
/// them.
#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub struct Request<'a> {
    /// The source text.
    pub source: &'a str,
    /// The source's path, as the diagnostics name it.
    pub path: &'a str,
    /// The model to check against, if any.
    pub model: Option<ModelInput<'a>>,
}

impl<'a> Request<'a> {
    /// A check of `source`, at `path`, with no model.
    pub fn new(source: &'a str, path: &'a str) -> Self {
        Request {
            source,
            path,
            model: None,
        }
    }

    /// The same check, against `model`.
    pub fn with_model(self, model: ModelInput<'a>) -> Self {
        Request {
            model: Some(model),
            ..self
        }
    }
}

/// The model inputs of a [`Request`]: a manifest's text, its path, and
/// the component whose template the source is. The component is always
/// explicit.
#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub struct ModelInput<'a> {
    /// The manifest's text.
    pub manifest: &'a str,
    /// The manifest's path, as the diagnostics name it.
    pub path: &'a str,
    /// The component whose template the source is.
    pub component: &'a str,
}

impl<'a> ModelInput<'a> {
    /// Model inputs: `manifest` at `path`, checking as `component`.
    pub fn new(manifest: &'a str, path: &'a str, component: &'a str) -> Self {
        ModelInput {
            manifest,
            path,
            component,
        }
    }
}

/// Which document a [`Report`]'s diagnostics are reported against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Document {
    /// The source.
    Source,
    /// The manifest: the check stopped at the model.
    Manifest,
}

/// What one check found.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The document the diagnostics are reported against.
    pub document: Document,
    /// Every diagnostic, in order.
    pub diagnostics: Vec<Diagnostic>,
}

impl Report {
    /// Whether any diagnostic is an error. Warnings alone don't fail a
    /// check.
    pub fn has_errors(&self) -> bool {
        has_errors(&self.diagnostics)
    }

    /// The report as a diagnostics document (see [`render_json`]), against
    /// the text and path of the document it's reported against. `request`
    /// must be the one the report came from.
    pub fn render_json(&self, request: &Request<'_>) -> String {
        let (text, path) = match (self.document, request.model) {
            (Document::Manifest, Some(model)) => (model.manifest, model.path),
            _ => (request.source, request.path),
        };
        render_json(text, path, &self.diagnostics)
    }
}

/// Whether `diagnostics` fail a check: whether any is an error. Warnings
/// alone don't.
pub fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
}

/// The whole check: [`Model::load`], and if it succeeds, [`source`].
pub fn run(request: &Request<'_>) -> Report {
    let model = match request.model {
        None => None,
        Some(input) => match Model::load(input.manifest, input.component) {
            Ok(model) => Some(model),
            Err(diagnostics) => {
                return Report {
                    document: Document::Manifest,
                    diagnostics,
                }
            }
        },
    };
    Report {
        document: Document::Source,
        diagnostics: source(request.source, model.as_ref()),
    }
}
