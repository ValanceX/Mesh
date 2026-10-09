//! What render and dispatch produce: the render tree (`render-v1`) and
//! the command intent (docs/manual/runtime.md).
//!
//! Values in them are in the boundary data model (§9.8.1), as
//! `serde_json::Value`s: `null`, booleans, finite numbers (never `-0`),
//! strings, lists and records.

use crate::value::Value as Scoped;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::rc::Rc;

/// A render tree.
#[derive(Debug, Clone, PartialEq)]
pub struct Tree {
    /// Shared (`Rc`), so a tree and the tree an update makes from it hold
    /// the very same node for every part the update left unchanged.
    pub root: Rc<Node>,
}

/// A primitive occurrence. Non-exhaustive: a later MESH may add
/// information, as render-v1 allows.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Node {
    pub key: String,
    pub component: String,
    /// Each written prop's value; an absent prop isn't here.
    pub props: BTreeMap<String, Value>,
    /// The text (§9.7.7) of each prop whose value is a number, a boolean
    /// or `null`, made by the runtime so that a renderer never makes it.
    /// A string prop has no entry (its text is itself), nor has a list or
    /// record prop (it has no text), nor an absent prop.
    pub prop_text: BTreeMap<String, String>,
    /// Each event binding's event name and handler identifier.
    pub events: BTreeMap<String, String>,
    pub children: Vec<TreeChild>,
    /// What this node was computed from, so that an update can reuse it.
    /// Not part of the tree: it never shows in equality or in `render-v1`.
    pub(crate) memo: Memo,
}

/// What a node was computed from (docs/manual/runtime.md, "Update"): the
/// values, in the scope the node was rendered in, of the names its subtree
/// reads from outside it, of the names its own props read, and, for each
/// text run under it, the values its text was computed from and the text.
/// A result computed from the same values of the same names is the result,
/// because an expression is a pure function of the scope names it reads.
#[derive(Debug, Clone, Default)]
pub(crate) struct Memo {
    /// The scope names the whole subtree reads from outside it, and their values.
    pub free: Vec<(String, Scoped)>,
    /// The scope names this node's own props read, and their values.
    pub own: Vec<(String, Scoped)>,
    /// Each text run child's key, the values it was computed from, and its text.
    pub runs: Vec<Run>,
    /// What each repeat among this node's children was made from.
    pub repeats: Vec<RepeatMemo>,
}

/// A text run under a node: its key, the values it was computed from, and its text.
pub(crate) type Run = (String, Vec<(String, Scoped)>, String);

/// What one repeat (`mesh-each`) under a node was made from: where it is,
/// the values of the scope names its key reads besides the item, and each
/// item with its key and the node it made. An update whose items are
/// identical, in a scope its key still reads the same, doesn't evaluate a
/// key, build a path or hash one for them.
#[derive(Debug, Clone)]
pub(crate) struct RepeatMemo {
    pub position: usize,
    pub key_other: Vec<(String, Scoped)>,
    pub items: Vec<RepeatItem>,
}

#[derive(Debug, Clone)]
pub(crate) struct RepeatItem {
    /// The item's declared key, as the repeat canonicalizes it.
    pub canonical: Rc<str>,
    pub item: Scoped,
    pub node: Rc<Node>,
}

impl PartialEq for Memo {
    /// Memos are not part of what a tree is: two trees are equal when their parts are.
    fn eq(&self, _: &Memo) -> bool {
        true
    }
}

/// A node's child.
#[derive(Debug, Clone, PartialEq)]
pub enum TreeChild {
    Node(Rc<Node>),
    Text { key: String, text: String },
}

impl Tree {
    /// The tree as a `render-v1` document.
    pub fn to_json(&self) -> String {
        json!({ "format": "mesh-render", "version": 1, "root": node(&self.root) }).to_string()
    }
}

pub(crate) fn node(node: &Node) -> Value {
    let mut out = json!({
        "type": "node",
        "key": node.key,
        "component": node.component,
        "props": Value::Object(node.props.clone().into_iter().collect::<Map<_, _>>()),
        "events": node.events,
        "children": node.children.iter().map(|child| match child {
            TreeChild::Node(child) => self::node(child),
            TreeChild::Text { key, text } => json!({ "type": "text", "key": key, "text": text }),
        }).collect::<Vec<_>>(),
    });
    // Present exactly when some prop has a text entry, so a node with
    // none is written as it was before `propText` existed.
    if !node.prop_text.is_empty() {
        out["propText"] = json!(node.prop_text);
    }
    out
}

/// A node's child as a render-v1 `node` or `text` object.
pub(crate) fn child(child: &TreeChild) -> Value {
    match child {
        TreeChild::Node(child) => node(child),
        TreeChild::Text { key, text } => json!({ "type": "text", "key": key, "text": text }),
    }
}

/// A command intent: which command a handler invoked, and its evaluated
/// arguments. For the host, never a renderer.
#[derive(Debug, Clone, PartialEq)]
pub struct Intent {
    /// The component whose template declares the command.
    pub component: String,
    /// The command's name.
    pub command: String,
    /// One per parameter, in order: `None` is absent, distinct from
    /// `Some(null)`.
    pub arguments: Vec<Option<Value>>,
}

impl Intent {
    /// The intent in docs/manual/runtime.md's form (`$defs/intent`).
    pub fn to_json(&self) -> String {
        json!({
            "command": { "component": self.component, "name": self.command },
            "arguments": self.arguments.iter().map(|argument| match argument {
                Some(value) => json!({ "value": value }),
                None => json!({ "absent": true }),
            }).collect::<Vec<_>>(),
        })
        .to_string()
    }
}
