//! The property test (outline, Definition of Done, "The property test";
//! "Structure, keys and handler identifiers"): for every clean template
//! in the corpus, as a single-template program, and generated snapshots
//! that fit its scope exactly (absent only where a type is optional),
//! every render either yields a tree that matches `render-v1`, or stops
//! with one of D8's evaluation diagnostics whose source is present in the
//! template:
//!
//! - `runtime-non-finite-output`: arithmetic can overflow, or divide by
//!   zero, from finite inputs;
//! - `runtime-absent-element-output`: a list literal can hold an absent
//!   element;
//! - an `any` check: the scope holds `any` somewhere.
//!
//! A template with none of those sources renders every time. All of a
//! template's trees have the same structure, keys and handler
//! identifiers.
//!
//! `MESH_PROPERTY_SNAPSHOTS` (default 100 per template) and
//! `MESH_PROPERTY_SEED`.

mod common;

use mesh_compiler::check;
use mesh_manifest::{Manifest, Type};
use mesh_runtime::{HostKey, HostRecord, HostValue, Node, Program, TreeChild};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

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
    fn chance(&mut self, one_in: usize) -> bool {
        self.below(one_in) == 0
    }
}

fn env(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|err| panic!("should read {}: {err}", path.display()))
}

/// Every `.mprx` directly in `dir`, sorted.
fn sources(dir: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(root().join(dir))
        .unwrap_or_else(|err| panic!("should list {dir}: {err}"))
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "mprx"))
        .collect();
    files.sort();
    files
}

/// Every corpus source with its model's path and component, as
/// `mesh-compiler`'s `compile_template.rs` lists them.
fn corpus() -> Vec<(PathBuf, &'static str, String)> {
    let mut runs = Vec::new();
    for path in sources("examples") {
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        // `user-card.mprx` is the template of `user-card-example`.
        let component = if stem == "user-card" {
            "user-card-example".into()
        } else {
            stem
        };
        runs.push((path, "examples/components.json", component));
    }
    for dir in [
        "examples/fixtures/check/pass",
        "examples/fixtures/check/fail",
    ] {
        for path in sources(dir) {
            runs.push((
                path,
                "examples/fixtures/check/components.json",
                "template".into(),
            ));
        }
    }
    runs
}

fn validator() -> jsonschema::Validator {
    let schema: Value =
        serde_json::from_str(&read(&root().join("schemas/render-v1.schema.json"))).unwrap();
    jsonschema::draft202012::new(&schema).unwrap()
}

const NUMBERS: &[f64] = &[
    0.0,
    -0.0,
    1.0,
    -1.0,
    0.1,
    0.5,
    1.5,
    123.456,
    1e21,
    1e-7,
    9007199254740992.0,
    f64::MAX,
    -f64::MAX,
    f64::MIN_POSITIVE,
    5e-324,
    1e308,
];

const STRINGS: &[&str] = &[
    "",
    "Ada",
    "a b",
    "😀",
    "𝄞 clef",
    "\u{10FFFF}",
    "\0",
    "<script>",
    "\"quoted\"",
    "line\nbreak",
];

fn number(rng: &mut Rng) -> f64 {
    if rng.chance(2) {
        return NUMBERS[rng.below(NUMBERS.len())];
    }
    loop {
        let value = f64::from_bits(rng.next());
        if value.is_finite() {
            return value;
        }
    }
}

fn string(rng: &mut Rng) -> String {
    let mut text = String::new();
    for _ in 0..rng.below(3) + 1 {
        text.push_str(STRINGS[rng.below(STRINGS.len())]);
    }
    text
}

