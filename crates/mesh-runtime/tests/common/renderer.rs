//! The reference renderer (D10), test-only: prints a render tree as
//! HTML-like text, so an expected rendering can be a committed file, and
//! compares two trees key by key, as a renderer reconciling in place
//! would (D6).
//!
//! It draws what it's given and computes nothing: prop values are printed
//! as the JSON they are, and text runs as their strings.

use mesh_runtime::{Node, Tree, TreeChild};
use std::collections::BTreeMap;
use std::fmt::Write;

/// The tree as indented, HTML-like text: one node per line, props in
/// code-point order as `name=<json>`, events as `on.<event>`, text runs
/// quoted. Keys aren't printed: they're opaque, and change with the
/// program.
pub fn print(tree: &Tree) -> String {
    let mut out = String::new();
    node(&tree.root, 0, &mut out);
    out
}

fn node(node: &Node, depth: usize, out: &mut String) {
    let indent = "  ".repeat(depth);
    let mut tag = format!("{indent}<{}", node.component);
    for (name, value) in &node.props {
        write!(tag, " {name}={value}").unwrap();
    }
    for event in node.events.keys() {
        write!(tag, " on.{event}").unwrap();
    }
    if node.children.is_empty() {
        out.push_str(&tag);
        out.push_str(" />\n");
        return;
    }
    out.push_str(&tag);
    out.push_str(">\n");
    for child in &node.children {
        match child {
            TreeChild::Node(child) => self::node(child, depth + 1, out),
            TreeChild::Text { text, .. } => {
                writeln!(out, "{indent}  {}", serde_json::to_string(text).unwrap()).unwrap();
            }
        }
    }
    writeln!(out, "{indent}</{}>", node.component).unwrap();
}

/// What a part of the tree is, by key: where it is (for people), and its
/// props or text (for comparing).
enum Part {
    Node {
        at: String,
        props: BTreeMap<String, serde_json::Value>,
    },
    Text {
        at: String,
        text: String,
    },
}

fn parts(node: &Node, at: String, out: &mut BTreeMap<String, Part>) {
    out.insert(
        node.key.clone(),
        Part::Node {
            at: at.clone(),
            props: node.props.clone(),
        },
    );
    for (index, child) in node.children.iter().enumerate() {
        match child {
            TreeChild::Node(child) => {
                parts(child, format!("{at}/{}[{index}]", child.component), out)
            }
            TreeChild::Text { key, text } => {
                out.insert(
                    key.clone(),
                    Part::Text {
                        at: format!("{at}/text[{index}]"),
                        text: text.clone(),
                    },
                );
            }
        }
    }
}

/// Every prop and text run whose value changed from `old` to `new`,
/// matched by key, one per line, in document order of `new`. Panics if
/// the trees' keys differ: two renders of one program always have the
/// same keys.
pub fn compare(old: &Tree, new: &Tree) -> String {
    let (mut before, mut after) = (BTreeMap::new(), BTreeMap::new());
    parts(&old.root, old.root.component.clone(), &mut before);
    parts(&new.root, new.root.component.clone(), &mut after);
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>(),
        "two renders of one program have the same keys"
    );
    let mut changes: Vec<(String, String)> = Vec::new();
    for (key, part) in &after {
        match (&before[key], part) {
            (Part::Node { props: a, .. }, Part::Node { at, props: b }) => {
                for name in a
                    .keys()
                    .chain(b.keys())
                    .collect::<std::collections::BTreeSet<_>>()
                {
                    let (x, y) = (a.get(name), b.get(name));
                    if x != y {
                        let show = |v: Option<&serde_json::Value>| {
                            v.map_or("(absent)".to_string(), ToString::to_string)
                        };
                        changes.push((
                            at.clone(),
                            format!("{at} {name}: {} -> {}", show(x), show(y)),
                        ));
                    }
                }
            }
            (Part::Text { text: a, .. }, Part::Text { at, text: b }) if a != b => {
                changes.push((
                    at.clone(),
                    format!(
                        "{at}: {} -> {}",
                        serde_json::to_string(a).unwrap(),
                        serde_json::to_string(b).unwrap()
                    ),
                ));
            }
            (Part::Text { .. }, Part::Text { .. }) => {}
            _ => panic!("a key changed kind"),
        }
    }
    changes.sort();
    changes.into_iter().map(|(_, line)| line + "\n").collect()
}
