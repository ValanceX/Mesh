//! The MESH runtime (docs/manual/runtime.md): MPRX's one evaluator.
//!
//! It renders a **program** of templates (`mesh-template`) against a
//! host's **snapshot**, producing a render tree for a renderer to draw,
//! and turns the events a renderer reports into **command intents** for
//! the host. Its whole surface is two operations, render and dispatch.
//!
//! It implements the spec's §9.7 (evaluation) and §9.8 (the boundary)
//! exactly once (I11). It accepts no MPRX source, only templates (I12),
//! resolves no names (every resolution is in the templates), and uses the
//! model only to validate values. It validates every input it is given,
//! on every call (I14). It has no I/O, no clock, no randomness and no
//! global state: identical inputs give identical results.

mod boundary;
mod diagnostic;
mod dispatch;
pub mod encoding;
mod eval;
mod number;
mod program;
mod render;
mod tree;
mod types;
mod value;

pub use diagnostic::{to_json, Form, Location, PathSegment, RuntimeCode, RuntimeDiagnostic};
pub use dispatch::{dispatch, dispatch_from};
pub use number::number_to_text;
pub use program::Program;
pub use render::{render, Render};
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
