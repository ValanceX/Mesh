//! Programs of several templates (D7): each assembly rule broken, with
//! its code and location, and composition working: scope binding,
//! nesting, keys through composites, and a component without a template
//! as a primitive.

mod common;

use common::{codes, compile, compile_with, try_render, MODEL, SNAPSHOT};
use mesh_runtime::{Location, Node, TreeChild};
use serde_json::json;

const CARD: &str =
    "<page title={user.name}><probe maybeAny={compact} /><probe on.tap={selectUser(user)} /></page>";

fn card() -> String {
    compile("card", CARD)
}

fn page(source: &str) -> String {
    compile("view", source)
}

fn source_at(location: &Location) -> (String, usize) {
    match location {
        Location::Source { component, span } => (component.clone(), span.start.byte),
        other => panic!("{other:?}"),
    }
}

fn nodes(node: &Node, out: &mut Vec<Node>) {
    out.push(node.clone());
    for child in &node.children {
        if let TreeChild::Node(child) = child {
            nodes(child, out);
        }
    }
}

// --- the assembly rules ------------------------------------------------------

#[test]
fn rule_1_one_template_per_component() {
    let diagnostics = try_render(
        "view",
        &[page("<card user={user} />"), card(), card()],
        SNAPSHOT,
    )
    .unwrap_err();
    assert_eq!(codes(&diagnostics), ["assembly-duplicate-template"]);
    assert_eq!(source_at(&diagnostics[0].location), ("card".into(), 0));
}

#[test]
fn rule_2_the_roots_template() {
    let diagnostics = try_render("view", &[card()], SNAPSHOT).unwrap_err();
    assert_eq!(codes(&diagnostics), ["assembly-missing-root"]);
    assert_eq!(diagnostics[0].location, Location::Program);
}

#[test]
fn rule_3_one_model() {
    let other = MODEL.replace("\"tap\": {},", "\"tap\": {}, \"hold\": {},");
    assert_ne!(other, MODEL);
    let stray = compile_with(&other, "card", CARD);
    let diagnostics =
        try_render("view", &[page("<card user={user} />"), stray], SNAPSHOT).unwrap_err();
    assert_eq!(codes(&diagnostics), ["assembly-fingerprint-mismatch"]);
    assert_eq!(
        diagnostics[0].location,
        Location::Template {
            index: 1,
            component: Some("card".into())
        }
    );
}

/// `frame` binds its scope name `user`; the models below break that.
fn frame_model(props: &str) -> String {
    MODEL.replace(
        "\"frame\": {\n      \"props\": { \"user\": { \"type\": { \"kind\": \"named\", \"name\": \"User\" }, \"required\": true } },",
        &format!("\"frame\": {{\n      \"props\": {props},"),
    )
}

fn render_frame(model: &str, occurrence: &str) -> Vec<mesh_runtime::RuntimeDiagnostic> {
    let root = compile_with(model, "view", &format!("<page>{occurrence}</page>"));
    let frame = compile_with(model, "frame", "<text>{user.name}</text>");
    let texts = [root.as_str(), frame.as_str()];
    mesh_runtime::render(
        &mesh_runtime::Program {
            root: "view",
            templates: &texts,
        },
        model,
        &common::snapshot(SNAPSHOT),
    )
    .unwrap_err()
}

#[test]
fn rule_4a_every_scope_name_is_bound() {
    let model = frame_model("{ \"person\": { \"type\": { \"kind\": \"named\", \"name\": \"User\" }, \"required\": true } }");
    assert_ne!(model, MODEL);
    let diagnostics = render_frame(&model, "<frame person={user} />");
    assert_eq!(codes(&diagnostics), ["assembly-unbound-scope-name"]);
    assert_eq!(source_at(&diagnostics[0].location), ("view".into(), 6));
}

#[test]
fn rule_4b_a_binding_whose_types_dont_fit() {
    let model =
        frame_model("{ \"user\": { \"type\": { \"kind\": \"string\" }, \"required\": true } }");
    let diagnostics = render_frame(&model, "<frame user={name} />");
    assert_eq!(codes(&diagnostics), ["assembly-unsound-binding"]);
}

#[test]
fn rule_4b_an_optional_prop_cant_bind_a_scope_name_that_isnt() {
    let model = frame_model("{ \"user\": { \"type\": { \"kind\": \"named\", \"name\": \"User\" }, \"required\": false } }");
    let diagnostics = render_frame(&model, "<frame user={user} />");
    assert_eq!(codes(&diagnostics), ["assembly-unsound-binding"]);
    // `card`'s optional `compact` binds `compact: boolean?`: sound.
    try_render("view", &[page("<card user={user} />"), card()], SNAPSHOT).expect("sound");
}

#[test]
fn rule_5_no_cycles_direct_or_indirect() {
    // Direct: frame's template holds a frame.
    let frame = compile("frame", "<page><frame user={user} /></page>");
    let diagnostics =
        try_render("view", &[page("<frame user={user} />"), frame], SNAPSHOT).unwrap_err();
    assert_eq!(codes(&diagnostics), ["assembly-cycle"]);
    assert_eq!(source_at(&diagnostics[0].location).0, "frame");
    // Indirect: frame holds a card, card holds a frame. Both occurrences
    // on the cycle are reported; the root's, off it, isn't.
    let frame = compile("frame", "<page><card user={user} /></page>");
    let card = compile("card", "<page><frame user={user} /></page>");
    let diagnostics = try_render(
        "view",
        &[page("<frame user={user} />"), frame, card],
        SNAPSHOT,
    )
    .unwrap_err();
    assert_eq!(codes(&diagnostics), ["assembly-cycle", "assembly-cycle"]);
    let at: Vec<String> = diagnostics
        .iter()
        .map(|d| source_at(&d.location).0)
        .collect();
    assert_eq!(at, ["card", "frame"]);
}

