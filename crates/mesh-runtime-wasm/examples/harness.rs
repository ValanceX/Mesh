//! The native harness: the runtime module's operations, natively, for
//! the package's parity tests (I11). It reads one JSON request per line
//! on stdin and prints one JSON result per line on stdout, flushing each.
//!
//! A request carries exactly the bytes the package would give the
//! module, base64 where they're binary:
//!
//! - `{"op":"render","root":…,"templates":<b64 text list>,"model":…,"snapshot":<b64 value>}`
//! - `{"op":"dispatch", …render's fields…, "handler":…,"payload":<b64 value>|null}`
//! - `{"op":"numbers","bits":<b64 binary64s>}`
//!
//! A result is `{"status":<0, 1 or 2>,"result":<the module's result, as
//! a string>}`: exactly what the module keeps for `mesh_result_ptr`.
//!
//! ```console
//! $ cargo build -p mesh-runtime-wasm --example harness
//! ```

use mesh_runtime_wasm::{number_texts, respond_dispatch, respond_render, Refusal};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

fn base64(text: &str) -> Vec<u8> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bits = 0u32;
    let mut count = 0;
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    for byte in text.bytes().filter(|&b| b != b'=') {
        let value = ALPHABET.iter().position(|&a| a == byte).expect("base64") as u32;
        bits = (bits << 6) | value;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
        }
    }
    out
}

fn field<'a>(request: &'a Value, name: &str) -> &'a str {
    request[name]
        .as_str()
        .unwrap_or_else(|| panic!("a request's `{name}` is a string"))
}

fn answer(request: &Value) -> (u32, String) {
    let outcome = match field(request, "op") {
        "numbers" => {
            return match number_texts(&base64(field(request, "bits"))) {
                Some(texts) => (0, texts),
                None => (1, String::new()),
            }
        }
        "render" => respond_render(
            field(request, "root"),
            &base64(field(request, "templates")),
            field(request, "model"),
            &base64(field(request, "snapshot")),
        ),
        "dispatch" => respond_dispatch(
            field(request, "root"),
            &base64(field(request, "templates")),
            field(request, "model"),
            &base64(field(request, "snapshot")),
            field(request, "handler"),
            request["payload"].as_str().map(base64).as_deref(),
        ),
        op => panic!("no operation {op}"),
    };
    match outcome {
        Ok(document) => (0, document),
        Err(Refusal::Encoding(_)) => (1, String::new()),
        Err(Refusal::Input(message)) => (2, message),
    }
}

fn main() {
    let stdout = io::stdout();
    for line in io::stdin().lock().lines() {
        let line = line.expect("stdin");
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = serde_json::from_str(&line).expect("a request is JSON");
        let (status, result) = answer(&request);
        let mut out = stdout.lock();
        writeln!(out, "{}", json!({ "status": status, "result": result })).expect("stdout");
        out.flush().expect("stdout");
    }
}
