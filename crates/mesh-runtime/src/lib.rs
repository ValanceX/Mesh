//! The MESH runtime (docs/manual/runtime.md): MPRX's one evaluator.
//!
//! It renders a **program** of templates (`mesh-template`) against a
//! host's **snapshot**, producing a render tree for a renderer to draw,
//! and turns the events a renderer reports into **command intents** for
//! the host. Its surface is render, dispatch, `update` (render again,
//! reusing what is unchanged, and say what changed as patches) and
//! `declared_events`, and `resolve`, the reference implementation of event resolution
//! (§9.9), a pure function of a render tree that renderers implement for
//! their own targets.
//!
//! It implements the spec's §9.7 (evaluation) and §9.8 (the boundary)
//! exactly once (I11). It accepts no MPRX source, only templates (I12),
//! resolves no names (every resolution is in the templates), and uses the
//! model only to validate values. It validates every input it is given,
//! on every call (I14). It has no I/O, no clock, no randomness and no
//! global state: identical inputs give identical results.

mod boundary;
mod changes;
mod diagnostic;
mod dispatch;
pub mod encoding;
mod eval;
mod number;
mod patch;
mod program;
mod render;
mod resolve;
mod tree;
mod types;
mod value;

pub use changes::{Change, Changes};
pub use diagnostic::{to_json, Form, Location, PathSegment, RuntimeCode, RuntimeDiagnostic};
pub use dispatch::{dispatch, dispatch_from};
#[doc(hidden)]
pub use eval::evaluations;
pub use number::number_to_text;
pub use patch::{diff, patches_to_json, Patch};
pub use program::{declared_events_to_json, DeclaredEvent, Program};
pub use render::{render, update, update_changes, update_with, Render, Update};
pub use resolve::{resolve, Interaction, ResolveError, Resolved};
pub use tree::{Intent, Node, Tree, TreeChild};
pub use value::{HostKey, HostRecord, HostValue};

/// Validates `program` against `model` as render and dispatch do before
/// anything else (D3's program validation: the model, then the templates,
/// then their fingerprints, then the assembly rules), and returns what it
/// finds: empty when the program is valid. The compiler's program check
/// is this.
pub fn check_program(program: &Program<'_>, model: &str) -> Vec<RuntimeDiagnostic> {
    program::validate(program, model).err().unwrap_or_default()
}

/// Validates `program` against `model` exactly as [`check_program`] does,
/// and, if it is valid, returns every event binding its templates declare
/// (see [`DeclaredEvent`]); otherwise returns [`check_program`]'s
/// diagnostics, unchanged.
///
/// The declarations are the validated program's own: templates of every
/// component the host supplied, composites included, and every element of
/// each, so an event in an inactive `mesh-if` alternative or in a
/// `mesh-each` body is there. It takes no snapshot and no handler.
pub fn declared_events(
    program: &Program<'_>,
    model: &str,
) -> Result<Vec<DeclaredEvent>, Vec<RuntimeDiagnostic>> {
    program::validate(program, model).map(|valid| program::declared_events(&valid))
}
