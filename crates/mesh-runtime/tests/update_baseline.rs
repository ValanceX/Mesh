//! The baseline for update's refinements (docs/superpowers/specs/
//! 2026-10-06-mesh-fine-grained-reactivity-and-composition.md, A6): what a
//! full render costs when one item of a large keyed list changes. Run with
//! `cargo test -p mesh-runtime --release --test update_baseline -- --ignored --nocapture`.

mod common;

use common::{compile_with, snapshot};
use mesh_runtime::{evaluations, render, update, Program};
use std::time::Instant;

const MODEL: &str = r#"{
  "version": 1, "types": {},
  "components": {
    "page": { "props": {}, "events": {}, "commands": {}, "scope": {} },
    "row":  { "props": { "title": { "type": { "kind": "string" }, "required": true } }, "events": {}, "commands": {}, "scope": {} },
    "mesh-each": { "props": {
        "items": { "type": { "kind": "list", "element": { "kind": "any" } }, "required": true },
        "as":    { "type": { "kind": "string" }, "required": true },
        "key":   { "type": { "kind": "any" }, "required": true } },
      "events": {}, "commands": {}, "scope": {} },
    "view": { "props": {}, "events": {}, "commands": {},
      "scope": { "items": { "kind": "list", "element": { "kind": "record", "fields": {
            "id": { "type": { "kind": "number" }, "required": true },
            "label": { "type": { "kind": "string" }, "required": true } } } } } }
  }
}"#;

const VIEW: &str = "<page><mesh-each items={items} as=\"item\" key={item.id}>\
  <row title={item.label}>{item.label}</row></mesh-each></page>";

fn items(n: usize, changed: Option<usize>) -> String {
    let rows: Vec<String> = (0..n)
        .map(|i| {
            let label = if Some(i) == changed {
                "changed".to_string()
            } else {
                format!("item {i}")
            };
            format!(r#"{{"id":{i},"label":"{label}"}}"#)
        })
        .collect();
    format!(r#"{{"items":[{}]}}"#, rows.join(","))
}

#[test]
#[ignore = "a benchmark: prints, asserts nothing"]
fn full_render_of_a_large_list_with_one_change() {
    let template = compile_with(MODEL, "view", VIEW);
    let texts = [template.as_str()];
    let program = Program {
        root: "view",
        templates: &texts,
    };
    for n in [1_000usize, 10_000] {
        let before = snapshot(&items(n, None));
        let after = snapshot(&items(n, Some(n / 2)));
        let first = render(&program, MODEL, &before).expect("renders");
        let (e0, t0) = (evaluations(), Instant::now());
        render(&program, MODEL, &after).expect("renders");
        println!(
            "full render,   {n:>6} items, one changed: {:>7} expressions evaluated, {:?}",
            evaluations() - e0,
            t0.elapsed()
        );
        let (e0, t0) = (evaluations(), Instant::now());
        let updated = update(&first, &after).expect("updates");
        println!(
            "update,        {n:>6} items, one changed: {:>7} expressions evaluated, {:?}, {} patches",
            evaluations() - e0,
            t0.elapsed(),
            updated.patches.len()
        );
    }
}

#[test]
#[ignore = "a benchmark: prints, asserts nothing"]
fn where_an_update_spends_its_time() {
    let template = compile_with(MODEL, "view", VIEW);
    let texts = [template.as_str()];
    let program = Program {
        root: "view",
        templates: &texts,
    };
    let n = 10_000;
    let before = snapshot(&items(n, None));
    let after = snapshot(&items(n, Some(n / 2)));
    let first = render(&program, MODEL, &before).expect("renders");
    let t = Instant::now();
    for _ in 0..10 {
        update(&first, &after).expect("updates");
    }
    println!("update x10: {:?} each", t.elapsed() / 10);
    let t = Instant::now();
    for _ in 0..10 {
        mesh_runtime::check_program(&program, MODEL);
    }
    println!("program validation x10: {:?} each", t.elapsed() / 10);
    let t = Instant::now();
    for _ in 0..10 {
        let _ = first.clone();
    }
    println!(
        "render clone (tree + snapshot + memo) x10: {:?} each",
        t.elapsed() / 10
    );
}
