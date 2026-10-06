//! The MESH runtime for WebAssembly: what `@valancex/mesh-runtime` runs.
//!
//! It has no semantics of its own (I11). [`respond_render`] is
//! [`mesh_runtime::render`], [`respond_dispatch`] is
//! [`mesh_runtime::dispatch_from`], [`respond_declared_events`] is
//! [`mesh_runtime::declared_events`] and [`respond_update`] is
//! [`mesh_runtime::update`], each with its inputs decoded from
//! the byte encoding (`mesh_runtime::encoding`) and its result rendered
//! as one JSON document; everything else only moves bytes across the
//! boundary between WebAssembly's linear memory and JavaScript. It links
//! no parser: a host that renders needs no compiler.
//!
//! **Its one state between calls is the renders `update` keeps,** so that
//! the next `update` from one needn't derive it again: a table of handles
//! (`respond_update` makes one, `release` ends it). `render`, `dispatch`
//! and `declared_events` keep nothing.
//!
//! Build it with Cargo alone:
//!
//! ```console
//! $ cargo build -p mesh-runtime-wasm --target wasm32-unknown-unknown --profile wasm
//! ```
//!
//! **The exports are internal**: only the package's wrapper calls them,
//! and any release may change them, except `mesh_version_ptr` and
//! `mesh_version_len`, which never change, so any wrapper can always read
//! any module's version and refuse one that isn't its own (I10).
//!
//! **A panic is a trap.** The `wasm` profile builds with
//! `panic = "abort"`. The wrapper observes the trap, reports it, and
//! never calls that instance again.

use mesh_runtime::encoding::{decode_snapshot, decode_texts, decode_value, EncodingError};
use mesh_runtime::Program;

/// Why a call gave no result document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The bytes aren't the encoding: the wrapper's fault. Status 1.
    Encoding(String),
    /// The host's value can't be taken at all: a snapshot that isn't a
    /// record, or a value nested too deeply. Status 2; the wrapper
    /// reports it as a `TypeError` with this message.
    Input(String),
}

impl From<EncodingError> for Refusal {
    fn from(error: EncodingError) -> Refusal {
        match error {
            EncodingError::Malformed(_) => Refusal::Encoding(error.to_string()),
            EncodingError::TooDeep | EncodingError::NotARecord => Refusal::Input(error.to_string()),
        }
    }
}

/// Renders the program of `root` and the encoded `templates` against
/// `model` and the encoded `snapshot`: `{"tree": <render-v1>}`, or
/// `{"diagnostics": <runtime-diagnostics-v1>}`.
pub fn respond_render(
    root: &str,
    templates: &[u8],
    model: &str,
    snapshot_bytes: &[u8],
) -> Result<String, Refusal> {
    let templates = decode_texts(templates)?;
    let snapshot = decode_snapshot(snapshot_bytes)?;
    let texts: Vec<&str> = templates.iter().map(String::as_str).collect();
    let program = Program {
        root,
        templates: &texts,
    };
    Ok(match mesh_runtime::render(&program, model, &snapshot) {
        Ok(render) => format!("{{\"tree\":{}}}", render.tree().to_json()),
        Err(diagnostics) => format!(
            "{{\"diagnostics\":{}}}",
            mesh_runtime::to_json(&diagnostics, model)
        ),
    })
}

/// Renders the module holds for `update`, by handle. This is the module's
/// only state between calls. A handle is made by [`respond_update`], never
/// reused in this module's life, and ends at [`release`]; a module that is
/// replaced (the wrapper discards one after a trap) takes its handles with it.
mod retained {
    use mesh_runtime::Render;
    use std::cell::{Cell, RefCell};
    use std::collections::BTreeMap;

    thread_local! {
        static TABLE: RefCell<BTreeMap<u32, Render>> = const { RefCell::new(BTreeMap::new()) };
        static NEXT: Cell<u32> = const { Cell::new(1) };
    }

    pub(super) fn keep(render: Render) -> u32 {
        let handle = NEXT.with(|next| {
            let handle = next.get();
            next.set(handle.checked_add(1).expect("handles don't run out"));
            handle
        });
        TABLE.with(|table| table.borrow_mut().insert(handle, render));
        handle
    }

    /// Runs `use_it` on the render `handle` names, in place: a render is
    /// never copied out of the table.
    pub(super) fn with<T>(handle: u32, use_it: impl FnOnce(&Render) -> T) -> Option<T> {
        TABLE.with(|table| table.borrow().get(&handle).map(use_it))
    }

    pub(super) fn release(handle: u32) {
        TABLE.with(|table| table.borrow_mut().remove(&handle));
    }

    pub(super) fn count() -> usize {
        TABLE.with(|table| table.borrow().len())
    }
}

/// Releases the render `handle` names. A handle that isn't kept (already
/// released, or never made) is ignored.
pub fn release(handle: u32) {
    retained::release(handle);
}

/// How many renders the module holds. For the package's memory tests.
pub fn retained_renders() -> usize {
    retained::count()
}

