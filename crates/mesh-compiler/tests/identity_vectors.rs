//! Keeps the identity conformance vectors honest (spec §9.10,
//! `examples/conformance/identity/`).
//!
//! The vectors are hand-written render-v1 trees with no program behind
//! them, so nothing here runs the runtime or any renderer. These tests
//! check the *vectors*: every tree is a valid render-v1 tree, each
//! identity table matches its tree, and every expectation (kept, created,
//! removed, moved, handlers, the pairing by position) follows from the
//! identities alone, by the definitions in the vectors' README. A renderer
//! checks itself against the vectors; this keeps the vectors from drifting
//! from the contract they pin.

use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &str) -> String {
    let path = root().join(path);
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("should read {}: {err}", path.display()))
}

fn vectors() -> Vec<Value> {
    let document: Value =
        serde_json::from_str(&read("examples/conformance/identity/cases.json")).expect("JSON");
    assert_eq!(document["format"], "mesh-identity-vectors");
    assert_eq!(document["version"], 1);
    document["vectors"]
        .as_array()
        .expect("`vectors` is a list")
        .clone()
}

fn name(vector: &Value) -> &str {
    vector["name"].as_str().expect("a vector has a name")
}

/// The vector with this name's prefix (`C2` is `C2-insert`).
fn find<'a>(all: &'a [Value], prefix: &str) -> &'a Value {
    all.iter()
        .find(|vector| name(vector).starts_with(&format!("{prefix}-")))
        .unwrap_or_else(|| panic!("a vector named {prefix}-…"))
}

/// One node or text run of a tree, by identity.
struct Occurrence {
    parent: Option<String>,
    index: usize,
    is_node: bool,
    /// event name → handler identifier
    handlers: BTreeMap<String, String>,
}

/// Every occurrence of `tree`, by identity name. Checks that each key has
/// exactly one identity, that no key or identity repeats, and that the table
/// names nothing the tree doesn't contain.
fn occurrences(vector: &str, side: &Value) -> BTreeMap<String, Occurrence> {
    let table = side["identities"].as_object().expect("an identity table");
    let mut found = BTreeMap::new();
    let mut keys = BTreeSet::new();
    visit(
        vector,
        &side["tree"]["root"],
        None,
        0,
        table,
        &mut found,
        &mut keys,
    );
    assert_eq!(
        keys.len(),
        table.len(),
        "{vector}: the identity table names a key the tree doesn't have"
    );
    found
}

fn visit(
    vector: &str,
    part: &Value,
    parent: Option<&str>,
    index: usize,
    table: &Map<String, Value>,
    found: &mut BTreeMap<String, Occurrence>,
    keys: &mut BTreeSet<String>,
) {
    let key = part["key"].as_str().expect("a key");
    assert!(keys.insert(key.to_string()), "{vector}: key {key} repeats");
    let identity = table
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{vector}: key {key} has no identity"))
        .to_string();
    let is_node = part["type"] == "node";
    let handlers = part["events"]
        .as_object()
        .map(|events| {
            events
                .iter()
                .map(|(event, handler)| (event.clone(), handler.as_str().unwrap().to_string()))
                .collect()
        })
        .unwrap_or_default();
    let occurrence = Occurrence {
        parent: parent.map(str::to_string),
        index,
        is_node,
        handlers,
    };
    assert!(
        found.insert(identity.clone(), occurrence).is_none(),
        "{vector}: identity {identity} repeats within one tree"
    );
    for (position, child) in part["children"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        visit(vector, child, Some(&identity), position, table, found, keys);
    }
}

fn names(found: &BTreeMap<String, Occurrence>, nodes_only: bool) -> BTreeSet<&str> {
    found
        .iter()
        .filter(|(_, occurrence)| occurrence.is_node || !nodes_only)
        .map(|(identity, _)| identity.as_str())
        .collect()
}

fn set(value: &Value) -> BTreeSet<String> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("a list, not {value}"))
        .iter()
        .map(|item| item.as_str().expect("a string").to_string())
        .collect()
}

fn owned(set: &BTreeSet<&str>) -> BTreeSet<String> {
    set.iter().map(|item| item.to_string()).collect()
}

/// "identity:event" → handler identifier.
fn handler_map(found: &BTreeMap<String, Occurrence>) -> BTreeMap<String, String> {
    found
        .iter()
        .flat_map(|(identity, occurrence)| {
            occurrence
                .handlers
                .iter()
                .map(move |(event, handler)| (format!("{identity}:{event}"), handler.clone()))
        })
        .collect()
}

