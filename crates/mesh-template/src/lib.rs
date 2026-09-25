//! The MESH template format, `template-v1` (docs/manual/templates.md).
//!
//! A **template** is one component's MPRX, checked clean against a
//! component model and compiled: every name resolved to the declaration
//! it names, every literal converted, and no values. The MESH compiler
//! writes templates, and the MESH runtime reads them.
//!
//! This crate is the format and nothing else:
//!
//! - the template's types ([`Template`] and the tree under it);
//! - [`from_json`], which reads a template and refuses one that is
//!   malformed or of an unsupported version ([`Refusal`]), and
//!   [`to_json`], which writes one;
//! - [`fingerprint`], the model fingerprint of a manifest: the one
//!   implementation the compiler (which writes it) and the runtime
//!   (which checks it) share.
//!
//! It knows nothing of MPRX source, and nothing of evaluation: it has no
//! parser, so the runtime can depend on it without one, and it checks
//! only what a template must be on its own. Whether a template's names
//! are declared by a particular model is the runtime's check.

mod fingerprint;
mod read;
mod types;
mod write;

pub use fingerprint::{fingerprint, Fingerprint, ParseFingerprintError};
pub use read::{from_json, Problem, Refusal};
pub use types::{
    BinaryOperator, Child, Element, EventBinding, Expression, Field, Literal, Offset, Prop, Span,
    Template, UnaryOperator,
};
pub use write::to_json;

/// The template format version this crate reads and writes.
pub const VERSION: u64 = 1;

/// The value of every template's `format` property.
pub const FORMAT: &str = "mesh-template";
