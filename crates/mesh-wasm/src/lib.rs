//! The MESH compiler for WebAssembly: what `@valancex/mesh-compiler` runs.
//!
//! It has no semantics of its own (outline v0.4 I6). [`respond`] is the
//! check operation, [`mesh_compiler::check::run`], rendered as the
//! diagnostics document, [`respond_compile`] is compiling,
//! [`mesh_compiler::check::template`], rendered with its template, and
//! [`respond_check_program`] is the program check,
//! [`mesh_compiler::check::program`];
//! everything else only moves bytes across the boundary between
//! WebAssembly's linear memory and JavaScript.
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

/// Runs one compile and renders its result: the diagnostics document
/// `mesh compile --format json` prints, and the template it writes, as
/// `{"diagnostics": <document>, "template": <template-v1> | null}`.
///
/// A manifest that doesn't load is reported against the manifest, as
/// `mesh check` reports it, with no template. Otherwise this is exactly
/// `check::template`, its diagnostics rendered against the source.
pub fn respond_compile(
    source: &str,
    path: &str,
    manifest: &str,
    manifest_path: &str,
    component: &str,
) -> String {
    let (document, template) = match check::Model::load(manifest, component) {
        Err(diagnostics) => (
            mesh_compiler::render_json(manifest, manifest_path, &diagnostics),
            None,
        ),
        Ok(model) => {
            let compiled = check::template(source, &model);
            (
                mesh_compiler::render_json(source, path, &compiled.diagnostics),
                compiled.template,
            )
        }
    };
    let template = template.map_or_else(|| "null".to_string(), |t| mesh_template::to_json(&t));
    format!("{{\"diagnostics\":{document},\"template\":{template}}}")
}

/// Runs one program check and renders its result: the runtime
/// diagnostics document `mesh check-program --format json` prints, for
/// the program of `root` and the templates `templates` encodes (a text
/// list, `mesh_runtime::encoding`), against `manifest`. `None` if
/// `templates` isn't a text list.
pub fn respond_check_program(manifest: &str, root: &str, templates: &[u8]) -> Option<String> {
    let templates = mesh_runtime::encoding::decode_texts(templates).ok()?;
    let texts: Vec<&str> = templates.iter().map(String::as_str).collect();
    let diagnostics = check::program(manifest, root, &texts);
    Some(mesh_runtime::to_json(&diagnostics, manifest))
}

#[cfg(target_arch = "wasm32")]
mod exports;
