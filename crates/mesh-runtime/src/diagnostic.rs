//! Runtime and assembly diagnostics, and the runtime diagnostics document
//! (docs/manual/runtime.md, "Diagnostics").

use mesh_syntax::source_map::{ColumnUnit, SourceMap};
use serde_json::{json, Value};
use std::fmt;

/// The six forms a diagnostic's location takes. Each code always uses
/// the same one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Form {
    /// A span in the manifest's text.
    Model,
    /// The program as a whole.
    Program,
    /// A template, by its position in the host's list.
    Template,
    /// A span in a template's source.
    Source,
    /// A path into the snapshot or the payload.
    Input,
    /// The handler identifier given to dispatch.
    Handler,
}

impl Form {
    /// The form's name in the document: `location.kind`.
    pub fn as_str(self) -> &'static str {
        match self {
            Form::Model => "model",
            Form::Program => "program",
            Form::Template => "template",
            Form::Source => "source",
            Form::Input => "input",
            Form::Handler => "handler",
        }
    }
}

/// An assembly or runtime diagnostic code, and the location form it
/// always has. Codes are stable: never renamed or reused.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct RuntimeCode(&'static str, Form);

impl RuntimeCode {
    // Assembly: program validation, and `mesh check-program`.
    pub const MALFORMED_TEMPLATE: RuntimeCode =
        RuntimeCode("assembly-malformed-template", Form::Template);
    pub const UNSUPPORTED_FORMAT_VERSION: RuntimeCode =
        RuntimeCode("assembly-unsupported-format-version", Form::Template);
    pub const FINGERPRINT_MISMATCH: RuntimeCode =
        RuntimeCode("assembly-fingerprint-mismatch", Form::Template);
    pub const DUPLICATE_TEMPLATE: RuntimeCode =
        RuntimeCode("assembly-duplicate-template", Form::Source);
    pub const MISSING_ROOT: RuntimeCode = RuntimeCode("assembly-missing-root", Form::Program);
    pub const UNBOUND_SCOPE_NAME: RuntimeCode =
        RuntimeCode("assembly-unbound-scope-name", Form::Source);
    pub const UNSOUND_BINDING: RuntimeCode = RuntimeCode("assembly-unsound-binding", Form::Source);
    pub const CYCLE: RuntimeCode = RuntimeCode("assembly-cycle", Form::Source);
    pub const COMPOSITE_EVENT: RuntimeCode = RuntimeCode("assembly-composite-event", Form::Source);
    pub const COMPOSITE_CHILDREN: RuntimeCode =
        RuntimeCode("assembly-composite-children", Form::Source);
    // Input validation.
    pub const MISSING_VALUE: RuntimeCode = RuntimeCode("runtime-missing-value", Form::Input);
    pub const VALUE_MISMATCH: RuntimeCode = RuntimeCode("runtime-value-mismatch", Form::Input);
    pub const UNKNOWN_FIELD: RuntimeCode = RuntimeCode("runtime-unknown-field", Form::Input);
    pub const ABSENT_ELEMENT: RuntimeCode = RuntimeCode("runtime-absent-element", Form::Input);
    pub const NUMBER_OUT_OF_RANGE: RuntimeCode =
        RuntimeCode("runtime-number-out-of-range", Form::Input);
    pub const NON_FINITE_INPUT: RuntimeCode = RuntimeCode("runtime-non-finite-input", Form::Input);
    pub const UNPAIRED_SURROGATE: RuntimeCode =
        RuntimeCode("runtime-unpaired-surrogate", Form::Input);
    pub const UNSUPPORTED_VALUE: RuntimeCode =
        RuntimeCode("runtime-unsupported-value", Form::Input);
    pub const UNEXPECTED_PAYLOAD: RuntimeCode =
        RuntimeCode("runtime-unexpected-payload", Form::Input);
    pub const HANDLER_OTHER_PROGRAM: RuntimeCode =
        RuntimeCode("runtime-handler-other-program", Form::Handler);
    pub const UNKNOWN_HANDLER: RuntimeCode = RuntimeCode("runtime-unknown-handler", Form::Handler);
    // Evaluation.
    pub const OPERAND_MISMATCH: RuntimeCode = RuntimeCode("runtime-operand-mismatch", Form::Source);
    pub const NOT_A_RECORD: RuntimeCode = RuntimeCode("runtime-not-a-record", Form::Source);
    pub const MISSING_MEMBER: RuntimeCode = RuntimeCode("runtime-missing-member", Form::Source);
    pub const CONTENT_NOT_TEXT: RuntimeCode = RuntimeCode("runtime-content-not-text", Form::Source);
    pub const PROP_MISMATCH: RuntimeCode = RuntimeCode("runtime-prop-mismatch", Form::Source);
    pub const ARGUMENT_MISMATCH: RuntimeCode =
        RuntimeCode("runtime-argument-mismatch", Form::Source);
    pub const NON_FINITE_OUTPUT: RuntimeCode =
        RuntimeCode("runtime-non-finite-output", Form::Source);
    pub const ABSENT_ELEMENT_OUTPUT: RuntimeCode =
        RuntimeCode("runtime-absent-element-output", Form::Source);
    // Internal.
    pub const KEY_COLLISION: RuntimeCode = RuntimeCode("runtime-key-collision", Form::Source);

