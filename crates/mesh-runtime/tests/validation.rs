//! Program validation and input validation (D3, D5, I12, I14): each case
//! the Definition of Done's "Fingerprints and versions" (the runtime's
//! half) and "Inputs" lists name.

mod common;

use common::{codes, compile, compile_with, snapshot, try_render, view, MODEL, SNAPSHOT};
use mesh_runtime::{
    HostKey, HostRecord, HostValue, Location, PathSegment, Program, RuntimeDiagnostic,
};

const PAGE: &str = "<page title={name}><text>{user.name}</text></page>";

fn path(segments: &[&str]) -> Location {
    Location::Input(
        segments
            .iter()
            .map(|s| match s.parse::<usize>() {
                Ok(index) => PathSegment::Index(index),
                Err(_) => PathSegment::Name((*s).to_string()),
            })
            .collect(),
    )
}

fn render_host(record: &HostRecord) -> Result<mesh_runtime::Render, Vec<RuntimeDiagnostic>> {
    let template = compile("view", PAGE);
    mesh_runtime::render(
        &Program {
            root: "view",
            templates: &[&template],
        },
        MODEL,
        record,
    )
}

/// The ordinary snapshot, with `name` set to `value` (or removed, for
/// `None`).
fn with(name: &str, value: Option<HostValue>) -> HostRecord {
    let mut record = snapshot(SNAPSHOT);
    record
        .0
        .retain(|(key, _)| *key != HostKey::Text(name.into()));
    if let Some(value) = value {
        record.0.push((HostKey::Text(name.into()), value));
    }
    record
}

fn only(result: Result<mesh_runtime::Render, Vec<RuntimeDiagnostic>>) -> RuntimeDiagnostic {
    let diagnostics = result.expect_err("it should fail");
    assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
    diagnostics.into_iter().next().unwrap()
}

// --- program validation ------------------------------------------------------

#[test]
fn a_broken_model_reports_only_its_errors() {
    let template = compile("view", PAGE);
    let diagnostics = mesh_runtime::render(
        &Program {
            root: "view",
            templates: &[&template],
        },
        "{ \"version\": 1 }",
        &snapshot(SNAPSHOT),
    )
    .expect_err("the model is broken");
    assert!(
        diagnostics.iter().all(|d| d.code.starts_with("manifest-")),
        "{diagnostics:#?}"
    );
    assert!(diagnostics
        .iter()
        .all(|d| matches!(d.location, Location::Model(_))));
}

#[test]
fn a_malformed_template_is_refused_at_the_template() {
    let good = compile("view", PAGE);
    for (bad, component) in [
        ("{ not json".to_string(), None),
        (
            good.replace("\"format\":\"mesh-template\"", "\"format\":\"mesh-render\""),
            Some("view"),
        ),
        (
            good.replace("\"kind\":\"scope\"", "\"kind\":\"nope\""),
            Some("view"),
        ),
    ] {
        let diagnostic = only(try_render("view", &[bad], SNAPSHOT));
        assert_eq!(diagnostic.code, "assembly-malformed-template");
        assert_eq!(
            diagnostic.location,
            Location::Template {
                index: 0,
                component: component.map(str::to_string)
            }
        );
    }
}

#[test]
fn another_format_version_is_unsupported() {
    let template = compile("view", PAGE).replace("\"version\":1", "\"version\":2");
    assert_eq!(
        codes(&try_render("view", &[template], SNAPSHOT).unwrap_err()),
        ["assembly-unsupported-format-version"]
    );
}

#[test]
fn a_name_the_model_doesnt_declare_makes_a_template_malformed() {
    let template = compile("view", PAGE);
    for forged in [
        template.replace("\"component\":\"text\"", "\"component\":\"nope\""),
        template.replace("\"prop\":\"title\"", "\"prop\":\"heading\""),
        template.replace("\"name\":\"name\"", "\"name\":\"nope\""),
    ] {
        assert_ne!(forged, template);
        assert_eq!(
            codes(&try_render("view", &[forged], SNAPSHOT).unwrap_err()),
            ["assembly-malformed-template"]
        );
    }
}

#[test]
fn a_template_of_another_model_is_refused() {
    let other = MODEL.replace("\"text\": { \"props\": {}", "\"text\": { \"props\": { \"size\": { \"type\": { \"kind\": \"number\" }, \"required\": false } }");
    assert_ne!(other, MODEL);
    let template = compile_with(&other, "view", PAGE);
    let diagnostic = only(try_render("view", &[template], SNAPSHOT));
    assert_eq!(diagnostic.code, "assembly-fingerprint-mismatch");
    assert_eq!(
        diagnostic.location,
        Location::Template {
            index: 0,
            component: Some("view".into())
        }
    );
}

