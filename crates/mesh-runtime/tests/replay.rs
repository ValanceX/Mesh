//! Replay (Bet 15, R1 to R15): the session logs in `examples/conformance/replay/` and 1,000 random sessions, natively.
//!
//! A log is line-JSON (`examples/conformance/replay/README.md`). This file is the native player: it
//! renders the header's snapshot and replays the log by all three routes (an `update_changes` chain, an
//! `update` chain over whole snapshots, and a `render` of each snapshot, never `update`), and requires
//! every route to give the recorded tree hashes and the recorded outcome of every event. The same logs
//! are replayed by `packages/mesh-runtime/test/replay.test.mjs` through the WebAssembly package.
//!
//! `MESH_BLESS=1` records each directory's `scenario.json` into its `log.jsonl` (and the random
//! sessions' hashes into `random.expected.json`); review every rewritten file. Never by hand.

use mesh_compiler::check;
use mesh_runtime::{
    dispatch, render, update, update_changes, Changes, HostRecord, HostValue, Program, Render,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/conformance/replay")
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|err| panic!("should read {}: {err}", path.display()))
}

fn sha(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn blessing() -> bool {
    std::env::var_os("MESH_BLESS").is_some()
}

/// A replay program: its sources (for the header's hash), compiled templates and model.
struct Loaded {
    model: String,
    root: String,
    templates: Vec<String>,
    program_hash: String,
    manifest_hash: String,
}

fn load(name: &str) -> Loaded {
    let here = dir().join(name);
    let model = read(&dir().join("components.json"));
    let program: Value = serde_json::from_str(&read(&here.join("program.json"))).unwrap();
    let root = program["root"].as_str().unwrap().to_string();
    let mut sources = String::new();
    let mut templates = Vec::new();

    for component in program["templates"].as_array().unwrap() {
        let component = component.as_str().unwrap();
        let source = read(&here.join(format!("{component}.mprx")));
        let loaded = check::Model::load(&model, component).expect("the manifest loads");
        let compiled = check::template(&source, &loaded);

        assert!(
            compiled.diagnostics.is_empty(),
            "{name}/{component}: {:#?}",
            compiled.diagnostics
        );
        templates.push(mesh_template::to_json(
            &compiled.template.expect("it compiles"),
        ));
        sources.push_str(&source);
        sources.push('\0');
    }

    Loaded {
        program_hash: sha(&sources),
        manifest_hash: sha(&model),
        model,
        root,
        templates,
    }
}

fn texts(loaded: &Loaded) -> Vec<&str> {
    loaded.templates.iter().map(String::as_str).collect()
}

fn start(loaded: &Loaded, snapshot: &Value) -> Render {
    let texts = texts(loaded);

    render(
        &Program {
            root: &loaded.root,
            templates: &texts,
        },
        &loaded.model,
        &HostRecord::from_json(&snapshot.to_string()).expect("a record"),
    )
    .unwrap_or_else(|diagnostics| panic!("the snapshot renders: {diagnostics:#?}"))
}

/// The hash of a tree: SHA-256 of the runtime's compact JSON.
fn hash(render: &Render) -> String {
    sha(&render.tree().to_json())
}

fn tree_value(render: &Render) -> Value {
    serde_json::from_str(&render.tree().to_json()).unwrap()
}

/// Applies host-side changes to a plain snapshot: `set`, `insert` and `remove` at a path.
fn apply(snapshot: &mut Value, changes: &[Value]) {
    for change in changes {
        let path = change["path"].as_array().unwrap();
        let (last, parents) = path.split_last().unwrap();
        let mut at = &mut *snapshot;

        for step in parents {
            at = match step {
                Value::String(name) => &mut at[name.as_str()],
                other => &mut at[other.as_u64().unwrap() as usize],
            };
        }

        match (change["op"].as_str().unwrap(), last) {
            ("set", Value::String(name)) => at[name.as_str()] = change["value"].clone(),
            ("set", index) => at[index.as_u64().unwrap() as usize] = change["value"].clone(),
            ("insert", index) => at
                .as_array_mut()
                .unwrap()
                .insert(index.as_u64().unwrap() as usize, change["value"].clone()),
            ("remove", Value::String(name)) => {
                at.as_object_mut().unwrap().remove(name.as_str());
            }
            ("remove", index) => {
                at.as_array_mut()
                    .unwrap()
                    .remove(index.as_u64().unwrap() as usize);
            }
            (other, _) => panic!("the change {other}"),
        }
    }
}

fn changes_of(render: &Render, changes: &[Value]) -> Changes {
    let document = json!({ "base": render.version(), "changes": changes });

    Changes::from_host(&HostValue::from_json(&document.to_string()).unwrap())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"))
}

/// What a dispatch gave, as the log writes it: `{ "intent": … }` or `{ "diag": code }`.
fn outcome(render: &Render, handler: &str, payload: Option<&Value>) -> Value {
    let payload = payload.map(|value| HostValue::from_json(&value.to_string()).unwrap());

    match dispatch(render, handler, payload.as_ref()) {
        Ok(intent) => {
            json!({ "intent": serde_json::from_str::<Value>(&intent.to_json()).unwrap() })
        }
        Err(diagnostics) => json!({ "diag": diagnostics[0].code }),
    }
}

/// The handler of `name` on the `nth` node of `component` in `tree`, depth first.
fn handler_in(tree: &Value, component: &str, nth: usize, name: &str) -> String {
    fn walk<'a>(node: &'a Value, component: &str, found: &mut Vec<&'a Value>) {
        if node["type"] == "node" {
            if node["component"] == component {
                found.push(node);
            }

            for child in node["children"].as_array().unwrap() {
                walk(child, component, found);
            }
        }
    }

    let mut found = Vec::new();

    walk(&tree["root"], component, &mut found);

    found[nth]["events"][name]
        .as_str()
        .unwrap_or_else(|| panic!("{component} #{nth} has no {name}"))
        .to_string()
}

