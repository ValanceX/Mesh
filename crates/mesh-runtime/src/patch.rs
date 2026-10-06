//! Patches (`render-patch-v1`): what turns one render tree into the next
//! when both come from one program. The runtime's `update` produces them;
//! a renderer applies them in order.
//!
//! A patch list is correct by one law: applying it to the previous tree
//! gives exactly the tree a full render of the new snapshot gives. The
//! encoding of a list is not unique, so a renderer never compares lists, it
//! applies them. The operations change a prop or a text, and insert, remove
//! and move a node or text among its siblings by key; a tree that none of
//! them can turn into the next (a different root, or a node whose event
//! bindings differ, which one program never produces) is a `replace`.

use crate::tree::{child, Node, Tree, TreeChild};
use serde_json::{json, Value};

/// One operation of a patch list.
#[derive(Debug, Clone, PartialEq)]
pub enum Patch {
    /// The prop now has this value, and this text if it has one.
    SetProp {
        key: String,
        prop: String,
        value: Value,
        prop_text: Option<String>,
    },
    /// The prop is now absent.
    RemoveProp { key: String, prop: String },
    /// A text child's new text.
    SetText { key: String, text: String },
    /// A node or text run, with everything under it, is new under the node
    /// `parent`: before its child `before`, or last when there is none.
    Insert {
        parent: String,
        before: Option<String>,
        child: TreeChild,
    },
    /// The node or text run, with everything under it, is gone.
    Remove { key: String },
    /// The same node or text run, kept, now stands before its sibling
    /// `before`, or last when there is none.
    Move { key: String, before: Option<String> },
    /// The tree is this one: the renderer draws it afresh, as for a
    /// program change, reusing nothing.
    Replace { tree: Tree },
}

/// The patches that turn `old` into `new`.
pub fn diff(old: &Tree, new: &Tree) -> Vec<Patch> {
    let mut patches = Vec::new();
    if diff_node(&old.root, &new.root, &mut patches).is_none() {
        return vec![Patch::Replace { tree: new.clone() }];
    }
    patches
}

/// `None` when the two nodes differ in a way no patch expresses: a
/// different key, component or event binding.
fn diff_node(old: &Node, new: &Node, out: &mut Vec<Patch>) -> Option<()> {
    if !same_part(old, new) {
        return None;
    }
    for (name, value) in &new.props {
        if old.props.get(name) != Some(value) || old.prop_text.get(name) != new.prop_text.get(name)
        {
            out.push(Patch::SetProp {
                key: new.key.clone(),
                prop: name.clone(),
                value: value.clone(),
                prop_text: new.prop_text.get(name).cloned(),
            });
        }
    }
    for name in old
        .props
        .keys()
        .filter(|name| !new.props.contains_key(*name))
    {
        out.push(Patch::RemoveProp {
            key: new.key.clone(),
            prop: name.clone(),
        });
    }
    diff_children(&new.key, &old.children, &new.children, out)
}

/// Whether `new` can be `old` kept: the same key, component and bindings.
/// Only props and children can then differ, and patches say how.
fn same_part(old: &Node, new: &Node) -> bool {
    old.key == new.key && old.component == new.component && old.events == new.events
}

fn key_of(part: &TreeChild) -> &str {
    match part {
        TreeChild::Node(node) => &node.key,
        TreeChild::Text { key, .. } => key,
    }
}

/// Whether the old child and the new child are one part, kept: a node and
/// a node of the same key, component and bindings, or two text runs of the
/// same key. A key in both trees that isn't compatible is a part removed
/// and a part inserted, as the contract has it for a key whose component
/// changes.
fn kept(old: &TreeChild, new: &TreeChild) -> bool {
    match (old, new) {
        (TreeChild::Node(old), TreeChild::Node(new)) => same_part(old, new),
        (TreeChild::Text { key: old, .. }, TreeChild::Text { key: new, .. }) => old == new,
        _ => false,
    }
}