    /// Every code, in catalogue order.
    pub const ALL: &'static [RuntimeCode] = &[
        RuntimeCode::MALFORMED_TEMPLATE,
        RuntimeCode::UNSUPPORTED_FORMAT_VERSION,
        RuntimeCode::FINGERPRINT_MISMATCH,
        RuntimeCode::DUPLICATE_TEMPLATE,
        RuntimeCode::MISSING_ROOT,
        RuntimeCode::UNBOUND_SCOPE_NAME,
        RuntimeCode::UNSOUND_BINDING,
        RuntimeCode::CYCLE,
        RuntimeCode::COMPOSITE_EVENT,
        RuntimeCode::COMPOSITE_CHILDREN,
        RuntimeCode::MISSING_VALUE,
        RuntimeCode::VALUE_MISMATCH,
        RuntimeCode::UNKNOWN_FIELD,
        RuntimeCode::ABSENT_ELEMENT,
        RuntimeCode::NUMBER_OUT_OF_RANGE,
        RuntimeCode::NON_FINITE_INPUT,
        RuntimeCode::UNPAIRED_SURROGATE,
        RuntimeCode::UNSUPPORTED_VALUE,
        RuntimeCode::UNEXPECTED_PAYLOAD,
        RuntimeCode::HANDLER_OTHER_PROGRAM,
        RuntimeCode::UNKNOWN_HANDLER,
        RuntimeCode::OPERAND_MISMATCH,
        RuntimeCode::NOT_A_RECORD,
        RuntimeCode::MISSING_MEMBER,
        RuntimeCode::CONTENT_NOT_TEXT,
        RuntimeCode::PROP_MISMATCH,
        RuntimeCode::ARGUMENT_MISMATCH,
        RuntimeCode::NON_FINITE_OUTPUT,
        RuntimeCode::ABSENT_ELEMENT_OUTPUT,
        RuntimeCode::KEY_COLLISION,
    ];

    /// The code as it appears in the document.
    pub const fn as_str(self) -> &'static str {
        self.0
    }

    /// The location form every diagnostic with this code has.
    pub const fn form(self) -> Form {
        self.1
    }
}

impl fmt::Debug for RuntimeCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RuntimeCode({:?})", self.0)
    }
}

impl fmt::Display for RuntimeCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// One segment of an input path: a name (a scope name, `$event`, or a
/// record field) or a list index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathSegment {
    Name(String),
    Index(usize),
}

impl PartialOrd for PathSegment {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Names by code point, indices numerically. A name and an index never
/// share a position in one path (a value is a record or a list), so how
/// they compare with each other doesn't matter; names come first.
impl Ord for PathSegment {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (PathSegment::Name(a), PathSegment::Name(b)) => a.cmp(b),
            (PathSegment::Index(a), PathSegment::Index(b)) => a.cmp(b),
            (PathSegment::Name(_), PathSegment::Index(_)) => std::cmp::Ordering::Less,
            (PathSegment::Index(_), PathSegment::Name(_)) => std::cmp::Ordering::Greater,
        }
    }
}

