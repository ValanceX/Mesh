//! The MESH component manifest: the declared model an MPRX template is
//! checked against.
//!
//! A manifest is a versioned JSON document that declares components (their
//! props, events, commands and expression scope) and shared named types.
//! [`load`] parses and validates one completely. It either returns a
//! [`Manifest`] that is valid in every respect, or every problem it found,
//! as [`Diagnostic`]s whose spans index the manifest's own text. A
//! malformed manifest never reaches MPRX checking.
//!
//! The JSON format is specified by `schemas/manifest-v1.schema.json`. The
//! model types here are plain data: `serde_json` is an implementation
//! detail of [`load`] and never appears in this crate's API.

use mesh_syntax::{Diagnostic, DiagnosticCode, Severity, Span};
use std::collections::BTreeMap;

mod json;
mod validate;

/// The manifest schema version this MESH reads.
pub const VERSION: u64 = 1;

/// The deepest that a manifest's JSON arrays and objects may nest. The
/// document itself is level 1. A deeper manifest is a
/// `manifest-nesting-too-deep` error, reported before anything else is
/// checked.
pub const MAX_NESTING_DEPTH: usize = 128;

/// A type a manifest can write. Named references are aliases for their
/// definition; [`Manifest::expand`] follows them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    String,
    Number,
    Boolean,
    /// The value `null`. Not absence: that is [`Type::Optional`].
    Null,
    /// Any present value. `Optional(Any)` also allows absence.
    Any,
    /// `list<T>`.
    List(Box<Type>),
    /// An exact record: these fields, and no others.
    Record(BTreeMap<String, Field>),
    /// A reference to one of the manifest's named types.
    Named(String),
    /// `T?`: a `T`, or absent.
    ///
    /// In a loaded [`Manifest`], `Optional(inner)` never has an `inner`
    /// that [`Manifest::expand`]s to `Optional`: not directly, and not
    /// through any chain of named references. `inner` may still *contain*
    /// optional types (a list of them, a record with optional fields).
    Optional(Box<Type>),
}

/// A prop or record field declaration: its type, and whether it must be
/// supplied.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub ty: Type,
    pub required: bool,
}

/// An event a component can raise.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// The type of `$event` in a handler, or `None` if the event carries
    /// no value.
    pub payload: Option<Type>,
}

/// A command a component's template can invoke.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    /// The parameters in order. Every one is required.
    pub parameters: Vec<Parameter>,
}

/// One command parameter.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    pub name: String,
    pub ty: Type,
}

/// One component's interface.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Component {
    /// Inputs, used where an instance appears in another template.
    pub props: BTreeMap<String, Field>,
    /// Events, bound with `on.<name>` on an instance.
    pub events: BTreeMap<String, Event>,
    /// Commands the component's own template can invoke.
    pub commands: BTreeMap<String, Command>,
    /// Every name an expression in the component's own template can
    /// reference, and its type. Nothing else is in scope.
    pub scope: BTreeMap<String, Type>,
}

/// A declaration a template can name, to look its span up with
/// [`Manifest::span_of`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Declaration<'a> {
    /// A component, by tag name.
    Component(&'a str),
    /// A prop of a component.
    Prop { component: &'a str, prop: &'a str },
    /// An event of a component.
    Event { component: &'a str, event: &'a str },
    /// A command of a component.
    Command {
        component: &'a str,
        command: &'a str,
    },
    /// The parameter at `index` (0-based) of a component's command.
    Parameter {
        component: &'a str,
        command: &'a str,
        index: usize,
    },
    /// A name in a component's scope.
    Scope { component: &'a str, name: &'a str },
    /// A named type.
    NamedType(&'a str),
    /// A field of a named type that is a record. The fields of a record
    /// nested inside another type have no name to find them by.
    Field { ty: &'a str, field: &'a str },
}

/// [`Declaration`], owned, as the key of a manifest's span table.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DeclarationKey {
    Component(String),
    Prop(String, String),
    Event(String, String),
    Command(String, String),
    Parameter(String, String, usize),
    Scope(String, String),
    NamedType(String),
    Field(String, String),
}

impl From<Declaration<'_>> for DeclarationKey {
    fn from(declaration: Declaration<'_>) -> Self {
        let own = str::to_string;
        match declaration {
            Declaration::Component(name) => DeclarationKey::Component(own(name)),
            Declaration::Prop { component, prop } => {
                DeclarationKey::Prop(own(component), own(prop))
            }
            Declaration::Event { component, event } => {
                DeclarationKey::Event(own(component), own(event))
            }
            Declaration::Command { component, command } => {
                DeclarationKey::Command(own(component), own(command))
            }
            Declaration::Parameter {
                component,
                command,
                index,
            } => DeclarationKey::Parameter(own(component), own(command), index),
            Declaration::Scope { component, name } => {
                DeclarationKey::Scope(own(component), own(name))
            }
            Declaration::NamedType(name) => DeclarationKey::NamedType(own(name)),
            Declaration::Field { ty, field } => DeclarationKey::Field(own(ty), own(field)),
        }
    }
}

