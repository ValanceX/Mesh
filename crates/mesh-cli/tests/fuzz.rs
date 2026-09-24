//! A seeded mutation fuzzer over every entry point that takes untrusted
//! text (outline D7, as amended after the third audit, C11): the parser,
//! the compiler with and without a model, editor recovery, completion, the
//! offset queries, and the language server's request handlers.
//!
//! It mutates every `.mprx` file under `examples/` with a small,
//! dependency-free PRNG, so a failure reproduces exactly from its seed.
//! CI runs the default budget. For a long run, in release:
//!
//! ```console
//! $ MESH_FUZZ_ITERATIONS=1000000 MESH_FUZZ_SEED=1 cargo test --release -p mesh-cli --test fuzz -- --nocapture
//! ```

use mesh_compiler::{compile, compile_with, editor, ColumnUnit, CompileOptions, SourceMap};
use mesh_lsp::testing::{file_uri, test_options, Client};
use mesh_manifest::Manifest;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// The default number of mutants: a few seconds in a debug build.
const DEFAULT_ITERATIONS: u64 = 2_000;

/// The default seed. Any fixed value keeps CI deterministic.
const DEFAULT_SEED: u64 = 0x4d45_5348_5f46_555a;

/// LSP's `ContentModified` error code.
const CONTENT_MODIFIED: i32 = -32801;

/// One in this many mutants also goes through a language server.
const SERVER_EVERY: u64 = 50;

/// What a mutation may insert: MPRX's tokens and delimiters, names the
/// example manifest declares, and text that stresses positions
/// (multi-byte and astral characters, a byte-order mark, `\r\n`).
const TOKENS: &[&str] = &[
    "<",
    ">",
    "/>",
    "</",
    "{",
    "}",
    "(",
    ")",
    "[",
    "]",
    ".",
    ",",
    ":",
    "?",
    "\"",
    "=",
    "!",
    "-",
    "+",
    "*",
    "&&",
    "||",
    "==",
    "<=",
    "on.",
    "on.click",
    "$event",
    "$",
    "\\",
    "\\\"",
    " ",
    "\n",
    "\t",
    "\r\n",
    "\r",
    "user",
    "user.",
    "avatar",
    "page",
    "text",
    "selectUser",
    "compact",
    "name",
    "null",
    "true",
    "1.5",
    "é",
    "😀",
    "\u{feff}",
    "\u{200b}",
];

/// xorshift64: small, fast, and the same everywhere.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// A number in `0..n` (`0` when `n` is `0`).
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        (self.next() % n as u64) as usize
    }
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .map(|value| {
            value
                .parse()
                .unwrap_or_else(|_| panic!("{name} must be a number, not {value:?}"))
        })
        .unwrap_or(default)
}

fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

/// Every `.mprx` file under `examples/`, sorted, so the corpus is the
/// same on every machine.
fn corpus() -> Vec<String> {
    let mut files = Vec::new();
    let mut dirs = vec![examples_dir()];
    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(&dir).expect("the directory reads") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|ext| ext == "mprx") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
        .iter()
        .map(|path| fs::read_to_string(path).expect("the file reads"))
        .collect()
}

