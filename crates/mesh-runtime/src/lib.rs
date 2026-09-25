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

mod diagnostic;

pub use diagnostic::{to_json, Form, Location, PathSegment, RuntimeCode, RuntimeDiagnostic};
