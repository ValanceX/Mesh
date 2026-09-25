//! The test host and the reference renderer (D10), natively: for each
//! program under `tests/programs/`, the host renders each snapshot
//! fixture in turn, the renderer prints each tree and compares each with
//! the one before, and the host dispatches each scripted event with the
//! render it names. Every result is compared with its committed expected
//! file. `MESH_BLESS=1` rewrites them; review every rewritten file.

mod common;

use common::host::Host;
use common::renderer;
use std::fs;
use std::path::{Path, PathBuf};

fn programs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> =
        fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.is_dir())
            .collect();
    dirs.sort();
    dirs
}

fn sorted_files(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == extension))
        .collect();
    files.sort();
    files
}

/// Compares `actual` with the file at `path`, or writes it when blessing.
fn expect(path: &Path, actual: &str, failures: &mut Vec<String>) {
    if std::env::var_os("MESH_BLESS").is_some() {
        fs::write(path, actual).unwrap();
        return;
    }
    match fs::read_to_string(path) {
        Ok(expected) if expected == actual => {}
        Ok(expected) => failures.push(format!(
            "{}:\n--- expected ---\n{expected}--- actual ---\n{actual}",
            path.display()
        )),
        Err(err) => failures.push(format!("{}: {err}", path.display())),
    }
}

#[test]
fn every_program_renders_and_dispatches_as_committed() {
    let mut failures = Vec::new();
    let dirs = programs();
    assert!(dirs.len() >= 2);
    for dir in dirs {
        let templates = sorted_files(&dir, "mprx")
            .iter()
            .map(|path| {
                let component = path.file_stem().unwrap().to_string_lossy().into_owned();
                common::compile(&component, &fs::read_to_string(path).unwrap())
            })
            .collect();
        let mut host = Host {
            model: common::MODEL.to_string(),
            root: "view".to_string(),
            templates,
            renders: Vec::new(),
        };
        let snapshots = sorted_files(&dir.join("snapshots"), "json");
        for (index, snapshot) in snapshots.iter().enumerate() {
            let render = host
                .render(&fs::read_to_string(snapshot).unwrap())
                .unwrap_or_else(|d| panic!("{}: {d:#?}", snapshot.display()));
            let printed = renderer::print(render.tree());
            expect(&snapshot.with_extension("html"), &printed, &mut failures);
            if index > 0 {
                let changes =
                    renderer::compare(host.renders[index - 1].tree(), host.renders[index].tree());
                expect(&snapshot.with_extension("changes"), &changes, &mut failures);
            }
        }
        let events: Vec<serde_json::Value> =
            serde_json::from_str(&fs::read_to_string(dir.join("events.json")).unwrap()).unwrap();
        let mut results = String::new();
        for event in &events {
            let payload = (!event["payload"].is_null()).then(|| event["payload"].to_string());
            results.push_str(&host.dispatch(
                event["render"].as_u64().unwrap() as usize,
                event["event"].as_str().unwrap(),
                event["occurrence"].as_u64().unwrap() as usize,
                payload.as_deref(),
            ));
            results.push('\n');
        }
        expect(&dir.join("events.out"), &results, &mut failures);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