/// `index` floored to a character boundary of `text`.
fn floor(text: &str, index: usize) -> usize {
    let mut index = index.min(text.len());
    while !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

/// One to six edits of `seed`: an insertion, a deletion of up to eight
/// bytes, or a truncation, each at a character boundary.
fn mutate(rng: &mut Rng, seed: &str) -> String {
    let mut text = seed.to_string();
    for _ in 0..=rng.below(6) {
        let at = floor(&text, rng.below(text.len() + 1));
        match rng.below(5) {
            0..=2 => text.insert_str(at, TOKENS[rng.below(TOKENS.len())]),
            3 => {
                let end = floor(&text, at + 1 + rng.below(8)).max(at);
                text.replace_range(at..end, "");
            }
            _ => text.truncate(at),
        }
    }
    text
}

/// Offsets to query: any byte, including inside a character and past
/// the end, which every API must floor or clamp.
fn offsets(rng: &mut Rng, text: &str) -> [usize; 8] {
    let mut offsets = [0; 8];
    for offset in &mut offsets {
        *offset = rng.below(text.len() + 4);
    }
    offsets[0] = text.len();
    offsets
}

/// Runs `text` through every library entry point.
fn through_the_library(manifest: &Manifest, text: &str, offsets: &[usize]) {
    let template = manifest.template("users-page").expect("declared");
    let _ = mesh_parser::parse(text);
    let _ = mesh_parser::recover(text);
    let _ = compile(text);
    let compiled = compile_with(text, &CompileOptions::with_template(template));
    let recovery = editor::recover(text, template);
    let map = SourceMap::new(text);
    for &offset in offsets {
        let _ = mesh_parser::context_at(text, offset);
        let _ = editor::candidates(text, offset, template, compiled.analysis.as_ref());
        let _ = editor::candidates(text, offset, template, None);
        let _ = recovery.resolution_at(offset);
        let _ = recovery.typed_at(offset);
        if let Some(analysis) = &compiled.analysis {
            let _ = analysis.resolution_at(offset);
            let _ = analysis.typed_at(offset);
            let _ = analysis.facts_at(offset);
        }
        for unit in [ColumnUnit::Char, ColumnUnit::Utf8, ColumnUnit::Utf16] {
            let at = map.line_column(text, offset, unit);
            let _ = map.offset(text, at, unit);
        }
    }
}

/// Sends `text` to the server as a change, then asks hover, definition
/// and completion at a few offsets: every answer is a result or
/// `ContentModified`, never an internal error.
fn through_the_server(client: &mut Client, uri: &str, version: i32, text: &str, offsets: &[usize]) {
    client.change(uri, version, text);
    let map = SourceMap::new(text);
    for &offset in offsets.iter().take(3) {
        let at = map.line_column(text, offset, ColumnUnit::Utf16);
        let params = json!({
            "textDocument": { "uri": uri },
            "position": { "line": at.line, "character": at.column },
        });
        for method in [
            "textDocument/hover",
            "textDocument/definition",
            "textDocument/completion",
        ] {
            let response = client.request(method, params.clone());
            if let Err(error) = response.response_result {
                assert_eq!(
                    error.code, CONTENT_MODIFIED,
                    "{method} at {offset}: {error:?}"
                );
            }
        }
    }
}

#[test]
fn mutated_examples_never_panic() {
    let iterations = env_u64("MESH_FUZZ_ITERATIONS", DEFAULT_ITERATIONS);
    let seed = env_u64("MESH_FUZZ_SEED", DEFAULT_SEED);
    // The server's compile thread has an 8 MiB stack; so does this one.
    let run = std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(move || fuzz(iterations, seed))
        .expect("the fuzz thread starts");
    if let Err(payload) = run.join() {
        std::panic::resume_unwind(payload);
    }
}

fn fuzz(iterations: u64, seed: u64) {
    let corpus = corpus();
    assert!(corpus.len() >= 40, "only {} files", corpus.len());
    let manifest_text =
        fs::read_to_string(examples_dir().join("components.json")).expect("the manifest reads");
    let manifest = mesh_manifest::load(&manifest_text).expect("the example manifest loads");

    let workspace = tempfile::tempdir().expect("a temp dir");
    fs::write(workspace.path().join("components.json"), &manifest_text).expect("writes");
    let uri = file_uri(&workspace.path().join("page.mprx"));
    let (mut client, _) = Client::initialized_with(
        test_options(),
        json!({
            "processId": null,
            "rootUri": file_uri(workspace.path()),
            "capabilities": {},
            "initializationOptions": {
                "model": "components.json",
                "components": { "page.mprx": "users-page" }
            },
        }),
    );
    client.open(&uri, 0, "");
    let mut version = 0;

    let started = Instant::now();
    let mut rng = Rng::new(seed);
    for iteration in 0..iterations {
        let pick = rng.below(corpus.len());
        let text = mutate(&mut rng, &corpus[pick]);
        let offsets = offsets(&mut rng, &text);
        let to_server = iteration % SERVER_EVERY == 0;
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            through_the_library(&manifest, &text, &offsets);
        }));
        if outcome.is_err() {
            panic!(
                "a panic on mutant {iteration} of seed {seed}; reproduce with \
                 MESH_FUZZ_SEED={seed} MESH_FUZZ_ITERATIONS={}. The mutant:\n{text:?}",
                iteration + 1
            );
        }
        if to_server {
            version += 1;
            through_the_server(&mut client, &uri, version, &text, &offsets);
        }
    }
    eprintln!(
        "fuzz: {iterations} mutants of seed {seed} in {:.1?}, {} through the server",
        started.elapsed(),
        iterations.div_ceil(SERVER_EVERY)
    );
    assert_eq!(
        client.exit(true),
        mesh_lsp::Outcome::Exited {
            after_shutdown: true
        }
    );
}
