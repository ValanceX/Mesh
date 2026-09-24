//! The MESH language server (a placeholder until v0.3's Pass 1).
//!
//! In v0.3 it serves diagnostics and quick fixes, hover, go-to-definition
//! and completion for MPRX. It is a client of `mesh-compiler`, the single
//! semantic authority: its diagnostics are the compiler's own, and it has
//! no parser or type system of its own.
