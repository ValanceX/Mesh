//! `content-not-text` (§9.7.8): an interpolation in content whose static
//! type is a list, a record, `list<nothing>` or an optional of one has no
//! text form, and is an error at the interpolation.

mod common;

use common::at;
use mesh_analysis::{Analysis, Fact};

const MANIFEST: &str = r#"{
  "version": 1,
  "types": {
    "User": { "kind": "record", "fields": {
      "name": { "type": { "kind": "string" }, "required": true }
    } },
    "Names": { "kind": "list", "element": { "kind": "string" } },
    "MaybeUser": { "kind": "optional", "type": { "kind": "named", "name": "User" } }
  },
  "components": {
    "view": {
      "props": {}, "events": {}, "commands": {},
      "scope": {
        "user": { "kind": "named", "name": "User" },
        "maybeUser": { "kind": "named", "name": "MaybeUser" },
        "names": { "kind": "named", "name": "Names" },
        "maybeNames": { "kind": "optional", "type": { "kind": "list", "element": { "kind": "string" } } },
        "name": { "kind": "string" },
        "count": { "kind": "number" },
        "flag": { "kind": "boolean" },
        "nothing": { "kind": "null" },
        "anything": { "kind": "any" },
        "maybeAnything": { "kind": "optional", "type": { "kind": "any" } },
        "maybeName": { "kind": "optional", "type": { "kind": "string" } }
      }
    },
    "text": { "props": {}, "events": {}, "commands": {}, "scope": {} }
  }
}"#;

fn analyze(source: &str) -> Analysis {
    common::analyze(MANIFEST, "view", source)
}

fn content_facts(analysis: &Analysis) -> Vec<&Fact> {
    analysis
        .facts()
        .iter()
        .filter(|fact| matches!(fact, Fact::ContentNotText { .. }))
        .collect()
}

#[test]
fn lists_and_records_in_content_have_no_text() {
    for expression in [
        "user",
        "maybeUser",
        "names",
        "maybeNames",
        "[1]",
        "[]",
        "{ a: 1 }",
        "flag ? names : names",
    ] {
        let source = format!("<text>{{{expression}}}</text>");
        let analysis = analyze(&source);
        let facts = content_facts(&analysis);
        assert_eq!(facts.len(), 1, "{expression}: {:#?}", analysis.facts());
        assert_eq!(facts[0].span(), at(&source, expression), "{expression}");
        assert_eq!(analysis.facts().len(), 1, "{expression}: only one fact");
    }
}

#[test]
fn values_with_a_text_form_are_content() {
    for expression in [
        "name",
        "count",
        "flag",
        "nothing",
        "maybeName",
        "anything",
        "maybeAnything",
        "user.name",
        "\"literal\"",
        "count + 1",
    ] {
        let source = format!("<text>{{{expression}}}</text>");
        assert!(
            analyze(&source).facts().is_empty(),
            "{expression}: {:#?}",
            analyze(&source).facts()
        );
    }
}

#[test]
fn a_mistake_inside_the_interpolation_is_reported_once() {
    let source = "<text>{usr}</text>";
    let analysis = analyze(source);
    assert!(content_facts(&analysis).is_empty());
    assert_eq!(analysis.facts().len(), 1);
}
