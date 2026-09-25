//! The MESH runtime for WebAssembly: what `@valancex/mesh-runtime` runs.
//!
//! It has no semantics of its own (I11). [`respond_render`] is
//! [`mesh_runtime::render`] and [`respond_dispatch`] is
//! [`mesh_runtime::dispatch_from`], each with its inputs decoded from
//! the byte encoding (`mesh_runtime::encoding`) and its result rendered
//! as one JSON document; everything else only moves bytes across the
//! boundary between WebAssembly's linear memory and JavaScript. It links
//! no parser: a host that renders needs no compiler.
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
