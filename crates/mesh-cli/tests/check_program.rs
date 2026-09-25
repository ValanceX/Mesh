//! `mesh check-program` and `mesh_compiler::check::program` (v0.5 D2,
//! D7): a program's manifest, templates, fingerprints and assembly rules,
//! with exactly the diagnostics, codes and locations render gives.
//!
//! Each directory under `tests/check-program/` is a case: `program.json`
//! names the root and the templates in order. A `.mprx` entry is compiled
//! against `components.json` (or the entry's own `model`) as the template
//! of the component its name gives; a `.json` entry is used as written.
//! `checkModel` names the manifest the program is checked against, when
//! it isn't `components.json`. The human output is pinned by
//! `expected.stderr`, the JSON by `expected.json`; `MESH_BLESS=1`
//! rewrites both. `@valancex/mesh-compiler`'s `checkProgram()` test
//! reads the same cases.

use assert_cmd::Command;
use mesh_compiler::check;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

fn cases_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/check-program")
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|err| panic!("should read {}: {err}", path.display()))
}

/// A case, laid out in a temporary directory as the CLI will read it.
struct Case {
    name: String,
    dir: tempfile::TempDir,
    model: String,
    root: String,
    /// The template files, relative to `dir`, in the program's order.
    files: Vec<String>,
}

impl Case {
    fn load(case: &Path) -> Case {
        let program: Value = serde_json::from_str(&read(&case.join("program.json"))).unwrap();
        let shared = read(&cases_dir().join("components.json"));
        let dir = tempfile::tempdir().unwrap();
        let mut files = Vec::new();
        for entry in program["templates"].as_array().unwrap() {
            let (source, model) = match entry {
                Value::String(source) => (source.as_str(), None),
                entry => (entry["source"].as_str().unwrap(), entry["model"].as_str()),
            };
            if let Some(stem) = source.strip_suffix(".mprx") {
                let manifest = model.map_or_else(|| shared.clone(), |m| read(&case.join(m)));
                let model = check::Model::load(&manifest, stem).expect("the model loads");
                let compiled = check::template(&read(&case.join(source)), &model);
                let template = compiled
                    .template
                    .unwrap_or_else(|| panic!("{source} compiles: {:#?}", compiled.diagnostics));
                let file = format!("{stem}.template.json");
                fs::write(dir.path().join(&file), mesh_template::to_json(&template)).unwrap();
                files.push(file);
            } else {
                fs::write(dir.path().join(source), read(&case.join(source))).unwrap();
                files.push(source.to_string());
            }
        }
        let model = program["checkModel"]
            .as_str()
            .unwrap_or("components.json")
            .to_string();
        let manifest = if model == "components.json" {
            shared
        } else {
            read(&case.join(&model))
        };
        fs::write(dir.path().join(&model), manifest).unwrap();
        Case {
            name: case.file_name().unwrap().to_string_lossy().into_owned(),
            dir,
            model,
            root: program["root"].as_str().unwrap().to_string(),
            files,
        }
    }

    fn texts(&self) -> (String, Vec<String>) {
        let model = read(&self.dir.path().join(&self.model));
        let templates = self
            .files
            .iter()
            .map(|file| read(&self.dir.path().join(file)))
            .collect();
        (model, templates)
    }

    fn run(&self, json: bool) -> std::process::Output {
        let mut command = Command::cargo_bin("mesh").unwrap();
        command
            .current_dir(self.dir.path())
            .args([
                "check-program",
                "--model",
                &self.model,
                "--root",
                &self.root,
            ])
            .args(&self.files);
        if json {
            command.args(["--format", "json"]);
        }
        command.output().unwrap()
    }
}