const CHECKPOINT: usize = 50;

fn tree_line(render: &Render, lines: usize, last: bool) -> Value {
    let mut line = json!({ "line": "tree", "sha256": hash(render) });

    if last || lines.is_multiple_of(CHECKPOINT) {
        line["tree"] = tree_value(render);
    }

    line
}

/// Records `scenario.json` of `name` as the lines of its log, by the `update_changes` route.
fn record(name: &str) -> Vec<Value> {
    let loaded = load(name);
    let scenario: Value =
        serde_json::from_str(&read(&dir().join(name).join("scenario.json"))).unwrap();
    let mut snapshot = scenario["snapshot"].clone();
    let mut current = start(&loaded, &snapshot);
    let mut renders = vec![current.clone()];
    let mut trees = vec![tree_value(&current)];
    let mut lines = vec![json!({
        "line": "header",
        "format": 1,
        "runtime": env!("CARGO_PKG_VERSION"),
        "program": loaded.program_hash,
        "manifest": loaded.manifest_hash,
        "snapshot": snapshot,
    })];
    let mut sequence = 0;

    lines.push(tree_line(&current, lines.len() + 1, false));

    for step in scenario["steps"].as_array().unwrap() {
        if let Some(changes) = step.get("changes") {
            let changes = changes.as_array().unwrap();
            let rejects = step.get("rejects").is_some();
            let outcome = update_changes(&current, &changes_of(&current, changes), None);

            let mut line = json!({ "line": "changes", "changes": changes });

            match (outcome, rejects) {
                (Ok(next), false) => {
                    apply(&mut snapshot, changes);
                    current = next.render;
                    lines.push(line);
                    renders.push(current.clone());
                    trees.push(tree_value(&current));
                }
                (Err(diagnostics), true) => {
                    line["rejects"] = json!(diagnostics[0].code);
                    lines.push(line);
                }
                (Ok(_), true) => panic!("{name}: the changes were to be rejected"),
                (Err(diagnostics), false) => panic!("{name}: {diagnostics:#?}"),
            }

            let at = lines.len() + 1;

            lines.push(tree_line(&current, at, false));
        } else {
            let event = &step["event"];
            let ordinal = |value: &Value| value.as_u64().map_or(renders.len() - 1, |n| n as usize);
            let on = ordinal(&event["on"]);
            let handler = match event.get("handler") {
                Some(handler) => handler.as_str().unwrap().to_string(),
                None => {
                    let find = &event["find"];

                    handler_in(
                        &trees[ordinal(&find["tree"])],
                        find["component"].as_str().unwrap(),
                        find["nth"].as_u64().unwrap() as usize,
                        event["name"].as_str().unwrap(),
                    )
                }
            };

            sequence += 1;

            let payload = event.get("payload");
            let mut line = json!({
                "line": "event",
                "n": sequence,
                "on": on,
                "handler": handler,
                "out": outcome(&renders[on], &handler, payload),
            });

            if let Some(payload) = payload {
                line["payload"] = payload.clone();
            }

            lines.push(line);
        }
    }

    // the whole last tree, at the end
    let last = lines.len();

    lines.push(tree_line(&current, last, true));

    lines
}