/// Where a diagnostic is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Location {
    /// A span in the manifest's text, in bytes.
    Model(mesh_syntax::Span),
    Program,
    /// A template: its position in the host's list, and its component
    /// when it can be read.
    Template {
        index: usize,
        component: Option<String>,
    },
    /// A span in the source of the template of `component`.
    Source {
        component: String,
        span: mesh_template::Span,
    },
    /// A path into the snapshot (from a scope name) or the payload (from
    /// `$event`).
    Input(Vec<PathSegment>),
    Handler,
}

impl Location {
    pub fn form(&self) -> Form {
        match self {
            Location::Model(_) => Form::Model,
            Location::Program => Form::Program,
            Location::Template { .. } => Form::Template,
            Location::Source { .. } => Form::Source,
            Location::Input(_) => Form::Input,
            Location::Handler => Form::Handler,
        }
    }
}

/// One diagnostic. Every one is an error: the runtime has no warnings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDiagnostic {
    /// The code: an assembly or runtime code, or a `manifest-*` code.
    pub code: &'static str,
    pub message: String,
    pub location: Location,
}

impl RuntimeDiagnostic {
    /// A diagnostic with `code`, which must be located in `code`'s form.
    pub fn new(code: RuntimeCode, message: impl Into<String>, location: Location) -> Self {
        assert_eq!(
            code.form(),
            location.form(),
            "{code} is always located at a {}",
            code.form().as_str()
        );
        RuntimeDiagnostic {
            code: code.as_str(),
            message: message.into(),
            location,
        }
    }

    /// A manifest's diagnostic, located in the manifest.
    pub fn from_manifest(diagnostic: &mesh_syntax::Diagnostic) -> Self {
        RuntimeDiagnostic {
            code: diagnostic.code.as_str(),
            message: diagnostic.message.clone(),
            location: Location::Model(diagnostic.span),
        }
    }
}

/// The runtime diagnostics document (`runtime-diagnostics-v1`) for
/// `diagnostics`, in the order given. `model` is the manifest's text,
/// which `model` locations are positions in.
pub fn to_json(diagnostics: &[RuntimeDiagnostic], model: &str) -> String {
    let map = SourceMap::new(model);
    let position = |byte: usize| {
        let at = map.line_column(model, byte, ColumnUnit::Char);
        json!({
            "byte": byte,
            "line": at.line + 1,
            "column": at.column + 1,
            "utf16": map.utf16_offset(model, byte),
            "utf16Column": map.line_column(model, byte, ColumnUnit::Utf16).column + 1,
        })
    };
    let offset =
        |offset: mesh_template::Offset| json!({ "byte": offset.byte, "utf16": offset.utf16 });
    let diagnostics: Vec<Value> = diagnostics
        .iter()
        .map(|diagnostic| {
            let location = match &diagnostic.location {
                Location::Model(span) => json!({
                    "kind": "model",
                    "span": { "start": position(span.start_byte), "end": position(span.end_byte) },
                }),
                Location::Program => json!({ "kind": "program" }),
                Location::Template { index, component } => match component {
                    Some(component) => {
                        json!({ "kind": "template", "index": index, "component": component })
                    }
                    None => json!({ "kind": "template", "index": index }),
                },
                Location::Source { component, span } => json!({
                    "kind": "source",
                    "component": component,
                    "span": { "start": offset(span.start), "end": offset(span.end) },
                }),
                Location::Input(path) => json!({
                    "kind": "input",
                    "path": path.iter().map(|segment| match segment {
                        PathSegment::Name(name) => json!(name),
                        PathSegment::Index(index) => json!(index),
                    }).collect::<Vec<_>>(),
                }),
                Location::Handler => json!({ "kind": "handler" }),
            };
            json!({
                "severity": "error",
                "code": diagnostic.code,
                "message": diagnostic.message,
                "location": location,
            })
        })
        .collect();
    json!({ "version": 1, "diagnostics": diagnostics }).to_string()
}