#[test]
fn a_template_of_another_compiler_version_runs() {
    let template = compile("view", PAGE).replace(
        &format!("\"compiler\":\"{}\"", env!("CARGO_PKG_VERSION")),
        "\"compiler\":\"9.9.9-anything\"",
    );
    assert!(template.contains("9.9.9-anything"));
    try_render("view", &[template], SNAPSHOT).expect("the compiler version is provenance only");
}

#[test]
fn a_program_without_its_roots_template_is_refused() {
    let diagnostic = only(try_render("page", &[compile("view", PAGE)], SNAPSHOT));
    assert_eq!(diagnostic.code, "assembly-missing-root");
    assert_eq!(diagnostic.location, Location::Program);
}

#[test]
fn a_root_that_contains_itself_is_a_cycle() {
    // Used as an occurrence, `view` is a composite, so every assembly
    // rule applies to it: the cycle, and each scope name no prop binds.
    // The cycle comes first: the same occurrence, and codes in order.
    let template = compile("view", "<page><view /></page>");
    let diagnostics = try_render("view", &[template], SNAPSHOT).unwrap_err();
    assert_eq!(diagnostics[0].code, "assembly-cycle");
    assert!(diagnostics[1..]
        .iter()
        .all(|d| d.code == "assembly-unbound-scope-name"));
    assert_eq!(
        diagnostics.len(),
        1 + 12,
        "the cycle, and view's 12 unbound scope names"
    );
}

// --- the snapshot -----------------------------------------------------------

#[test]
fn a_missing_required_scope_name_is_reported_and_an_absent_optional_one_is_not() {
    let diagnostic = only(render_host(&with("count", None)));
    assert_eq!(diagnostic.code, "runtime-missing-value");
    assert_eq!(diagnostic.location, path(&["count"]));
    render_host(&with("maybeName", None)).expect("an optional scope name may be absent");
}

#[test]
fn names_the_root_doesnt_declare_are_ignored_whatever_they_hold() {
    let mut record = snapshot(SNAPSHOT);
    record
        .0
        .push((HostKey::Text("extra".into()), HostValue::Number(f64::NAN)));
    record.0.push((
        HostKey::Text("more".into()),
        HostValue::Unsupported("function".into()),
    ));
    record
        .0
        .push((HostKey::Text("odd".into()), HostValue::Utf16(vec![0xd800])));
    record
        .0
        .push((HostKey::Utf16(vec![0xdc00]), HostValue::Null));
    render_host(&record).expect("undeclared names are never looked at");
}

#[test]
fn a_wrong_kind_at_depth_is_reported_at_its_path() {
    let json = SNAPSHOT.replace(
        "{ \"name\": \"Grace\", \"active\": false }",
        "{ \"name\": \"Grace\", \"active\": \"no\" }",
    );
    let diagnostic = only(render_host(&snapshot(&json)));
    assert_eq!(diagnostic.code, "runtime-value-mismatch");
    assert_eq!(diagnostic.location, path(&["users", "1", "active"]));
}

#[test]
fn records_are_exact() {
    let extra = SNAPSHOT.replace(
        "\"avatar\": \"ada.png\", \"active\": true }",
        "\"avatar\": \"ada.png\", \"active\": true, \"age\": 36 }",
    );
    let diagnostic = only(render_host(&snapshot(&extra)));
    assert_eq!(diagnostic.code, "runtime-unknown-field");
    assert_eq!(diagnostic.location, path(&["user", "age"]));

    let no_avatar = SNAPSHOT.replace("\"avatar\": \"ada.png\", ", "");
    render_host(&snapshot(&no_avatar)).expect("a field that isn't required may be absent");

    let no_active = SNAPSHOT.replace(
        "\"avatar\": \"ada.png\", \"active\": true }",
        "\"avatar\": \"ada.png\" }",
    );
    let diagnostic = only(render_host(&snapshot(&no_active)));
    assert_eq!(diagnostic.code, "runtime-missing-value");
    assert_eq!(diagnostic.location, path(&["user", "active"]));
}

#[test]
fn null_is_not_absence_either_way() {
    let null_for_absent = only(render_host(&with("maybeName", Some(HostValue::Null))));
    assert_eq!(null_for_absent.code, "runtime-value-mismatch");
    assert_eq!(null_for_absent.location, path(&["maybeName"]));
    let absent_for_null = only(render_host(&with("nothing", None)));
    assert_eq!(absent_for_null.code, "runtime-missing-value");
    assert_eq!(absent_for_null.location, path(&["nothing"]));
}

