//! The slice (outline, Definition of Done, "The slice"), natively:
//! `examples/slice/`'s `users` page uses the composite `user-card` twice.
//! Its tree, printed rendering, first avatar click's intent, and the
//! changes after one user changes are each a committed file under
//! `examples/slice/expected/`. `MESH_BLESS=1` rewrites them; review
//! every rewritten file. `@valancex/mesh-runtime`'s `slice.test.mjs`
//! checks the same files in Node.

mod common;

use common::renderer;
use mesh_compiler::check;
use mesh_runtime::{HostRecord, HostValue, Node, Program, TreeChild};
use std::fs;
use std::path::{Path, PathBuf};

fn slice() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/slice")
}

fn read(name: &str) -> String {
    fs::read_to_string(slice().join(name)).unwrap_or_else(|err| panic!("should read {name}: {err}"))
}

fn expect(name: &str, actual: &str, failures: &mut Vec<String>) {
    let path = slice().join("expected").join(name);
    if std::env::var_os("MESH_BLESS").is_some() {
        fs::write(&path, actual).unwrap();
        return;
    }
    let expected = fs::read_to_string(&path).unwrap_or_default();
    if expected != actual {
        failures.push(format!(
            "{name}:\n--- expected ---\n{expected}--- actual ---\n{actual}"
        ));
    }
}

/// Both templates, compiled against the slice's manifest, in the order
/// the program lists them.
fn templates(model: &str) -> Vec<String> {
    ["users", "user-card"]
        .iter()
        .map(|component| {
            let model = check::Model::load(model, component).expect("the manifest loads");
            let compiled = check::template(&read(&format!("{component}.mprx")), &model);
            assert!(
                compiled.diagnostics.is_empty(),
                "{component}: {:#?}",
                compiled.diagnostics
            );
            mesh_template::to_json(&compiled.template.expect("it compiles"))
        })
        .collect()
}

fn components(node: &Node, out: &mut Vec<String>) {
    out.push(node.component.clone());
    for child in &node.children {
        if let TreeChild::Node(child) = child {
            components(child, out);
        }
    }
}

fn first_avatar_click(node: &Node) -> Option<String> {
    if node.component == "avatar" {
        return node.events.get("click").cloned();
    }
    node.children.iter().find_map(|child| match child {
        TreeChild::Node(child) => first_avatar_click(child),
        TreeChild::Text { .. } => None,
    })
}

#[test]
fn the_slice_renders_dispatches_and_rerenders_as_committed() {
    let model = read("components.json");
    let templates = templates(&model);
    let texts: Vec<&str> = templates.iter().map(String::as_str).collect();
    assert_eq!(
        check::program(&model, "users", &texts),
        vec![],
        "the program is valid"
    );
    let program = Program {
        root: "users",
        templates: &texts,
    };
    let first = HostRecord::from_json(&read("snapshots/first.json")).unwrap();
    let second = HostRecord::from_json(&read("snapshots/second.json")).unwrap();
    let mut failures = Vec::new();

    let rendered = mesh_runtime::render(&program, &model, &first).expect("first renders");
    let tree = rendered.tree();
    let mut names = Vec::new();
    components(&tree.root, &mut names);
    assert!(
        names.iter().all(|name| name != "user-card"),
        "primitives only: {names:?}"
    );
    expect(
        "first.tree.json",
        &format!("{}\n", tree.to_json()),
        &mut failures,
    );
    expect("first.html", &renderer::print(tree), &mut failures);

    let click = first_avatar_click(&tree.root).expect("an avatar with a click");
    let payload = HostValue::from_json(r#"{ "x": 12, "y": 34 }"#).unwrap();
    let intent = mesh_runtime::dispatch(&rendered, &click, Some(&payload)).expect("an intent");
    expect(
        "select-first.intent.json",
        &format!("{}\n", intent.to_json()),
        &mut failures,
    );

    let again = mesh_runtime::render(&program, &model, &second).expect("second renders");
    expect(
        "first-to-second.changes",
        &renderer::compare(tree, again.tree()),
        &mut failures,
    );
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
fn the_slice_is_not_in_the_check_corpus_and_checks_clean() {
    // `crates/mesh-cli/tests/fixtures.rs` lists `.mprx` files directly in
    // `examples/` and its fixture directories, never subdirectories.
    let listed: Vec<String> = fs::read_dir(slice().join(".."))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "mprx"))
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert!(!listed.contains(&"users.mprx".to_string()), "{listed:?}");
    let model = read("components.json");
    for component in ["users", "user-card"] {
        let model = check::Model::load(&model, component).unwrap();
        assert_eq!(
            check::source(&read(&format!("{component}.mprx")), Some(&model)),
            vec![],
            "{component} checks clean"
        );
    }
}