/// The patches for one node's children: matched by key and nothing else
/// (never by position, MESH §9.10), as removals first, then the
/// insertions and moves that put the kept parts in the new order, then the
/// changes inside each kept part.
fn diff_children(
    parent: &str,
    old: &[TreeChild],
    new: &[TreeChild],
    out: &mut Vec<Patch>,
) -> Option<()> {
    use std::collections::HashMap;

    let old_at: HashMap<&str, &TreeChild> = old.iter().map(|part| (key_of(part), part)).collect();
    let new_at: HashMap<&str, &TreeChild> = new.iter().map(|part| (key_of(part), part)).collect();
    let is_kept = |key: &str| match (old_at.get(key), new_at.get(key)) {
        (Some(old), Some(new)) => kept(old, new),
        _ => false,
    };

    for part in old.iter().filter(|part| !is_kept(key_of(part))) {
        out.push(Patch::Remove {
            key: key_of(part).to_string(),
        });
    }

    // The kept parts, in their old order: what the DOM, say, holds now.
    let mut current: Vec<&str> = old.iter().map(key_of).filter(|key| is_kept(key)).collect();
    for (at, part) in new.iter().enumerate() {
        let key = key_of(part);
        if is_kept(key) {
            if current.get(at) != Some(&key) {
                let from = current
                    .iter()
                    .position(|candidate| *candidate == key)
                    .expect("a kept part is in the current order");
                current.remove(from);
                current.insert(at, key);
                out.push(Patch::Move {
                    key: key.to_string(),
                    before: current.get(at + 1).map(|key| (*key).to_string()),
                });
            }
        } else {
            current.insert(at, key);
            out.push(Patch::Insert {
                parent: parent.to_string(),
                before: current.get(at + 1).map(|key| (*key).to_string()),
                child: part.clone(),
            });
        }
    }

    for part in new.iter().filter(|part| is_kept(key_of(part))) {
        match (old_at[key_of(part)], part) {
            (TreeChild::Node(old), TreeChild::Node(new)) => diff_node(old, new, out)?,
            (TreeChild::Text { text: old, .. }, TreeChild::Text { key, text }) => {
                if old != text {
                    out.push(Patch::SetText {
                        key: key.clone(),
                        text: text.clone(),
                    });
                }
            }
            _ => unreachable!("a kept part is a node and a node, or a text and a text"),
        }
    }
    Some(())
}

/// A patch list as a `render-patch-v1` document.
pub fn patches_to_json(patches: &[Patch]) -> String {
    json!({
        "format": "mesh-render-patch",
        "version": 1,
        "patches": patches.iter().map(op).collect::<Vec<_>>(),
    })
    .to_string()
}

fn op(patch: &Patch) -> Value {
    match patch {
        Patch::SetProp {
            key,
            prop,
            value,
            prop_text,
        } => {
            let mut out = json!({ "op": "setProp", "key": key, "prop": prop, "value": value });
            if let Some(text) = prop_text {
                out["propText"] = json!(text);
            }
            out
        }
        Patch::RemoveProp { key, prop } => json!({ "op": "removeProp", "key": key, "prop": prop }),
        Patch::SetText { key, text } => json!({ "op": "setText", "key": key, "text": text }),
        Patch::Insert {
            parent,
            before,
            child: part,
        } => {
            let mut out = json!({ "op": "insert", "parent": parent, "node": child(part) });
            if let Some(before) = before {
                out["before"] = json!(before);
            }
            out
        }
        Patch::Remove { key } => json!({ "op": "remove", "key": key }),
        Patch::Move { key, before } => {
            let mut out = json!({ "op": "move", "key": key });
            if let Some(before) = before {
                out["before"] = json!(before);
            }
            out
        }
        Patch::Replace { tree } => {
            json!({ "op": "replace", "tree": serde_json::from_str::<Value>(&tree.to_json()).expect("a tree is JSON") })
        }
    }
}