/// Children of `parent` that are nodes, in order.
fn children<'a>(found: &'a BTreeMap<String, Occurrence>, parent: &str) -> Vec<&'a str> {
    let mut kids: Vec<(usize, &str)> = found
        .iter()
        .filter(|(_, occurrence)| {
            occurrence.is_node && occurrence.parent.as_deref() == Some(parent)
        })
        .map(|(identity, occurrence)| (occurrence.index, identity.as_str()))
        .collect();
    kids.sort_unstable();
    kids.into_iter().map(|(_, identity)| identity).collect()
}

fn with_trees(all: &[Value]) -> impl Iterator<Item = &Value> {
    all.iter().filter(|vector| vector.get("previous").is_some())
}

#[test]
fn the_vector_set_is_the_contract_set() {
    let all = vectors();
    for number in 1..=10 {
        let prefix = format!("C{number}-");
        assert_eq!(
            all.iter().filter(|v| name(v).starts_with(&prefix)).count(),
            1,
            "exactly one vector named {prefix}…"
        );
    }
    assert_eq!(all.len(), 10);
}

#[test]
fn every_tree_is_a_valid_render_v1_tree() {
    let schema: Value =
        serde_json::from_str(&read("schemas/render-v1.schema.json")).expect("the schema is JSON");
    let validator = jsonschema::draft202012::new(&schema).expect("a valid 2020-12 schema");
    for vector in with_trees(&vectors()) {
        for side in ["earlier", "previous", "next"] {
            if let Some(side) = vector.get(side) {
                assert!(
                    validator.is_valid(&side["tree"]),
                    "{}: a tree isn't valid render-v1",
                    name(vector)
                );
            }
        }
    }
}

/// Every expectation is what the identities alone give, by the definitions in
/// `examples/conformance/identity/README.md`.
#[test]
fn every_expectation_follows_from_the_identities() {
    for vector in with_trees(&vectors()) {
        let label = name(vector);
        let previous = occurrences(label, &vector["previous"]);
        let next = occurrences(label, &vector["next"]);
        let expect = &vector["expect"];
        let (before, after) = (names(&previous, true), names(&next, true));

        let kept: BTreeSet<&str> = before.intersection(&after).copied().collect();
        let created: BTreeSet<&str> = after.difference(&before).copied().collect();
        let removed: BTreeSet<&str> = before.difference(&after).copied().collect();
        assert_eq!(owned(&kept), set(&expect["kept"]), "{label}: kept");
        assert_eq!(owned(&created), set(&expect["created"]), "{label}: created");
        assert_eq!(owned(&removed), set(&expect["removed"]), "{label}: removed");

        // Text runs follow their node.
        for (identity, text) in next.iter().filter(|(_, o)| !o.is_node) {
            let parent = text.parent.as_deref().expect("a text run has a parent");
            assert_eq!(
                previous.contains_key(identity),
                previous.contains_key(parent),
                "{label}: the text run {identity} must follow its node"
            );
        }

        // Moved: kept, and in the opposite order to some other kept sibling.
        let mut moved = BTreeSet::new();
        for parent in kept.iter().chain(created.iter()) {
            let siblings: Vec<&str> = children(&next, parent)
                .into_iter()
                .filter(|child| kept.contains(child))
                .collect();
            for a in &siblings {
                for b in &siblings {
                    let (was_a, was_b) = (previous[*a].index, previous[*b].index);
                    let (is_a, is_b) = (next[*a].index, next[*b].index);
                    if a != b && (was_a < was_b) != (is_a < is_b) {
                        moved.insert(a.to_string());
                    }
                }
            }
        }
        assert_eq!(moved, set(&expect["moved"]), "{label}: moved");

        // Handlers.
        let (hp, hn) = (handler_map(&previous), handler_map(&next));
        let same: BTreeSet<String> = hp
            .iter()
            .filter(|(key, handler)| hn.get(*key) == Some(handler))
            .map(|(key, _)| key.clone())
            .collect();
        for key in hp.keys().filter(|key| hn.contains_key(*key)) {
            assert_eq!(
                hp[key], hn[key],
                "{label}: a kept identity's handler changed"
            );
        }
        let new: BTreeSet<String> = hn
            .keys()
            .filter(|k| !hp.contains_key(*k))
            .cloned()
            .collect();
        let gone: BTreeSet<String> = hp
            .keys()
            .filter(|k| !hn.contains_key(*k))
            .cloned()
            .collect();
        let distinct = |map: &BTreeMap<String, String>| {
            map.values().collect::<BTreeSet<_>>().len() == map.len()
        };
        let handlers = &expect["handlers"];
        assert_eq!(same, set(&handlers["same"]), "{label}: same handlers");
        assert_eq!(new, set(&handlers["new"]), "{label}: new handlers");
        assert_eq!(gone, set(&handlers["gone"]), "{label}: gone handlers");
        assert_eq!(
            distinct(&hp) && distinct(&hn),
            handlers["distinctWithinEachTree"],
            "{label}: distinct handlers"
        );

        // Matching by position.
        let positional = &expect["positional"];
        let parent = positional["parent"].as_str().expect("a parent");
        let (was, now) = (children(&previous, parent), children(&next, parent));
        let pairs: Vec<Value> = (0..was.len().max(now.len()))
            .map(|i| serde_json::json!([was.get(i), now.get(i)]))
            .collect();
        assert_eq!(&Value::Array(pairs), &positional["pairs"], "{label}: pairs");
        let correct = was.iter().zip(now.iter()).all(|(a, b)| a == b);
        assert_eq!(
            positional["correct"], correct,
            "{label}: positional.correct"
        );

        // A returning node: present earlier, absent before this render, created now.
        if let Some(earlier) = vector.get("earlier") {
            let earlier = occurrences(label, earlier);
            let he = handler_map(&earlier);
            let recurring: BTreeSet<String> = created
                .iter()
                .filter(|identity| earlier.contains_key(**identity))
                .map(|identity| identity.to_string())
                .collect();
            assert_eq!(recurring, set(&expect["recurring"]), "{label}: recurring");
            assert!(!recurring.is_empty(), "{label}: nothing recurs");
            let same_as_earlier: BTreeSet<String> = hn
                .iter()
                .filter(|(key, handler)| he.get(*key) == Some(handler))
                .map(|(key, _)| key.clone())
                .collect();
            assert_eq!(
                same_as_earlier,
                set(&handlers["sameAsEarlier"]),
                "{label}: sameAsEarlier"
            );
            assert!(
                recurring.iter().all(|r| created.contains(r.as_str())),
                "{label}: a returning identity is created, not kept"
            );
        }
    }
}