/// One line as the log writes it: `line` first, the other members in key order.
fn line_text(line: &Value) -> String {
    let mut rest = line.as_object().unwrap().clone();
    let kind = rest.remove("line").unwrap();
    let rest = Value::Object(rest).to_string();

    if rest == "{}" {
        format!("{{\"line\":{kind}}}")
    } else {
        format!("{{\"line\":{kind},{}", &rest[1..])
    }
}

fn log_text(lines: &[Value]) -> String {
    lines
        .iter()
        .map(|line| format!("{}\n", line_text(line)))
        .collect()
}

/// Plays `lines` by one route; returns the tree hashes it gave, in order. `route` is `changes`, `update` or `render`.
fn play(name: &str, lines: &[Value], route: &str) -> Vec<String> {
    let loaded = load(name);
    let header = &lines[0];

    assert_eq!(
        header["program"], loaded.program_hash,
        "{name}: the program is the one recorded"
    );
    assert_eq!(
        header["manifest"], loaded.manifest_hash,
        "{name}: the manifest is the one recorded"
    );

    let mut snapshot = header["snapshot"].clone();
    let mut current = start(&loaded, &snapshot);
    let mut renders = vec![current.clone()];
    let mut hashes = Vec::new();

    for (index, line) in lines.iter().enumerate().skip(1) {
        match line["line"].as_str().unwrap() {
            "changes" => {
                let changes = line["changes"].as_array().unwrap();
                let mut next_snapshot = snapshot.clone();

                apply(&mut next_snapshot, changes);

                let result = match route {
                    "changes" => update_changes(&current, &changes_of(&current, changes), None),
                    "update" => update(
                        &current,
                        &HostRecord::from_json(&next_snapshot.to_string()).unwrap(),
                    ),
                    _ => {
                        let texts = texts(&loaded);

                        render(
                            &Program {
                                root: &loaded.root,
                                templates: &texts,
                            },
                            &loaded.model,
                            &HostRecord::from_json(&next_snapshot.to_string()).unwrap(),
                        )
                        .map(|render| mesh_runtime::Update {
                            render,
                            patches: Vec::new(),
                        })
                    }
                };

                match (result, line.get("rejects")) {
                    (Ok(next), None) => {
                        snapshot = next_snapshot;
                        current = next.render;
                        renders.push(current.clone());
                    }
                    (Err(diagnostics), Some(code)) => {
                        assert_eq!(
                            diagnostics[0].code,
                            code.as_str().unwrap(),
                            "{name} line {}: {route}",
                            index + 1
                        );
                    }
                    (Ok(_), Some(code)) => panic!(
                        "{name} line {}: {route} accepted changes to be rejected with {code}",
                        index + 1
                    ),
                    (Err(diagnostics), None) => panic!(
                        "{name} line {}: {route} rejected: {diagnostics:#?}",
                        index + 1
                    ),
                }
            }
            "event" => {
                let on = line["on"].as_u64().unwrap() as usize;
                let out = outcome(
                    &renders[on],
                    line["handler"].as_str().unwrap(),
                    line.get("payload"),
                );

                assert_eq!(
                    out,
                    line["out"],
                    "{name} line {}: {route} dispatches as recorded",
                    index + 1
                );
            }
            "tree" => {
                assert_eq!(
                    hash(&current),
                    line["sha256"],
                    "{name} line {}: {route} gives the recorded tree",
                    index + 1
                );

                if let Some(tree) = line.get("tree") {
                    assert_eq!(
                        tree_value(&current),
                        *tree,
                        "{name} line {}: {route}'s checkpoint",
                        index + 1
                    );
                }

                hashes.push(hash(&current));
            }
            other => panic!("{name}: a line of kind {other}"),
        }
    }

    hashes
}