/// Updates to the encoded `snapshot`: `{"handle": <n>, "tree": <render-v1>,
/// "patches": <render-patch-v1>}`, or `{"diagnostics":
/// <runtime-diagnostics-v1>}`.
///
/// The previous render is the one `previous_handle` names, if the module
/// holds it. Otherwise the module derives it from the render's own inputs
/// as the wrapper kept them (the encoded `previous_bytes`), so a handle that
/// was released, or came from a module since replaced, costs a render more
/// and gives the same result. The new render is kept under a new handle, which
/// the caller releases; the previous render stays kept (and valid) until the
/// caller releases its handle, so a host can still dispatch with it until its
/// renderer has applied the patches. A refused update keeps nothing.
pub fn respond_update(
    root: &str,
    templates: &[u8],
    model: &str,
    previous_handle: Option<u32>,
    previous_bytes: &[u8],
    snapshot_bytes: &[u8],
) -> Result<String, Refusal> {
    let templates = decode_texts(templates)?;
    let snapshot = decode_snapshot(snapshot_bytes)?;
    let result = match previous_handle.and_then(|handle| {
        retained::with(handle, |previous| mesh_runtime::update(previous, &snapshot))
    }) {
        Some(result) => result,
        None => {
            let previous_snapshot = decode_snapshot(previous_bytes)?;
            let texts: Vec<&str> = templates.iter().map(String::as_str).collect();
            let program = Program {
                root,
                templates: &texts,
            };
            mesh_runtime::render(&program, model, &previous_snapshot)
                .and_then(|previous| mesh_runtime::update(&previous, &snapshot))
        }
    };
    Ok(match result {
        Ok(updated) => {
            let (tree, patches) = (
                updated.render.tree().to_json(),
                mesh_runtime::patches_to_json(&updated.patches),
            );
            let handle = retained::keep(updated.render);
            format!("{{\"handle\":{handle},\"tree\":{tree},\"patches\":{patches}}}")
        }
        Err(diagnostics) => format!(
            "{{\"diagnostics\":{}}}",
            mesh_runtime::to_json(&diagnostics, model)
        ),
    })
}

/// Dispatches `handler` with the encoded `payload` (absent when `None`)
/// against a render's inputs, as the wrapper kept them: `{"intent":
/// <intent>}`, or `{"diagnostics": <runtime-diagnostics-v1>}`.
pub fn respond_dispatch(
    root: &str,
    templates: &[u8],
    model: &str,
    snapshot_bytes: &[u8],
    handler: &str,
    payload_bytes: Option<&[u8]>,
) -> Result<String, Refusal> {
    let templates = decode_texts(templates)?;
    let snapshot = decode_snapshot(snapshot_bytes)?;
    let payload = payload_bytes.map(decode_value).transpose()?;
    let texts: Vec<&str> = templates.iter().map(String::as_str).collect();
    let program = Program {
        root,
        templates: &texts,
    };
    Ok(
        match mesh_runtime::dispatch_from(&program, model, &snapshot, handler, payload.as_ref()) {
            Ok(intent) => format!("{{\"intent\":{}}}", intent.to_json()),
            Err(diagnostics) => format!(
                "{{\"diagnostics\":{}}}",
                mesh_runtime::to_json(&diagnostics, model)
            ),
        },
    )
}

/// The events declared by the program of `root` and the encoded
/// `templates`, against `model`: `{"events": [...]}`, each event as
/// [`mesh_runtime::declared_events_to_json`] writes it, or
/// `{"diagnostics": <runtime-diagnostics-v1>}`. It takes no snapshot.
pub fn respond_declared_events(
    root: &str,
    templates: &[u8],
    model: &str,
) -> Result<String, Refusal> {
    let templates = decode_texts(templates)?;
    let texts: Vec<&str> = templates.iter().map(String::as_str).collect();
    let program = Program {
        root,
        templates: &texts,
    };
    Ok(match mesh_runtime::declared_events(&program, model) {
        Ok(events) => format!(
            "{{\"events\":{}}}",
            mesh_runtime::declared_events_to_json(&events)
        ),
        Err(diagnostics) => format!(
            "{{\"diagnostics\":{}}}",
            mesh_runtime::to_json(&diagnostics, model)
        ),
    })
}

/// MPRX's text for each number in `bits` (8 bytes each, little-endian
/// binary64 bits), one per line: [`mesh_runtime::number_to_text`], for
/// the package's number tests. `None` if `bits` isn't whole numbers, or
/// holds a non-finite one, which has no text.
pub fn number_texts(bits: &[u8]) -> Option<String> {
    let (words, rest) = bits.as_chunks::<8>();
    if !rest.is_empty() {
        return None;
    }
    let mut out = String::with_capacity(bits.len() * 3);
    for word in words {
        let number = f64::from_bits(u64::from_le_bytes(*word));
        if !number.is_finite() {
            return None;
        }
        out.push_str(&mesh_runtime::number_to_text(number));
        out.push('\n');
    }
    Some(out)
}

#[cfg(target_arch = "wasm32")]
mod exports;