/// The failure of matching by position, which the vectors must expose for
/// insertion, removal from the middle, removal from the front and reorder.
#[test]
fn the_vectors_expose_matching_by_position_where_it_is_wrong() {
    let all = vectors();
    assert_eq!(
        find(&all, "C1")["expect"]["positional"]["correct"],
        true,
        "with nothing changed, position and identity agree"
    );
    for prefix in ["C2", "C3", "C4", "C5"] {
        assert_eq!(
            find(&all, prefix)["expect"]["positional"]["correct"],
            false,
            "{prefix}: matching by position must be shown to be wrong"
        );
    }
}

/// C10: equal content, different identity.
#[test]
fn equal_content_does_not_make_equal_identity() {
    let all = vectors();
    let vector = find(&all, "C10");
    let previous = occurrences("C10", &vector["previous"]);
    let texts: BTreeSet<&str> = vector["previous"]["tree"]["root"]["children"][0]["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["children"][0]["text"].as_str().unwrap())
        .collect();
    assert_eq!(texts.len(), 1, "C10's items must show the same text");
    assert!(
        previous.contains_key("page/list/item[a]") && previous.contains_key("page/list/item[b]")
    );
    assert_eq!(
        set(&vector["expect"]["removed"]),
        BTreeSet::from(["page/list/item[a]".to_string()])
    );
}

/// C6: a duplicate identity has no render-v1 tree, and the render is rejected.
#[test]
fn a_duplicate_identity_is_rejected_and_has_no_tree() {
    let all = vectors();
    let vector = find(&all, "C6");
    assert!(vector.get("previous").is_none() && vector.get("next").is_none());
    assert_eq!(vector["expect"]["rejected"], true);
    assert!(vector["expect"]["tree"].is_null());
    let identities: Vec<&str> = vector["wouldBe"]
        .as_array()
        .unwrap()
        .iter()
        .map(|occurrence| occurrence["identity"].as_str().unwrap())
        .collect();
    assert_eq!(
        identities.len(),
        identities.iter().collect::<BTreeSet<_>>().len() + 1,
        "exactly one identity occurs twice"
    );
}