#[test]
fn rule_6_no_composite_events() {
    // `probe` declares events: with a template, it's a composite.
    let probe = compile("probe", "<text>probe</text>");
    let diagnostics = try_render(
        "view",
        &[page("<page><probe /><probe /></page>"), probe],
        SNAPSHOT,
    )
    .unwrap_err();
    assert_eq!(
        codes(&diagnostics),
        ["assembly-composite-event", "assembly-composite-event"]
    );
}

#[test]
fn rule_7_no_composite_children_but_whitespace_isnt_a_child() {
    for child in ["<text>x</text>", "Ada", "{name}"] {
        let diagnostics = try_render(
            "view",
            &[page(&format!("<card user={{user}}>{child}</card>")), card()],
            SNAPSHOT,
        )
        .unwrap_err();
        assert_eq!(
            codes(&diagnostics),
            ["assembly-composite-children"],
            "{child}"
        );
    }
    try_render(
        "view",
        &[page("<card user={user}>\n   \n</card>"), card()],
        SNAPSHOT,
    )
    .expect("whitespace isn't a child");
}

#[test]
fn an_unreached_template_is_checked_and_ignored() {
    let render = try_render("view", &[page("<text>{name}</text>"), card()], SNAPSHOT)
        .expect("an unreached template is valid");
    assert_eq!(render.tree().root.component, "text");
    let broken = compile("frame", "<page><frame user={user} /></page>");
    let diagnostics =
        try_render("view", &[page("<text>{name}</text>"), broken], SNAPSHOT).unwrap_err();
    assert_eq!(
        codes(&diagnostics),
        ["assembly-cycle"],
        "unreached, but still checked"
    );
}

#[test]
fn assembly_diagnostics_are_ordered() {
    // A missing root, then by template position, then by component.
    let frame = compile("frame", "<page><frame user={user} /></page>");
    let diagnostics = try_render("page", &[frame, card(), card()], SNAPSHOT).unwrap_err();
    assert_eq!(
        codes(&diagnostics),
        [
            "assembly-missing-root",
            "assembly-duplicate-template",
            "assembly-cycle"
        ]
    );
}

// --- composition -------------------------------------------------------------

#[test]
fn a_composite_is_replaced_by_its_templates_tree() {
    let render = try_render(
        "view",
        &[
            page("<page><card user={user} compact={flag} /></page>"),
            card(),
        ],
        SNAPSHOT,
    )
    .unwrap();
    let json = render.tree().to_json();
    assert!(!json.contains("card"), "composites never appear");
    let mut all = Vec::new();
    nodes(&render.tree().root, &mut all);
    let components: Vec<&str> = all.iter().map(|n| n.component.as_str()).collect();
    assert_eq!(components, ["page", "page", "probe", "probe"]);
    assert_eq!(
        all[1].props["title"],
        json!("Ada"),
        "the scope name bound from the prop"
    );
    assert_eq!(all[2].props["maybeAny"], json!(true));
}

#[test]
fn an_unwritten_optional_prop_binds_absent() {
    let render = try_render("view", &[page("<card user={user} />"), card()], SNAPSHOT).unwrap();
    let mut all = Vec::new();
    nodes(&render.tree().root, &mut all);
    assert!(
        all[1].props.is_empty(),
        "compact is absent, so its prop is omitted"
    );
}

#[test]
fn a_prop_with_no_scope_name_is_evaluated_and_unused() {
    let render = try_render(
        "view",
        &[page("<card user={user} extra={users} />"), card()],
        SNAPSHOT,
    )
    .unwrap();
    assert!(!render.tree().to_json().contains("Grace"));
    // Evaluated, so an error in it is still reported.
    let diagnostics = try_render(
        "view",
        &[page("<card user={user} extra={anything.nope} />"), card()],
        SNAPSHOT,
    )
    .unwrap_err();
    assert_eq!(codes(&diagnostics), ["runtime-missing-member"]);
}

#[test]
fn a_composite_prop_from_any_must_fit() {
    let diagnostics = try_render(
        "view",
        &[page("<card user={anything} />"), card()],
        SNAPSHOT,
    )
    .unwrap_err();
    assert_eq!(codes(&diagnostics), ["runtime-prop-mismatch"]);
    assert_eq!(source_at(&diagnostics[0].location).0, "view");
}

#[test]
fn composites_nest_and_their_keys_differ_from_the_same_primitive_outside() {
    let frame = compile("frame", "<page><card user={user} /><probe /></page>");
    let render = try_render(
        "view",
        &[
            page("<page><frame user={user} /><card user={user} /><probe /></page>"),
            frame,
            card(),
        ],
        SNAPSHOT,
    )
    .unwrap();
    let mut all = Vec::new();
    nodes(&render.tree().root, &mut all);
    let probes: Vec<&Node> = all.iter().filter(|n| n.component == "probe").collect();
    // frame's card's two, frame's own, the root's card's two, the root's own.
    assert_eq!(probes.len(), 6);
    let mut keys: Vec<&str> = all.iter().map(|n| n.key.as_str()).collect();
    let total = keys.len();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), total, "every key distinct");
    let taps: Vec<&String> = probes.iter().filter_map(|n| n.events.get("tap")).collect();
    assert_eq!(taps.len(), 2);
    assert_ne!(
        taps[0], taps[1],
        "the same handler in two occurrences of card"
    );
}

#[test]
fn a_component_without_a_template_is_a_primitive() {
    let render = try_render("view", &[page("<card user={user} />")], SNAPSHOT).unwrap();
    assert_eq!(render.tree().root.component, "card", "a node of its name");
    assert_eq!(render.tree().root.props["user"]["name"], json!("Ada"));
}
