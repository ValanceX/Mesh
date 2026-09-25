//! Evaluation (§9.7): every rule the Definition of Done's "Evaluation"
//! list spells out, each test naming its rule. Expressions are compiled
//! from real MPRX in `view`'s template and read back from the tree.

mod common;

use common::{codes, view};
use mesh_runtime::{Location, RuntimeDiagnostic, TreeChild};
use serde_json::{json, Value};

/// The value `expression` renders to, through `probe`'s `any` prop.
fn value(expression: &str) -> Value {
    let render = view(&format!("<probe any={{{expression}}} />"))
        .unwrap_or_else(|d| panic!("{expression}: {d:#?}"));
    render.tree().root.props["any"].clone()
}

/// A boolean expression's value, through `probe`'s `flag` prop.
fn boolean(expression: &str) -> bool {
    let render = view(&format!("<probe flag={{{expression}}} />"))
        .unwrap_or_else(|d| panic!("{expression}: {d:#?}"));
    render.tree().root.props["flag"]
        .as_bool()
        .expect("a boolean")
}

/// The text an interpolation renders to.
fn text(expression: &str) -> String {
    let render = view(&format!("<text>{{{expression}}}</text>"))
        .unwrap_or_else(|d| panic!("{expression}: {d:#?}"));
    match &render.tree().root.children[..] {
        [TreeChild::Text { text, .. }] => text.clone(),
        other => panic!("{other:?}"),
    }
}

/// The one evaluation error `source` gives, and the source text it's at.
fn error(source: &str) -> (&'static str, String) {
    let diagnostics = view(source).expect_err("it should fail");
    assert_eq!(
        diagnostics.len(),
        1,
        "evaluation reports exactly one: {diagnostics:#?}"
    );
    let RuntimeDiagnostic { code, location, .. } = &diagnostics[0];
    let Location::Source { component, span } = location else {
        panic!("{location:?}")
    };
    assert_eq!(component, "view");
    (code, source[span.start.byte..span.end.byte].to_string())
}

// §9.7.3 Numbers --------------------------------------------------------------

#[test]
fn rule_9_7_3_literals_are_their_binary64_values() {
    assert!(boolean("1 == 1.0"));
    assert!(boolean("1.50 == 1.5"));
    assert_eq!(value("0.1"), json!(0.1));
}

#[test]
fn rule_9_7_3_remainder_is_truncated_with_the_dividends_sign() {
    assert_eq!(value("7 % 2"), json!(1));
    assert_eq!(value("-7 % 2"), json!(-1));
    assert_eq!(value("7 % -2"), json!(1));
    assert_eq!(value("-7 % -2"), json!(-1));
    assert_eq!(value("7.5 % 2"), json!(1.5));
    // -4 % 2 and -0 % 5 are -0: seen only through the sign of 1 / it.
    assert!(boolean("1 / (-4 % 2) < 0"));
    assert!(boolean("1 / (-0 % 5) < 0"));
    assert!(boolean("1 / (4 % 2) > 0"));
    // x % 0 and ∞ % y are NaN; x % ∞ is x.
    assert!(boolean("(count % 0) != (count % 0)"));
    assert!(boolean("((1 / 0) % 2) != ((1 / 0) % 2)"));
    assert_eq!(value("5 % (1 / 0)"), json!(5));
}

#[test]
fn rule_9_7_3_division_follows_ieee_754() {
    assert!(boolean("1 / 0 > 1000000 * 1000000 * 1000000"));
    assert!(boolean("-1 / 0 < -(1000000 * 1000000 * 1000000)"));
    assert!(boolean("(0 / 0) != (0 / 0)"));
    assert!(boolean("1 / -0 < 0"));
    assert!(boolean("1 / 0 == 1 / 0"));
}

#[test]
fn rule_9_7_3_negative_zero_is_zero_at_every_output() {
    assert!(boolean("0 == -0"));
    assert!(!boolean("-0 < 0"));
    assert_eq!(value("0 * -1"), json!(0));
    assert_eq!(
        value("{ a: 0 * -1, b: [-0], c: { z: -0 } }"),
        json!({ "a": 0, "b": [0], "c": { "z": 0 } })
    );
    assert_eq!(text("0 * -1"), "0");
    let render = view("<probe num={-0} />").unwrap();
    assert_eq!(render.tree().root.props["num"], json!(0));
    assert!(!render.tree().to_json().contains("-0"));
}