#[test]
fn values_outside_the_boundary_data_model_are_refused() {
    for (value, code) in [
        (
            HostValue::OutOfRange("1e400".into()),
            "runtime-number-out-of-range",
        ),
        (HostValue::Number(f64::NAN), "runtime-non-finite-input"),
        (HostValue::Number(f64::INFINITY), "runtime-non-finite-input"),
        (
            HostValue::Unsupported("function".into()),
            "runtime-unsupported-value",
        ),
        (
            HostValue::Unsupported("object:Map".into()),
            "runtime-unsupported-value",
        ),
        (
            HostValue::Unsupported("cycle".into()),
            "runtime-unsupported-value",
        ),
    ] {
        let diagnostic = only(render_host(&with("count", Some(value))));
        assert_eq!(diagnostic.code, code);
        assert_eq!(diagnostic.location, path(&["count"]));
    }
    let diagnostic = only(render_host(&with(
        "name",
        Some(HostValue::Utf16(vec![0x61, 0xd800])),
    )));
    assert_eq!(diagnostic.code, "runtime-unpaired-surrogate");
    // From JSON too: an out-of-range number, and an escaped lone surrogate.
    let json = SNAPSHOT
        .replace("\"count\": 3", "\"count\": 1e400")
        .replace("\"name\": \"Ada\",\n", "\"name\": \"\\ud800\",\n");
    assert_eq!(
        codes(&render_host(&snapshot(&json)).unwrap_err()),
        ["runtime-number-out-of-range", "runtime-unpaired-surrogate"]
    );
}

#[test]
fn an_absent_list_element_is_refused_even_where_elements_may_be_absent() {
    let numbers = HostValue::List(vec![Some(HostValue::Number(1.0)), None]);
    let diagnostic = only(render_host(&with("numbers", Some(numbers.clone()))));
    assert_eq!(diagnostic.code, "runtime-absent-element");
    assert_eq!(diagnostic.location, path(&["numbers", "1"]));
    let diagnostic = only(render_host(&with("maybeNumbers", Some(numbers))));
    assert_eq!(
        diagnostic.code, "runtime-absent-element",
        "absence crosses only as an omitted entry"
    );
}

#[test]
fn a_value_of_type_any_is_still_checked_at_every_depth() {
    let deep = HostValue::Record(HostRecord(vec![(
        HostKey::Text("list".into()),
        HostValue::List(vec![None, Some(HostValue::Number(f64::NEG_INFINITY))]),
    )]));
    let diagnostics = render_host(&with("anything", Some(deep))).unwrap_err();
    assert_eq!(
        codes(&diagnostics),
        ["runtime-absent-element", "runtime-non-finite-input"]
    );
    assert_eq!(diagnostics[0].location, path(&["anything", "list", "0"]));
    assert_eq!(diagnostics[1].location, path(&["anything", "list", "1"]));
}

#[test]
fn every_mismatch_is_reported_in_path_order() {
    let json = SNAPSHOT
        .replace("\"count\": 3", "\"count\": \"three\"")
        .replace("\"flag\": true", "\"flag\": 1")
        .replace(
            "{ \"name\": \"Grace\", \"active\": false }",
            "{ \"name\": 7, \"active\": false }",
        )
        .replace(
            "\"avatar\": \"ada.png\", \"active\": true }",
            "\"avatar\": 1, \"active\": true, \"zzz\": 0 }",
        )
        .replace("\"numbers\": [1, 2.5]", "\"numbers\": [1, \"x\"]");
    let diagnostics = render_host(&snapshot(&json)).unwrap_err();
    let locations: Vec<Location> = diagnostics.iter().map(|d| d.location.clone()).collect();
    assert_eq!(
        locations,
        [
            path(&["count"]),
            path(&["flag"]),
            path(&["numbers", "1"]),
            path(&["user", "avatar"]),
            path(&["user", "zzz"]),
            path(&["users", "1", "name"]),
        ]
    );
}

#[test]
fn a_repeated_field_in_a_host_record_is_refused() {
    let mut record = snapshot(SNAPSHOT);
    let user = record
        .0
        .iter_mut()
        .find(|(key, _)| *key == HostKey::Text("user".into()))
        .unwrap();
    if let HostValue::Record(fields) = &mut user.1 {
        fields.0.push((
            HostKey::Text("name".into()),
            HostValue::String("Again".into()),
        ));
    }
    let diagnostic = only(render_host(&record));
    assert_eq!(diagnostic.code, "runtime-value-mismatch");
    assert_eq!(diagnostic.location, path(&["user", "name"]));
}

#[test]
fn an_ordinary_snapshot_renders() {
    view(PAGE).expect("the ordinary snapshot fits");
}
