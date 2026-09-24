//! `mesh_compiler::editor`: recovery for editors, and the properties the
//! recovery spec requires of it (P1 and P2).

use mesh_analysis::Target;
use mesh_compiler::{compile, compile_with, editor, CompileOptions};
use std::fs;
use std::path::{Path, PathBuf};

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

fn mprx_files(dir: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(examples().join(dir))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "mprx"))
        .collect();
    files.sort();
    files
}

/// Every template in the corpus with its manifest and component.
fn corpus() -> Vec<(PathBuf, String, String)> {
    let example_manifest = read(&examples().join("components.json"));
    let check_manifest = read(&examples().join("fixtures/check/components.json"));
    let mut runs = vec![
        (
            examples().join("users-page.mprx"),
            example_manifest.clone(),
            "users-page".to_string(),
        ),
        (
            examples().join("user-card.mprx"),
            example_manifest.clone(),
            "user-card-example".to_string(),
        ),
        (
            examples().join("page.mprx"),
            example_manifest,
            "page".to_string(),
        ),
    ];
    for dir in ["fixtures/check/pass", "fixtures/check/fail"] {
        for file in mprx_files(dir) {
            runs.push((file, check_manifest.clone(), "template".to_string()));
        }
    }
    runs
}

/// P1: on a file that parses, recovery is one part, and it answers
/// exactly what the canonical analysis answers, at every offset.
#[test]
fn recovery_equals_canonical_analysis_on_every_parsing_file() {
    let mut compared = 0;
    for (file, manifest, component) in corpus() {
        let source = read(&file);
        let manifest = mesh_manifest::load(&manifest).unwrap();
        let template = manifest.template(&component).unwrap();
        let result = compile_with(&source, &CompileOptions::with_template(template));
        let Some(analysis) = &result.analysis else {
            continue;
        };
        let recovery = editor::recover(&source, template);
        assert_eq!(recovery.len(), 1, "{}", file.display());
        for offset in 0..=source.len() {
            assert_eq!(
                recovery.resolution_at(offset),
                analysis.resolution_at(offset),
                "{} at {offset}",
                file.display()
            );
            assert_eq!(
                recovery.typed_at(offset),
                analysis.typed_at(offset),
                "{} at {offset}",
                file.display()
            );
        }
        compared += 1;
    }
    assert!(compared > 20, "only {compared} files compared");
}

/// P2: recovery never changes what the compiler says, for any broken file
/// or any prefix of the examples, with or without a model.
#[test]
fn compile_with_is_unchanged_by_recovery() {
    let manifest = mesh_manifest::load(&read(&examples().join("components.json"))).unwrap();
    let template = manifest.template("users-page").unwrap();
    let mut sources: Vec<String> = mprx_files("fixtures/fail")
        .iter()
        .map(|file| read(file))
        .collect();
    for example in ["users-page.mprx", "user-card.mprx"] {
        let source = read(&examples().join(example));
        sources.extend(
            source
                .char_indices()
                .map(|(end, _)| source[..end].to_string()),
        );
    }
    for source in &sources {
        let with = CompileOptions::with_template(template);
        let before = (compile(source), compile_with(source, &with));
        let recovery = editor::recover(source, template);
        let after = (compile(source), compile_with(source, &with));
        assert_eq!(before, after, "{source:?}");
        if before.1.ir.is_some() {
            assert_eq!(recovery.len(), 1, "{source:?}");
        }
    }
}

#[test]
fn the_innermost_part_wins() {
    let manifest = mesh_manifest::load(&read(&examples().join("components.json"))).unwrap();
    let template = manifest.template("users-page").unwrap();
    let source = "<page title=\"U\">\n  <avatar src={user.} alt={user.name} />\n</page>";
    let recovery = editor::recover(source, template);
    // The page (an element part) and `user` (an expression part pruned
    // from inside it).
    assert_eq!(recovery.len(), 2);
    let user = source.find("user").unwrap() + 1;
    assert_eq!(
        recovery.resolution_at(user).map(|r| &r.target),
        Some(&Target::Scope("user".to_string()))
    );
    let alt = source.find("alt").unwrap() + 1;
    assert_eq!(
        recovery.resolution_at(alt).map(|r| &r.target),
        Some(&Target::Prop {
            component: "avatar".to_string(),
            prop: "alt".to_string()
        })
    );
    let name = source.find("name").unwrap() + 1;
    assert_eq!(
        recovery.typed_at(name).map(|t| t.ty.to_string()),
        Some("string".to_string())
    );
}

#[test]
fn a_file_nested_too_deep_recovers_nothing() {
    let manifest = mesh_manifest::load(&read(&examples().join("components.json"))).unwrap();
    let template = manifest.template("users-page").unwrap();
    let depth = mesh_parser::MAX_NESTING_DEPTH + 72;
    let source = format!(
        "{}{{user.}}{}",
        "<text>".repeat(depth),
        "</text>".repeat(depth)
    );
    assert!(editor::recover(&source, template).is_empty());
}