#[test]
fn rule_9_7_3_non_finite_numbers_may_live_inside_evaluation() {
    // Compared, discarded by a conditional, or passed to a composite's
    // prop that its scope doesn't use: no error.
    assert!(boolean("1 / 0 > count"));
    assert_eq!(value("flag ? 1 : 1 / 0"), json!(1));
    assert_eq!(
        text("(0 / 0) == (0 / 0) ? \"same\" : \"never equal\""),
        "never equal"
    );
    let card = common::compile("card", "<text>{user.name}</text>");
    let page = common::compile("view", "<page><card user={user} extra={1 / 0} /></page>");
    common::try_render("view", &[page, card], common::SNAPSHOT)
        .expect("an unused composite prop may be infinite");
}

#[test]
fn rule_9_7_6_a_non_finite_number_reaching_an_output_is_an_error() {
    assert_eq!(
        error("<probe num={1 / 0} />"),
        ("runtime-non-finite-output", "1 / 0".into())
    );
    assert_eq!(
        error("<probe any={{ a: [1, 0 / 0] }} />"),
        ("runtime-non-finite-output", "{ a: [1, 0 / 0] }".into())
    );
    assert_eq!(
        error("<text>{-1 / 0}</text>"),
        ("runtime-non-finite-output", "-1 / 0".into())
    );
}

// §9.7.4 Equality and comparison ----------------------------------------------

#[test]
fn rule_9_7_4_nan_equals_nothing() {
    assert!(!boolean("(0 / 0) == (0 / 0)"));
    assert!(boolean("(0 / 0) != (0 / 0)"));
    assert!(!boolean("(0 / 0) < 1"));
    assert!(!boolean("(0 / 0) >= 1"));
    assert!(!boolean("[0 / 0] == [0 / 0]"));
}

#[test]
fn rule_9_7_4_equality_is_strict_and_structural() {
    assert!(!boolean("anything == \"x\""));
    assert!(boolean("[1, 2] == [1, 2]"));
    assert!(!boolean("[1, 2] == [2, 1]"));
    assert!(boolean("{ a: 1, b: \"x\" } == { b: \"x\", a: 1 }"));
    assert!(boolean("user == user"));
    assert!(!boolean("users == [user]"));
    // Absent equals absent, and nothing else; null isn't absent.
    assert!(boolean("maybeAnything == maybeAnything"));
    assert!(!boolean("maybeAnything == nothing"));
    assert!(boolean("maybeName != \"Ada\""));
}

// §9.7.5 Evaluation order ------------------------------------------------------

#[test]
fn rule_9_7_5_short_circuiting_skips_the_right_operand() {
    // `anything` isn't a boolean, which is an error only if evaluated.
    assert!(!boolean("false && anything"));
    assert!(boolean("true || anything"));
    assert_eq!(
        error("<probe flag={true && anything} />"),
        ("runtime-operand-mismatch", "anything".into())
    );
    assert_eq!(
        error("<probe flag={false || anything} />"),
        ("runtime-operand-mismatch", "anything".into())
    );
}

#[test]
fn rule_9_7_5_a_conditional_evaluates_one_branch() {
    assert_eq!(value("flag ? 1 : anything.missing"), json!(1));
    assert_eq!(value("!flag ? anything.missing : 2"), json!(2));
    assert_eq!(
        error("<probe any={flag ? anything.missing : 1} />"),
        ("runtime-missing-member", "anything.missing".into())
    );
}

#[test]
fn rule_9_7_5_the_first_error_in_evaluation_order_is_reported() {
    // Three errors, in document order: the prop's comes first.
    let source =
        "<page><probe num={anything} /><text>{1 / 0}</text><probe num={-anything} /></page>";
    assert_eq!(error(source), ("runtime-prop-mismatch", "anything".into()));
    // Operands before their operator: the left operand's error first.
    assert_eq!(
        error("<probe num={-anything + (1 / 0)} />"),
        ("runtime-operand-mismatch", "anything".into())
    );
}

// §9.7.6 Runtime checks ------------------------------------------------------

