//! Semantic analysis for MPRX: checks a template's Semantic IR against the
//! component manifest it is the template of.
//!
//! [`analyze`] resolves every name in the IR against the manifest: each
//! element's tag to a component, its attributes to that component's props
//! and its `on.` bindings to its events, and every reference and command to
//! the template's own scope and commands. It also checks that required
//! props are supplied and that commands and `$event` appear only where
//! they may.
//!
//! Analysis never changes the IR. Its results are kept beside it, in the
//! returned [`Analysis`]: what each name resolved to, and a [`Fact`] for
//! each problem found. Facts are data, not prose; turning them into
//! diagnostics (codes, messages and suggestions) is `mesh-compiler`'s job.

use mesh_manifest::Template;
use mesh_syntax::Span;

mod check;
mod relation;
mod types;

pub use relation::{is_assignable, join};
pub use types::{FieldTy, Ty};

/// Analyzes `ir` as the template of `template`'s component.
pub fn analyze(ir: &mesh_semantic::Element, template: Template<'_>) -> Analysis {
    check::analyze(ir, template)
}

/// The results of analyzing one template: kept beside the IR, never
/// written into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Analysis {
    facts: Vec<Fact>,
    resolutions: Vec<Resolution>,
}

impl Analysis {
    /// Every problem found, in source order: sorted by the start of each
    /// fact's span, and in the order they were found where two start at
    /// the same byte.
    pub fn facts(&self) -> &[Fact] {
        &self.facts
    }

    /// What each name that resolved refers to, in the order analysis met
    /// them. A name that didn't resolve has a [`Fact`] instead.
    pub fn resolutions(&self) -> &[Resolution] {
        &self.resolutions
    }
}

/// A name in the template, and the manifest declaration it refers to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    /// The name as written: a tag name, attribute name, event name,
    /// reference or command name.
    pub span: Span,
    pub target: Target,
}

/// The manifest declaration a name resolved to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A component, by tag name.
    Component(String),
    /// A prop of a component.
    Prop { component: String, prop: String },
    /// An event of a component.
    Event { component: String, event: String },
    /// A name in the template's scope.
    Scope(String),
    /// A command of the template's component.
    Command(String),
}

/// A problem analysis found, with the context a diagnostic needs.
///
/// Facts hold no prose and no suggestions: `candidates` lists the names
/// that *were* available, and choosing a close one is the diagnostic
/// layer's job. Candidates are in the manifest's (sorted) order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fact {
    /// An element's tag names no component. Span: the tag name.
    UnknownComponent {
        name: String,
        span: Span,
        candidates: Vec<String>,
    },
    /// An attribute isn't a prop of the element's component. Span: the
    /// attribute name.
    UnknownProp {
        component: String,
        prop: String,
        span: Span,
        candidates: Vec<String>,
    },
    /// An element omits a prop its component declares required. Span: the
    /// element's tag name.
    MissingRequiredProp {
        component: String,
        prop: String,
        span: Span,
    },
    /// An `on.` binding names an event the component doesn't have. Span:
    /// the event name.
    UnknownEvent {
        component: String,
        event: String,
        span: Span,
        candidates: Vec<String>,
    },
    /// A reference isn't in the template's scope. Span: the reference.
    UnknownReference {
        name: String,
        span: Span,
        candidates: Vec<String>,
    },
    /// A command the template's component doesn't declare. Span: the
    /// command name.
    UnknownCommand {
        command: String,
        span: Span,
        candidates: Vec<String>,
    },
    /// A command invoked with the wrong number of arguments. Span: the
    /// whole invocation.
    CommandArityMismatch {
        command: String,
        expected: usize,
        found: usize,
        span: Span,
    },
    /// A command invoked anywhere but as the whole handler of an `on.`
    /// binding. Span: the whole invocation.
    CommandOutsideHandler { command: String, span: Span },
    /// An `on.` handler that isn't a command invocation. Span: the
    /// handler expression.
    HandlerNotCommand { event: String, span: Span },
    /// `$event` outside the arguments of an `on.` handler's command.
    EventValueOutsideHandler { span: Span },
    /// `$event` in the handler of an event that declares no payload.
    EventHasNoPayload {
        component: String,
        event: String,
        span: Span,
    },
    /// A `$` name other than `$event`. `name` has no `$`.
    UnknownSpecialValue {
        name: String,
        span: Span,
        candidates: Vec<String>,
    },
}

impl Fact {
    /// The source span the fact is about.
    pub fn span(&self) -> Span {
        match self {
            Fact::UnknownComponent { span, .. }
            | Fact::UnknownProp { span, .. }
            | Fact::MissingRequiredProp { span, .. }
            | Fact::UnknownEvent { span, .. }
            | Fact::UnknownReference { span, .. }
            | Fact::UnknownCommand { span, .. }
            | Fact::CommandArityMismatch { span, .. }
            | Fact::CommandOutsideHandler { span, .. }
            | Fact::HandlerNotCommand { span, .. }
            | Fact::EventValueOutsideHandler { span }
            | Fact::EventHasNoPayload { span, .. }
            | Fact::UnknownSpecialValue { span, .. } => *span,
        }
    }
}
