//! The MESH compiler for WebAssembly: what `@valancex/mesh-compiler` runs.
//!
//! It has no semantics of its own (outline v0.4 I6). [`respond`] is the
//! check operation, [`mesh_compiler::check::run`], rendered as the
//! diagnostics document; everything else only moves bytes across the
//! boundary between WebAssembly's linear memory and JavaScript.
//!
//! Build it with Cargo alone (`clang` compiles Tree-sitter's C, through
//! `cc`, as for every target):
//!
//! ```console
//! $ cargo build -p mesh-wasm --target wasm32-unknown-unknown --profile wasm
//! ```
//!
//! **The exports are internal** (outline D11): only the package's wrapper
//! calls them, and any release may change them, except
//! `mesh_version_ptr` and `mesh_version_len`, which never change, so any
//! wrapper can always read any module's version and refuse one that
//! isn't its own (I10). They exist only in a `wasm32` build.
//!
//! **A panic is a trap.** The `wasm` profile builds with
//! `panic = "abort"`, which is all `wasm32-unknown-unknown` offers:
//! nothing here recovers from one. The wrapper observes the trap, reports
//! it, and never calls that instance again (outline D6).

use mesh_compiler::check::{self, ModelInput, Request};

/// Runs one check and renders its report: exactly
/// `check::run(request).render_json(request)`, for the request made of
/// `source` at `path` and, if given, a model of `(manifest, manifest
/// path, component)`. Paths only name documents; nothing reads them.
pub fn respond(source: &str, path: &str, model: Option<(&str, &str, &str)>) -> String {
    let mut request = Request::new(source, path);
    if let Some((manifest, manifest_path, component)) = model {
        request = request.with_model(ModelInput::new(manifest, manifest_path, component));
    }
    check::run(&request).render_json(&request)
}

#[cfg(target_arch = "wasm32")]
mod exports;
