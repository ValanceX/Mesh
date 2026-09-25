//! A seeded fuzzer over render and dispatch: mutated MPRX (compiled when
//! it still compiles), mutated template JSON, snapshots with hostile
//! values, mutated roots and template lists, forged handler identifiers
//! and hostile payloads. Every call must return a result or diagnostics,
//! never panic, and what it returns must match its schema.
//!
//! `MESH_FUZZ_ITERATIONS` (default 300) and `MESH_FUZZ_SEED` as in the
//! compiler's fuzzer.

mod common;

use mesh_runtime::{HostKey, HostRecord, HostValue, Program};
use serde_json::Value;
use std::fs;
use std::path::Path;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn env(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn validator(schema: &str) -> jsonschema::Validator {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas")
        .join(schema);
    let schema: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    jsonschema::draft202012::new(&schema).unwrap()
}

const TOKENS: &[&str] = &[
    "<",
    ">",
    "/>",
    "{",
    "}",
    "(",
    ")",
    "[",
    "]",
    "=",
    "\"",
    ".",
    ",",
    ":",
    "?",
    "!",
    "-",
    "+",
    "*",
    "/",
    "%",
    "==",
    "&&",
    "||",
    "on.tap",
    "$event",
    "user",
    "name",
    "count",
    "anything",
    "maybeName",
    "1",
    "0",
    "1.5",
    "true",
    "null",
    " ",
    "é",
    "😀",
    "<probe",
    "<card user={user}",
    "</page>",
    "{[",
    "{a: ",
];

fn mutate_text(rng: &mut Rng, text: &str) -> String {
    let mut chars: Vec<char> = text.chars().collect();
    for _ in 0..=rng.below(4) {
        let at = rng.below(chars.len() + 1);
        match rng.below(3) {
            0 => {
                let token = TOKENS[rng.below(TOKENS.len())];
                chars.splice(at..at, token.chars());
            }
            1 if at < chars.len() => {
                let end = (at + 1 + rng.below(6)).min(chars.len());
                chars.drain(at..end);
            }
            _ => chars.truncate(at),
        }
    }
    chars.into_iter().collect()
}

fn hostile(rng: &mut Rng, depth: usize) -> HostValue {
    match rng.below(if depth > 2 { 9 } else { 12 }) {
        0 => HostValue::Null,
        1 => HostValue::Boolean(rng.below(2) == 0),
        2 => HostValue::Number(f64::from_bits(rng.next())),
        3 => HostValue::Number((rng.below(200) as f64) - 100.0),
        4 => HostValue::String("Ada".into()),
        5 => HostValue::Utf16(vec![0xd800]),
        6 => HostValue::OutOfRange("1e999".into()),
        7 => HostValue::Unsupported(
            ["function", "symbol", "cycle", "object:Map"][rng.below(4)].into(),
        ),
        8 => HostValue::Number(f64::NAN),
        9 => HostValue::List(
            (0..rng.below(3))
                .map(|_| (rng.below(4) > 0).then(|| hostile(rng, depth + 1)))
                .collect(),
        ),
        _ => HostValue::Record(HostRecord(
            (0..rng.below(4))
                .map(|i| {
                    let key = ["name", "active", "avatar", "x", "deep"][i % 5];
                    (HostKey::Text(key.into()), hostile(rng, depth + 1))
                })
                .collect(),
        )),
    }
}

fn mutate_snapshot(rng: &mut Rng, record: &HostRecord) -> HostRecord {
    let mut record = record.clone();
    for _ in 0..rng.below(3) {
        if record.0.is_empty() {
            break;
        }
        let at = rng.below(record.0.len());
        match rng.below(3) {
            0 => {
                record.0.remove(at);
            }
            _ => record.0[at].1 = hostile(rng, 0),
        }
    }
    record
}

#[test]
fn render_and_dispatch_never_panic() {
    let iterations = env("MESH_FUZZ_ITERATIONS", 300);
    let seed = env("MESH_FUZZ_SEED", 0x4d45_5348_7275_6e74);
    let mut rng = Rng(seed | 1);
    let render_schema = validator("render-v1.schema.json");
    let diagnostics_schema = validator("runtime-diagnostics-v1.schema.json");
    let programs = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs");
    let sources: Vec<(String, String)> =
        ["greeting/view.mprx", "cards/view.mprx", "cards/card.mprx"]
            .iter()
            .map(|file| {
                let component = Path::new(file)
                    .file_stem()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                (component, fs::read_to_string(programs.join(file)).unwrap())
            })
            .collect();
    let base = common::snapshot(common::SNAPSHOT);
    let card = common::compile("card", &sources[2].1);
    let mut rendered = 0;
    for iteration in 0..iterations {
        // A program: mutated MPRX, compiled if it still compiles, or a
        // mutated template's JSON.
        let (component, source) = &sources[rng.below(2)];
        let mut template = {
            let mutated = mutate_text(&mut rng, source);
            let model = mesh_compiler::check::Model::load(common::MODEL, component).unwrap();
            match mesh_compiler::check::template(&mutated, &model).template {
                Some(template) => mesh_template::to_json(&template),
                None => common::compile(component, source),
            }
        };
        if rng.below(4) == 0 {
            template = mutate_text(&mut rng, &template);
        }
        // The program itself: sometimes another root, or a template list
        // with one dropped, repeated or out of order.
        let mut templates = vec![template.as_str(), card.as_str()];
        match rng.below(8) {
            0 => {
                templates.remove(rng.below(templates.len()));
            }
            1 => templates.push(templates[rng.below(templates.len())]),
            2 => templates.reverse(),
            _ => {}
        }
        let root = if rng.below(6) == 0 {
            ["card", "page", "nope", "", "view "][rng.below(5)]
        } else {
            "view"
        };
        let program = Program {
            root,
            templates: &templates,
        };
        let snapshot = mutate_snapshot(&mut rng, &base);
        let outcome =
            std::panic::catch_unwind(|| mesh_runtime::render(&program, common::MODEL, &snapshot));
        let Ok(result) = outcome else {
            panic!("render panicked on iteration {iteration} of seed {seed}:\n{template}\n{snapshot:?}");
        };
        match result {
            Err(diagnostics) => {
                let document: Value =
                    serde_json::from_str(&mesh_runtime::to_json(&diagnostics, common::MODEL))
                        .unwrap();
                assert!(diagnostics_schema.is_valid(&document), "{document}");
            }
            Ok(render) => {
                rendered += 1;
                let document: Value = serde_json::from_str(&render.tree().to_json()).unwrap();
                assert!(render_schema.is_valid(&document), "{document}");
                // Dispatch every handler, and some forged ones, with hostile payloads.
                let mut handlers = Vec::new();
                collect(&render.tree().root, &mut handlers);
                handlers.push(mutate_text(
                    &mut rng,
                    handlers.first().map_or("h", String::as_str),
                ));
                for handler in handlers {
                    let payload = (rng.below(3) > 0).then(|| hostile(&mut rng, 0));
                    let outcome = std::panic::catch_unwind(|| {
                        mesh_runtime::dispatch(&render, &handler, payload.as_ref())
                    });
                    let Ok(result) = outcome else {
                        panic!("dispatch panicked on iteration {iteration} of seed {seed}: {handler} {payload:?}\n{template}");
                    };
                    if let Err(diagnostics) = result {
                        let document: Value = serde_json::from_str(&mesh_runtime::to_json(
                            &diagnostics,
                            common::MODEL,
                        ))
                        .unwrap();
                        assert!(diagnostics_schema.is_valid(&document), "{document}");
                    }
                }
            }
        }
    }
    assert!(rendered > 0, "no mutant rendered");
    println!("fuzz: {iterations} programs of seed {seed}, {rendered} rendered");
}

fn collect(node: &mesh_runtime::Node, out: &mut Vec<String>) {
    out.extend(node.events.values().cloned());
    for child in &node.children {
        if let mesh_runtime::TreeChild::Node(child) = child {
            collect(child, out);
        }
    }
}