#[test]
fn rule_9_7_6_member_access() {
    // On a record type, an absent field reads as absent.
    let render = common::try_render(
        "view",
        &[common::compile("view", "<probe maybeStr={user.avatar} />")],
        &common::SNAPSHOT.replace("\"avatar\": \"ada.png\", ", ""),
    )
    .unwrap();
    assert!(render.tree().root.props.is_empty());
    // On `any`, the field must be present, and the value a record.
    assert_eq!(value("anything.deep"), json!([1, "two", null]));
    assert_eq!(
        error("<probe any={anything.nope} />"),
        ("runtime-missing-member", "anything.nope".into())
    );
    assert_eq!(
        error("<probe any={anything.deep.x} />"),
        ("runtime-not-a-record", "anything.deep".into())
    );
}

#[test]
fn rule_9_7_6_every_any_check() {
    assert_eq!(
        error("<probe num={-anything} />"),
        ("runtime-operand-mismatch", "anything".into())
    );
    assert_eq!(
        error("<probe flag={!anything} />"),
        ("runtime-operand-mismatch", "anything".into())
    );
    assert_eq!(
        error("<probe num={anything < 1 ? 1 : 2} />"),
        ("runtime-operand-mismatch", "anything".into())
    );
    assert_eq!(
        error("<probe any={anything ? 1 : 2} />"),
        ("runtime-operand-mismatch", "anything".into())
    );
    assert_eq!(
        error("<text>{anything}</text>"),
        ("runtime-content-not-text", "anything".into())
    );
    assert_eq!(
        error("<probe num={anything} />"),
        ("runtime-prop-mismatch", "anything".into())
    );
    assert_eq!(
        error("<probe user={anything} />"),
        ("runtime-prop-mismatch", "anything".into())
    );
}

#[test]
fn rule_9_7_6_an_absent_list_element_cant_reach_an_output() {
    assert_eq!(
        value("[maybeName, \"x\"] == [maybeName, \"x\"]"),
        json!(true)
    );
    assert_eq!(
        error("<probe any={[maybeName]} />"),
        ("runtime-absent-element-output", "[maybeName]".into())
    );
}

#[test]
fn rule_9_7_6_an_absent_prop_is_omitted_and_null_is_null() {
    let render =
        view("<probe maybeStr={maybeName} none={nothing} maybeAny={maybeAnything} />").unwrap();
    let props = &render.tree().root.props;
    assert_eq!(props.len(), 1, "{props:?}");
    assert_eq!(props["none"], Value::Null);
    // A record literal's absent field is no field.
    assert_eq!(value("{ a: maybeName, b: 1 }"), json!({ "b": 1 }));
}

// §9.7.7 Text ---------------------------------------------------------------

#[test]
fn rule_9_7_7_text() {
    assert_eq!(text("name"), "Ada");
    assert_eq!(text("flag"), "true");
    assert_eq!(text("!flag"), "false");
    assert_eq!(text("nothing"), "null");
    assert_eq!(text("maybeName"), "", "absent is the empty string");
    assert_eq!(text("count"), "3");
    assert_eq!(text("count / 4"), "0.75");
    assert_eq!(text("0.1 + 0.2"), "0.30000000000000004");
    assert_eq!(text("1000000 * 1000000 * 1000000 * 1000"), "1e+21");
}

/// The normative table's non-finite rows: NaN and the infinities are
/// stopped by the output check before text conversion.
#[test]
fn rule_9_7_7_non_finite_numbers_have_no_text() {
    for expression in ["0 / 0", "1 / 0", "-1 / 0"] {
        assert_eq!(
            error(&format!("<text>{{{expression}}}</text>")),
            ("runtime-non-finite-output", expression.to_string())
        );
    }
}

#[test]
fn a_text_run_joins_text_and_interpolations_and_is_kept_when_empty() {
    let render = view("<text>Hello, {name}! You have {count} new.</text>").unwrap();
    assert!(
        matches!(&render.tree().root.children[..], [TreeChild::Text { text, .. }] if text == "Hello, Ada! You have 3 new.")
    );
    let render = view("<text>{maybeName}</text>").unwrap();
    assert!(
        matches!(&render.tree().root.children[..], [TreeChild::Text { text, .. }] if text.is_empty())
    );
}

#[test]
fn diagnostics_are_codes_the_reference_lists() {
    let diagnostics = view("<probe num={1 / 0} />").unwrap_err();
    assert_eq!(codes(&diagnostics), ["runtime-non-finite-output"]);
}