/// A random value the boundary accepts: what `any` may hold.
fn anything(rng: &mut Rng, depth: usize) -> HostValue {
    let kinds = if depth >= 3 { 4 } else { 6 };
    match rng.below(kinds) {
        0 => HostValue::Null,
        1 => HostValue::Boolean(rng.chance(2)),
        2 => HostValue::Number(number(rng)),
        3 => HostValue::String(string(rng)),
        4 => HostValue::List(
            (0..rng.below(4))
                .map(|_| Some(anything(rng, depth + 1)))
                .collect(),
        ),
        _ => HostValue::Record(HostRecord(
            (0..rng.below(4))
                .map(|i| {
                    let name = ["name", "count", "x", "😀", ""][(i + rng.below(5)) % 5];
                    (HostKey::Text(name.into()), anything(rng, depth + 1))
                })
                .collect::<Vec<_>>()
                .into_iter()
                .fold(Vec::new(), |mut fields, field| {
                    if !fields.iter().any(|(key, _)| key == &field.0) {
                        fields.push(field);
                    }
                    fields
                }),
        )),
    }
}

/// A random value that fits `ty` exactly, or `None` (absent), which only
/// an optional type allows.
fn fitting(manifest: &Manifest, rng: &mut Rng, ty: &Type, depth: usize) -> Option<HostValue> {
    match manifest.expand(ty) {
        Type::Optional(inner) => {
            if rng.chance(3) {
                None
            } else {
                fitting(manifest, rng, inner, depth)
            }
        }
        Type::String => Some(HostValue::String(string(rng))),
        Type::Number => Some(HostValue::Number(number(rng))),
        Type::Boolean => Some(HostValue::Boolean(rng.chance(2))),
        Type::Null => Some(HostValue::Null),
        Type::Any => Some(anything(rng, depth)),
        Type::List(element) => {
            let length = if depth >= 3 { 0 } else { rng.below(4) };
            let mut items = Vec::new();
            for _ in 0..length {
                // An absent element can't cross the boundary (§9.8), so a
                // list of an optional type holds only present elements.
                let item = match manifest.expand(element) {
                    Type::Optional(inner) => fitting(manifest, rng, inner, depth + 1),
                    _ => fitting(manifest, rng, element, depth + 1),
                };
                items.push(Some(item.expect("present")));
            }
            Some(HostValue::List(items))
        }
        Type::Record(fields) => {
            let mut record = Vec::new();
            for (name, field) in fields {
                let value = if !field.required && rng.chance(3) {
                    None
                } else {
                    fitting(manifest, rng, &field.ty, depth + 1)
                };
                if let Some(value) = value {
                    record.push((HostKey::Text(name.clone()), value));
                }
            }
            shuffle(rng, &mut record);
            Some(HostValue::Record(HostRecord(record)))
        }
        Type::Named(_) => unreachable!("expanded"),
    }
}

fn shuffle<T>(rng: &mut Rng, items: &mut [T]) {
    for i in (1..items.len()).rev() {
        items.swap(i, rng.below(i + 1));
    }
}

fn contains_any(manifest: &Manifest, ty: &Type, seen: &mut BTreeSet<String>) -> bool {
    match ty {
        Type::Any => true,
        Type::Optional(inner) | Type::List(inner) => contains_any(manifest, inner, seen),
        Type::Record(fields) => fields
            .values()
            .any(|field| contains_any(manifest, &field.ty, seen)),
        Type::Named(name) => {
            seen.insert(name.clone()) && contains_any(manifest, &manifest.types()[name], seen)
        }
        Type::String | Type::Number | Type::Boolean | Type::Null => false,
    }
}

/// Which of the evaluation diagnostics' sources the template holds,
/// outside handlers (handlers aren't evaluated at render).
#[derive(Default, Debug)]
struct Sources {
    arithmetic: bool,
    list: bool,
}

