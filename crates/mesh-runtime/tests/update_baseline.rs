//! The baseline for update's refinements (docs/superpowers/specs/
//! 2026-10-06-mesh-fine-grained-reactivity-and-composition.md, A6): what a
//! full render costs when one item of a large keyed list changes. Run with
//! `cargo test -p mesh-runtime --release --test update_baseline -- --ignored --nocapture`.

mod common;

use common::{compile_with, snapshot};
use mesh_runtime::{
    evaluations, render, update, update_changes, Change, Changes, HostValue, PathSegment, Program,
};
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
    // The least of 15 runs: a machine's noise only ever adds time.
    let least = |run: &dyn Fn() -> u64| -> (std::time::Duration, u64) {
        let mut best = std::time::Duration::MAX;
        let mut expressions = 0;
        for _ in 0..15 {
            let (e0, t0) = (evaluations(), Instant::now());
            run();
            let took = t0.elapsed();
            if took < best {
                best = took;
                expressions = evaluations() - e0;
            }
        }
        (best, expressions)
    };
    for n in [1_000usize, 10_000] {
        let before = snapshot(&items(n, None));
        let after = snapshot(&items(n, Some(n / 2)));
        let first = render(&program, MODEL, &before).expect("renders");
        let (took, expressions) = least(&|| {
            render(&program, MODEL, &after).expect("renders");
            0
        });
        println!("full render,   {n:>6} items, one changed: {expressions:>7} expressions evaluated, {took:?}");
        let patches = update(&first, &after).expect("updates").patches.len();
        let (took, expressions) = least(&|| {
            update(&first, &after).expect("updates");
            0
        });
        println!("update,        {n:>6} items, one changed: {expressions:>7} expressions evaluated, {took:?}, {patches} patches");
        let edit = Changes {
            base: first.version(),
            changes: vec![Change::Set {
                path: vec![
                    PathSegment::Name("items".into()),
                    PathSegment::Index(n / 2),
                    PathSegment::Name("label".into()),
                ],
                value: HostValue::String("changed".into()),
            }],
        };
        let patches = update_changes(&first, &edit, None)
            .expect("updates")
            .patches
            .len();
        let (took, expressions) = least(&|| {
            update_changes(&first, &edit, None).expect("updates");
            0
        });
        println!("changes,       {n:>6} items, one changed: {expressions:>7} expressions evaluated, {took:?}, {patches} patches");
        let (took, expressions) = least(&|| {
            update(&first, &before).expect("updates");
            0
        });
        println!("update, nothing changed, {n:>6} items:    {expressions:>7} expressions evaluated, {took:?}");
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
        "render clone (tree + snapshot) x10: {:?} each",
        t.elapsed() / 10
    );
    let t = Instant::now();
    for _ in 0..10 {
        let _ = after.clone();
    }
    println!("snapshot clone x10: {:?} each", t.elapsed() / 10);
    let t = Instant::now();
    for _ in 0..10 {
        update(&first, &before).expect("updates");
    }
    println!("update, nothing changed, x10: {:?} each", t.elapsed() / 10);
}