fn names() -> Vec<String> {
    let mut found: Vec<String> = fs::read_dir(dir())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join("scenario.json").is_file())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect();

    found.sort();
    found
}

fn lines_of(text: &str) -> Vec<Value> {
    text.lines()
        .map(|line| serde_json::from_str(line).expect("a line of JSON"))
        .collect()
}

#[test]
fn there_are_at_least_six_logs() {
    assert!(names().len() >= 6, "{:?}", names());
}

#[test]
fn every_log_is_what_its_scenario_records() {
    for name in names() {
        let path = dir().join(&name).join("log.jsonl");
        let mut lines = record(&name);

        // `runtime` says which version recorded the log; it is information, and a release does not rewrite every log
        if let (false, Ok(old)) = (blessing(), fs::read_to_string(&path)) {
            lines[0]["runtime"] = lines_of(&old)[0]["runtime"].clone();
        }

        let recorded = log_text(&lines);

        if blessing() {
            fs::write(&path, &recorded).unwrap();
        } else {
            assert_eq!(
                read(&path),
                recorded,
                "{name}: log.jsonl is what scenario.json records (MESH_BLESS=1 rewrites it)"
            );
        }
    }
}

#[test]
fn every_log_replays_by_all_three_routes_to_the_recorded_hashes_and_outcomes() {
    for name in names() {
        let lines = lines_of(&read(&dir().join(&name).join("log.jsonl")));
        let by_changes = play(&name, &lines, "changes");

        assert_eq!(
            play(&name, &lines, "update"),
            by_changes,
            "{name}: update chain"
        );
        assert_eq!(
            play(&name, &lines, "render"),
            by_changes,
            "{name}: render of each snapshot"
        );
        assert!(by_changes.len() >= 2, "{name} has trees");
    }
}

/// A recorded log whose tree hash was changed must not replay (the check can fail).
#[test]
fn a_log_with_a_wrong_hash_does_not_replay() {
    let name = names()[0].clone();
    let mut lines = lines_of(&read(&dir().join(&name).join("log.jsonl")));

    lines[1]["sha256"] = json!("0".repeat(64));

    let outcome = std::panic::catch_unwind(|| play(&name, &lines, "changes"));

    assert!(outcome.is_err());
}

// ---- random sessions ------------------------------------------------------------------------------------------------------------------------------------------------------------

/// mulberry32: the generator both runtimes' tests use (the JavaScript one is `Math.imul` and `>>> 0` for the same steps).
struct Random(u32);

impl Random {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_add(0x6D2B_79F5);

        let mut t = self.0;

        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));

        t ^ (t >> 14)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() as usize) % n
    }
}

const SESSIONS: usize = 1000;
const STEPS: usize = 12;
const SEED: u32 = 20_261_017;