fn sources_in(value: &Value, found: &mut Sources) {
    match value {
        Value::Object(object) => {
            match (object.get("kind"), object.get("operator")) {
                (Some(Value::String(kind)), Some(Value::String(op)))
                    if kind == "binary"
                        && ["add", "subtract", "multiply", "divide", "remainder"]
                            .contains(&op.as_str()) =>
                {
                    found.arithmetic = true;
                }
                (Some(Value::String(kind)), _) if kind == "list" => found.list = true,
                _ => {}
            }
            for (key, value) in object {
                if key != "events" {
                    sources_in(value, found);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| sources_in(item, found)),
        _ => {}
    }
}

const ANY_CHECKS: &[&str] = &[
    "runtime-operand-mismatch",
    "runtime-not-a-record",
    "runtime-missing-member",
    "runtime-content-not-text",
    "runtime-prop-mismatch",
];

/// A tree without its values: keys, components, handler identifiers and
/// children, one line per node or text run.
fn structure(node: &Node, depth: usize, out: &mut String) {
    out.push_str(&format!(
        "{:depth$}{} {} {:?}\n",
        "", node.key, node.component, node.events
    ));
    for child in &node.children {
        match child {
            TreeChild::Node(child) => structure(child, depth + 1, out),
            TreeChild::Text { key, .. } => {
                out.push_str(&format!("{:1$}{key} text\n", "", depth + 1))
            }
        }
    }
}

/// A program under test, and what it may stop at.
struct Case<'a> {
    label: String,
    model: &'a str,
    manifest: &'a Manifest,
    root: &'a str,
    templates: Vec<String>,
}

#[derive(Default)]
struct Counts {
    renders: u64,
    stops: u64,
}

/// The evaluation diagnostics `case` may stop at: those whose source is
/// present in one of its templates (outside handlers), or, for the `any`
/// checks, in the scope of one of its templates' components.
fn allowed(case: &Case<'_>) -> Vec<&'static str> {
    let mut found = Sources::default();
    let mut any = false;
    for template in &case.templates {
        let json: Value = serde_json::from_str(template).unwrap();
        sources_in(&json, &mut found);
        let component = json["component"].as_str().unwrap();
        any |= case.manifest.components()[component]
            .scope
            .values()
            .any(|ty| contains_any(case.manifest, ty, &mut BTreeSet::new()));
    }
    let mut allowed = Vec::new();
    if found.arithmetic {
        allowed.push("runtime-non-finite-output");
    }
    if found.list {
        allowed.push("runtime-absent-element-output");
    }
    if any {
        allowed.extend(ANY_CHECKS);
    }
    allowed
}

/// Renders `snapshots` generated snapshots of `case`'s root's scope, and
/// checks every result.
fn run(
    case: &Case<'_>,
    snapshots: u64,
    rng: &mut Rng,
    validator: &jsonschema::Validator,
    counts: &mut Counts,
) {
    let allowed = allowed(case);
    let texts: Vec<&str> = case.templates.iter().map(String::as_str).collect();
    let program = Program {
        root: case.root,
        templates: &texts,
    };
    let scope = &case.manifest.components()[case.root].scope;
    let label = &case.label;
    let mut shape: Option<String> = None;
    for _ in 0..snapshots {
        let mut fields: Vec<(HostKey, HostValue)> = scope
            .iter()
            .filter_map(|(name, ty)| {
                fitting(case.manifest, rng, ty, 0).map(|v| (HostKey::Text(name.clone()), v))
            })
            .collect();
        shuffle(rng, &mut fields);
        let snapshot = HostRecord(fields);
        match mesh_runtime::render(&program, case.model, &snapshot) {
            Ok(render) => {
                counts.renders += 1;
                let document: Value = serde_json::from_str(&render.tree().to_json()).unwrap();
                assert!(validator.is_valid(&document), "{label}: {document}");
                let mut this = String::new();
                structure(&render.tree().root, 0, &mut this);
                match &shape {
                    None => shape = Some(this),
                    Some(shape) => {
                        assert_eq!(shape, &this, "{label}: every render has one structure")
                    }
                }
            }
            Err(diagnostics) => {
                counts.stops += 1;
                assert_eq!(
                    diagnostics.len(),
                    1,
                    "{label}: evaluation reports one error: {diagnostics:#?}\n{snapshot:?}"
                );
                assert!(
                    allowed.contains(&diagnostics[0].code),
                    "{label}: {} has no source in the program (allowed {allowed:?}): {diagnostics:#?}\n{snapshot:?}",
                    diagnostics[0].code
                );
            }
        }
    }
}

#[test]
fn generated_snapshots_render_or_stop_at_a_present_source() {
    let snapshots = env("MESH_PROPERTY_SNAPSHOTS", 100);
    let seed = env("MESH_PROPERTY_SEED", 0x4d45_5348_7072_6f70);
    let mut rng = Rng(seed);
    let validator = validator();
    let (mut programs, mut refused, mut counts) = (0, 0, Counts::default());
    for (path, model_path, component) in corpus() {
        let model_text = read(&root().join(model_path));
        let model = check::Model::load(&model_text, &component).expect("the model loads");
        let compiled = check::template(&read(&path), &model);
        let Some(template) = compiled.template else {
            continue;
        };
        let manifest = mesh_manifest::load(&model_text).expect("the manifest loads");
        let case = Case {
            label: path.display().to_string(),
            model: &model_text,
            manifest: &manifest,
            root: &component,
            templates: vec![mesh_template::to_json(&template)],
        };
        // A template that uses its own component (`examples/page.mprx`)
        // isn't a program on its own: assembly refuses it, whatever the
        // snapshot.
        let texts: Vec<&str> = case.templates.iter().map(String::as_str).collect();
        let refusal = mesh_runtime::check_program(
            &Program {
                root: &component,
                templates: &texts,
            },
            &model_text,
        );
        if !refusal.is_empty() {
            let codes: Vec<&str> = refusal.iter().map(|d| d.code).collect();
            assert_eq!(codes, ["assembly-cycle"], "{}", path.display());
            refused += 1;
            continue;
        }
        programs += 1;
        run(&case, snapshots, &mut rng, &validator, &mut counts);
    }
    assert!(programs >= 20, "only {programs} corpus programs compiled");
    println!(
        "property: {programs} programs × {snapshots} snapshots of seed {seed}: \
         {} rendered, {} stopped at a present source; \
         {refused} refused at assembly",
        counts.renders, counts.stops
    );
}

/// The same property over programs of several templates: every program
/// under `tests/programs/`, each `.mprx` the template of the component it
/// names, against the tests' model, with `view` as the root.
#[test]
fn generated_snapshots_of_composed_programs() {
    let snapshots = env("MESH_PROPERTY_SNAPSHOTS", 100);
    let seed = env("MESH_PROPERTY_SEED", 0x4d45_5348_7072_6f70);
    let mut rng = Rng(seed);
    let validator = validator();
    let model_text = common::MODEL.to_string();
    let manifest = mesh_manifest::load(&model_text).expect("the manifest loads");
    let mut counts = Counts::default();
    let mut composed = 0;
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs");
    let mut dirs: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    for dir in dirs {
        let templates: Vec<String> = sources(&dir.to_string_lossy())
            .iter()
            .map(|path| {
                let component = path.file_stem().unwrap().to_string_lossy().into_owned();
                let model = check::Model::load(&model_text, &component).unwrap();
                let compiled = check::template(&read(path), &model);
                mesh_template::to_json(&compiled.template.expect("a program's template compiles"))
            })
            .collect();
        composed += usize::from(templates.len() > 1);
        let case = Case {
            label: dir.display().to_string(),
            model: &model_text,
            manifest: &manifest,
            root: "view",
            templates,
        };
        run(&case, snapshots, &mut rng, &validator, &mut counts);
    }
    assert!(composed >= 1, "no program of several templates");
    println!(
        "property (composed): {} rendered, {} stopped at a present source",
        counts.renders, counts.stops
    );
}
