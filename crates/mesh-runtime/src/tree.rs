//! What render and dispatch produce: the render tree (`render-v1`) and
//! the command intent (docs/manual/runtime.md).
//!
//! Values in them are in the boundary data model (§9.8.1), as
//! `serde_json::Value`s: `null`, booleans, finite numbers (never `-0`),
//! strings, lists and records.

use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

/// A render tree.
#[derive(Debug, Clone, PartialEq)]
pub struct Tree {
    pub root: Node,
}

/// A primitive occurrence.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub key: String,
    pub component: String,
    /// Each written prop's value; an absent prop isn't here.
    pub props: BTreeMap<String, Value>,
    /// Each event binding's event name and handler identifier.
    pub events: BTreeMap<String, String>,
    pub children: Vec<TreeChild>,
}

/// A node's child.
#[derive(Debug, Clone, PartialEq)]
pub enum TreeChild {
    Node(Node),
    Text { key: String, text: String },
}

impl Tree {
    /// The tree as a `render-v1` document.
    pub fn to_json(&self) -> String {
        json!({ "format": "mesh-render", "version": 1, "root": node(&self.root) }).to_string()
    }
}

fn node(node: &Node) -> Value {
    json!({
        "type": "node",
        "key": node.key,
        "component": node.component,
        "props": Value::Object(node.props.clone().into_iter().collect::<Map<_, _>>()),
        "events": node.events,
        "children": node.children.iter().map(|child| match child {
            TreeChild::Node(child) => self::node(child),
            TreeChild::Text { key, text } => json!({ "type": "text", "key": key, "text": text }),
        }).collect::<Vec<_>>(),
    })
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