fn cases() -> Vec<Case> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(cases_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs.iter().map(|dir| Case::load(dir)).collect()
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

/// Each case's codes, and the location form of each: the rule it breaks.
const EXPECTED: &[(&str, &[(&str, &str)])] = &[
    ("broken-model", &[("manifest-unknown-type", "model")]),
    (
        "composite-children",
        &[
            ("assembly-composite-children", "source"),
            ("assembly-composite-children", "source"),
        ],
    ),
    ("composite-event", &[("assembly-composite-event", "source")]),
    ("cycle", &[("assembly-cycle", "source")]),
    (
        "duplicate-template",
        &[("assembly-duplicate-template", "source")],
    ),
    (
        "fingerprint-mismatch",
        &[("assembly-fingerprint-mismatch", "template")],
    ),
    (
        "malformed-template",
        &[("assembly-malformed-template", "template")],
    ),
    ("missing-root", &[("assembly-missing-root", "program")]),
    (
        "unbound-scope-name",
        &[("assembly-unbound-scope-name", "source")],
    ),
    (
        "unsound-binding",
        &[
            ("assembly-unsound-binding", "source"),
            ("assembly-unsound-binding", "source"),
        ],
    ),
    (
        "unsupported-version",
        &[("assembly-unsupported-format-version", "template")],
    ),
    ("valid", &[]),
];

#[test]
fn every_case_reports_its_rule_the_same_way_everywhere() {
    let mut failures = Vec::new();
    let cases = cases();
    let names: Vec<&str> = cases.iter().map(|case| case.name.as_str()).collect();
    let expected: Vec<&str> = EXPECTED.iter().map(|(name, _)| *name).collect();
    assert_eq!(names, expected, "every case has its expectation");
    for (case, (_, codes)) in cases.iter().zip(EXPECTED) {
        let (model, templates) = case.texts();
        let texts: Vec<&str> = templates.iter().map(String::as_str).collect();
        let diagnostics = check::program(&model, &case.root, &texts);
        let document = mesh_runtime::to_json(&diagnostics, &model);

        // The rule, at its code's location form.
        let parsed: Value = serde_json::from_str(&document).unwrap();
        let found: Vec<(&str, &str)> = parsed["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| {
                (
                    d["code"].as_str().unwrap(),
                    d["location"]["kind"].as_str().unwrap(),
                )
            })
            .collect();
        assert_eq!(&found, codes, "{}", case.name);

        // Render reports exactly the same, before looking at any input.
        let rendered = mesh_runtime::render(
            &mesh_runtime::Program {
                root: &case.root,
                templates: &texts,
            },
            &model,
            &mesh_runtime::HostRecord::default(),
        );
        if !diagnostics.is_empty() {
            let rendered = rendered.expect_err("an invalid program doesn't render");
            assert_eq!(
                mesh_runtime::to_json(&rendered, &model),
                document,
                "{}",
                case.name
            );
        }

        // The CLI, as JSON: the same document.
        let json = case.run(true);
        let stdout = String::from_utf8(json.stdout).unwrap();
        assert_eq!(stdout, format!("{document}\n"), "{}", case.name);
        assert_eq!(
            json.status.success(),
            diagnostics.is_empty(),
            "{}",
            case.name
        );
        expect(
            &cases_dir().join(&case.name).join("expected.json"),
            &stdout,
            &mut failures,
        );

        // The CLI, for people.
        let human = case.run(false);
        let stdout = String::from_utf8(human.stdout).unwrap();
        let stderr = String::from_utf8(human.stderr).unwrap();
        assert_eq!(
            human.status.success(),
            diagnostics.is_empty(),
            "{}",
            case.name
        );
        if diagnostics.is_empty() {
            assert_eq!(stdout, "no errors\n");
            assert_eq!(stderr, "");
        } else {
            assert_eq!(stdout, "", "{}", case.name);
            expect(
                &cases_dir().join(&case.name).join("expected.stderr"),
                &stderr,
                &mut failures,
            );
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn an_unreadable_template_is_reported_and_nothing_is_checked() {
    let case = Case::load(&cases_dir().join("valid"));
    let output = Command::cargo_bin("mesh")
        .unwrap()
        .current_dir(case.dir.path())
        .args([
            "check-program",
            "--model",
            "components.json",
            "--root",
            "view",
            "view.template.json",
            "missing.template.json",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "");
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .starts_with("error: could not read missing.template.json: "));
}

/// The CLI manual's example is the `cycle` case's output.
#[test]
fn the_manuals_example_is_the_cycle_case() {
    let manual = read(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/manual/mesh-cli.md"));
    let expected = read(&cases_dir().join("cycle/expected.stderr"));
    let program: Value =
        serde_json::from_str(&read(&cases_dir().join("cycle/program.json"))).unwrap();
    assert_eq!(
        program["templates"],
        serde_json::json!(["view.mprx", "card.mprx"])
    );
    let example = format!(
        "$ mesh check-program --model components.json --root view view.template.json card.template.json\n{expected}```"
    );
    assert!(manual.contains(&example), "the manual shows:\n{example}");
}