/// A loaded, fully validated manifest. It owns all of its data.
///
/// Only [`load`] creates one, so every named reference resolves, no named
/// type is recursive, and no optional type wraps another (see
/// [`Type::Optional`]). The model keeps types as the manifest wrote them:
/// named references stay [`Type::Named`], and [`Manifest::expand`]
/// follows them on demand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    types: BTreeMap<String, Type>,
    components: BTreeMap<String, Component>,
    /// The `"components"` key, where a missing component is reported.
    components_span: Span,
    /// Where each declaration's key is. Kept beside the model rather than
    /// in it, so that two types mean the same whether or not they were
    /// written in the same place.
    spans: BTreeMap<DeclarationKey, Span>,
}

impl Manifest {
    /// The named types, by name.
    pub fn types(&self) -> &BTreeMap<String, Type> {
        &self.types
    }

    /// The components, by tag name.
    pub fn components(&self) -> &BTreeMap<String, Component> {
        &self.components
    }

    /// Where `declaration` is declared: the span of its key, quotes
    /// included, such as `"avatar"`. A parameter has no key, so its span
    /// is its `"name"` value. `None` if the manifest doesn't declare it.
    pub fn span_of(&self, declaration: Declaration<'_>) -> Option<Span> {
        self.spans.get(&DeclarationKey::from(declaration)).copied()
    }

    /// `ty` with any named references at its top level followed to their
    /// definition. The result is never [`Type::Named`].
    pub fn expand<'a>(&'a self, mut ty: &'a Type) -> &'a Type {
        while let Type::Named(name) = ty {
            ty = &self.types[name];
        }
        ty
    }

    /// The template context for component `name`: the component whose
    /// template the file being checked is. A name the manifest doesn't
    /// declare is a `manifest-missing-component` error, located at the
    /// manifest's `"components"` key.
    pub fn template(&self, name: &str) -> Result<Template<'_>, Diagnostic> {
        match self.components.get_key_value(name) {
            Some((name, component)) => Ok(Template {
                manifest: self,
                name,
                component,
            }),
            None => Err(Diagnostic {
                severity: Severity::Error,
                code: DiagnosticCode::MANIFEST_MISSING_COMPONENT,
                message: format!(
                    "the manifest declares no component {name:?}, which this file is the template of"
                ),
                span: self.components_span,
                suggestions: Vec::new(),
            }),
        }
    }
}

/// A borrowed view of one declared component of a [`Manifest`]: the
/// component whose template is being checked.
///
/// A `Template` holds only references into the manifest that made it, so
/// it can't outlive that manifest, and it is cheap to copy. Its fields
/// are private and [`Manifest::template`] is the only constructor, so a
/// `Template` for an undeclared component can't exist:
///
/// ```compile_fail
/// fn forge<'m>(
///     manifest: &'m mesh_manifest::Manifest,
///     component: &'m mesh_manifest::Component,
/// ) -> mesh_manifest::Template<'m> {
///     mesh_manifest::Template { manifest, name: "anything", component }
/// }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Template<'m> {
    manifest: &'m Manifest,
    name: &'m str,
    component: &'m Component,
}

impl<'m> Template<'m> {
    pub fn manifest(&self) -> &'m Manifest {
        self.manifest
    }

    /// The component's tag name.
    pub fn name(&self) -> &'m str {
        self.name
    }

    pub fn component(&self) -> &'m Component {
        self.component
    }
}

/// Parses and validates a manifest. `source` is the manifest's text; a
/// leading byte-order mark is allowed. On failure, returns every problem
/// found, in source order, with spans into `source`.
pub fn load(source: &str) -> Result<Manifest, Vec<Diagnostic>> {
    let document = json::parse(source).map_err(|error| {
        let (code, message, span) = match error {
            json::ReadError::Syntax { message, span } => (
                DiagnosticCode::MANIFEST_SYNTAX_ERROR,
                format!("the manifest isn't valid JSON: {message}"),
                span,
            ),
            json::ReadError::TooDeep { span } => (
                DiagnosticCode::MANIFEST_NESTING_TOO_DEEP,
                format!(
                    "the manifest nests arrays and objects more than {MAX_NESTING_DEPTH} levels deep"
                ),
                span,
            ),
        };
        vec![Diagnostic {
            severity: Severity::Error,
            code,
            message,
            span,
            suggestions: Vec::new(),
        }]
    })?;
    validate::manifest(&document)
}