/// A random session of the keyed list: the initial snapshot and the changes of each step. Only host-side data, from the seed.
fn session(index: usize) -> (Value, Vec<Vec<Value>>) {
    let mut random = Random(SEED.wrapping_add((index as u32).wrapping_mul(7919)));
    let mut next_id = 0;
    let mut fresh = |random: &mut Random| {
        next_id += 1;

        json!({ "id": next_id, "label": format!("n{}-{}", next_id, random.below(1000)) })
    };
    let count = 1 + random.below(6);
    let mut items: Vec<Value> = (0..count).map(|_| fresh(&mut random)).collect();
    let initial = json!({ "items": items.clone() });
    let mut steps = Vec::new();

    for _ in 0..STEPS {
        let kind = if items.is_empty() {
            1
        } else if items.len() > 10 {
            2
        } else {
            random.below(5)
        };
        let changes = match kind {
            0 => {
                let at = random.below(items.len());
                let label = format!("v{}", random.below(1000));

                items[at]["label"] = json!(label);
                vec![json!({ "op": "set", "path": ["items", at, "label"], "value": label })]
            }
            1 => {
                let at = random.below(items.len() + 1);
                let item = fresh(&mut random);

                items.insert(at, item.clone());
                vec![json!({ "op": "insert", "path": ["items", at], "value": item })]
            }
            2 => {
                let at = random.below(items.len());

                items.remove(at);
                vec![json!({ "op": "remove", "path": ["items", at] })]
            }
            3 => {
                let at = random.below(items.len() + 1);
                let item = fresh(&mut random);

                items.insert(at, item.clone());

                let other = random.below(items.len());
                let label = format!("w{}", random.below(1000));

                items[other]["label"] = json!(label);
                vec![
                    json!({ "op": "insert", "path": ["items", at], "value": item }),
                    json!({ "op": "set", "path": ["items", other, "label"], "value": label }),
                ]
            }
            _ => {
                let k = random.below(5);

                items = (0..k).map(|_| fresh(&mut random)).collect();
                vec![json!({ "op": "set", "path": ["items"], "value": items.clone() })]
            }
        };

        steps.push(changes);
    }

    (initial, steps)
}

/// The digest of one random session by `route`: SHA-256 of its tree hashes, first 16 hexadecimal digits.
fn digest(index: usize, route: &str, loaded: &Loaded) -> String {
    let (initial, steps) = session(index);
    let mut snapshot = initial.clone();
    let mut current = start(loaded, &snapshot);
    let mut hashes = vec![hash(&current)];

    for changes in steps {
        apply(&mut snapshot, &changes);

        current = match route {
            "changes" => {
                update_changes(&current, &changes_of(&current, &changes), None)
                    .expect("changes apply")
                    .render
            }
            "update" => {
                update(
                    &current,
                    &HostRecord::from_json(&snapshot.to_string()).unwrap(),
                )
                .expect("update")
                .render
            }
            _ => start(loaded, &snapshot),
        };
        hashes.push(hash(&current));
    }

    sha(&hashes.join("\n"))[..16].to_string()
}

#[test]
fn a_thousand_random_sessions_agree_by_the_three_routes_and_with_the_committed_digests() {
    let loaded = load("keyed-list");
    let digests: Vec<String> = (0..SESSIONS)
        .map(|index| {
            let by_changes = digest(index, "changes", &loaded);

            assert_eq!(
                digest(index, "update", &loaded),
                by_changes,
                "session {index}: update chain"
            );
            assert_eq!(
                digest(index, "render", &loaded),
                by_changes,
                "session {index}: render of each snapshot"
            );

            by_changes
        })
        .collect();
    let document = json!({
        "program": "keyed-list",
        "generator": "mulberry32",
        "seed": SEED,
        "sessions": SESSIONS,
        "steps": STEPS,
        "digests": digests,
    });
    let text = serde_json::to_string_pretty(&document).unwrap() + "\n";
    let path = dir().join("random.expected.json");

    if blessing() {
        fs::write(&path, &text).unwrap();
    } else {
        assert_eq!(
            read(&path),
            text,
            "the random sessions give the committed digests (MESH_BLESS=1 rewrites them)"
        );
    }
}
