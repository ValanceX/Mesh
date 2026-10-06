//! Patches (`render-patch-v1`): what turns one render tree into the next
//! when both come from one program. The runtime's `update` produces them;
//! a renderer applies them in order.
//!
//! A patch list is correct by one law: applying it to the previous tree
//! gives exactly the tree a full render of the new snapshot gives. The
//! encoding of a list is not unique, so a renderer never compares lists, it
//! applies them. This version has the operations that change a prop or a
//! text; a change of structure is a `replace` of the whole tree.

use crate::tree::{Node, Tree, TreeChild};
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

/// `None` when the two nodes differ in a way no prop or text patch
/// expresses: a different key, component, event binding or child shape.
fn diff_node(old: &Node, new: &Node, out: &mut Vec<Patch>) -> Option<()> {
    if old.key != new.key || old.component != new.component || old.events != new.events {
        return None;
    }
    if old.children.len() != new.children.len() {
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
    for (old, new) in old.children.iter().zip(&new.children) {
        match (old, new) {
            (TreeChild::Node(old), TreeChild::Node(new)) => diff_node(old, new, out)?,
            (
                TreeChild::Text {
                    key: old_key,
                    text: old,
                },
                TreeChild::Text { key, text },
            ) if old_key == key => {
                if old != text {
                    out.push(Patch::SetText {
                        key: key.clone(),
                        text: text.clone(),
                    });
                }
            }
            _ => return None,
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
        Patch::Replace { tree } => {
            json!({ "op": "replace", "tree": serde_json::from_str::<Value>(&tree.to_json()).expect("a tree is JSON") })
        }
    }
}
