//! The module's core, natively: render and dispatch from encoded inputs
//! give exactly the runtime's results, and inputs that can't be taken
//! are refused with the right status.

use mesh_compiler::check;
use mesh_runtime::encoding::{encode_texts, encode_value};
use mesh_runtime::{HostKey, HostRecord, HostValue, Program};
use mesh_runtime_wasm::{number_texts, respond_dispatch, respond_render, Refusal};
use serde_json::Value;
use std::path::Path;

const VIEW: &str = "<page title={user.name}>\n  <card user={user} compact={flag} />\n  <button on.click={save()} />\n</page>\n";
const CARD: &str = "<page title={user.name}>\n  <button on.click={select(user)} />\n</page>\n";

fn model() -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../mesh-cli/tests/check-program/components.json"),
    )
    .unwrap()
}

fn compile(model: &str, component: &str, source: &str) -> String {
    let model = check::Model::load(model, component).unwrap();
    mesh_template::to_json(&check::template(source, &model).template.unwrap())
}

fn snapshot(name: &str) -> HostRecord {
    HostRecord(vec![
        (
            HostKey::Text("user".into()),
            HostValue::Record(HostRecord(vec![(
                HostKey::Text("name".into()),
                HostValue::String(name.into()),
            )])),
        ),
        (HostKey::Text("flag".into()), HostValue::Boolean(true)),
    ])
}

#[test]
fn render_and_dispatch_give_the_runtimes_results() {
    let model = model();
    let templates = [compile(&model, "view", VIEW), compile(&model, "card", CARD)];
    let texts: Vec<&str> = templates.iter().map(String::as_str).collect();
    let encoded = encode_texts(&texts);
    let snapshot = snapshot("Ada");
    let snapshot_bytes = encode_value(&HostValue::Record(snapshot.clone()));

    let native = mesh_runtime::render(
        &Program {
            root: "view",
            templates: &texts,
        },
        &model,
        &snapshot,
    )
    .unwrap();
    let rendered = respond_render("view", &encoded, &model, &snapshot_bytes).unwrap();
    assert_eq!(
        rendered,
        format!("{{\"tree\":{}}}", native.tree().to_json())
    );

    // The card's button: dispatch through the composite.
    let document: Value = serde_json::from_str(&rendered).unwrap();
    let handler = document["tree"]["root"]["children"][0]["children"][0]["events"]["click"]
        .as_str()
        .unwrap()
        .to_string();
    let intent =
        respond_dispatch("view", &encoded, &model, &snapshot_bytes, &handler, None).unwrap();
    let native_intent = mesh_runtime::dispatch(&native, &handler, None).unwrap();
    assert_eq!(
        intent,
        format!("{{\"intent\":{}}}", native_intent.to_json())
    );
    let intent: Value = serde_json::from_str(&intent).unwrap();
    assert_eq!(intent["intent"]["command"]["component"], "card");
    assert_eq!(intent["intent"]["arguments"][0]["value"]["name"], "Ada");

    // A payload for an event without one: diagnostics, not a refusal.
    let refused = respond_dispatch(
        "view",
        &encoded,
        &model,
        &snapshot_bytes,
        &handler,
        Some(&encode_value(&HostValue::Null)),
    )
    .unwrap();
    assert!(refused.starts_with("{\"diagnostics\":"), "{refused}");
}

#[test]
fn inputs_that_cant_be_taken_are_refused() {
    let model = model();
    let templates = encode_texts(&[&compile(&model, "card", CARD)]);
    let snapshot = encode_value(&HostValue::Record(snapshot("Ada")));
    assert!(matches!(
        respond_render("card", &[1, 2], &model, &snapshot),
        Err(Refusal::Encoding(_))
    ));
    assert!(matches!(
        respond_render("card", &templates, &model, &[9]),
        Err(Refusal::Encoding(_))
    ));
    assert_eq!(
        respond_render("card", &templates, &model, &encode_value(&HostValue::Null)),
        Err(Refusal::Input("a snapshot is a record".into()))
    );
    let mut deep = HostValue::Null;
    for _ in 0..200 {
        deep = HostValue::List(vec![Some(deep)]);
    }
    let deep = HostValue::Record(HostRecord(vec![(HostKey::Text("user".into()), deep)]));
    assert!(matches!(
        respond_render("card", &templates, &model, &encode_value(&deep)),
        Err(Refusal::Input(_))
    ));
}

#[test]
fn number_texts_are_the_runtimes() {
    let values = [0.0, -0.0, 1.5, 1e21, 5e-324, 123.456];
    let bits: Vec<u8> = values
        .iter()
        .flat_map(|v: &f64| v.to_bits().to_le_bytes())
        .collect();
    assert_eq!(
        number_texts(&bits).unwrap(),
        "0\n0\n1.5\n1e+21\n5e-324\n123.456\n"
    );
    assert_eq!(number_texts(&[0; 7]), None);
    assert_eq!(number_texts(&f64::NAN.to_bits().to_le_bytes()), None);
}
