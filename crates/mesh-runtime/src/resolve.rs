//! Event resolution (spec §9.9): which binding, if any, one interaction
//! reaches.
//!
//! The rule is MESH's and needs nothing but a render tree and the
//! interaction: it examines the innermost interacted node, then its
//! ancestors towards the root, and the first node whose primitive defines
//! the interaction's applicable event and which binds that event receives
//! it. Resolution stops there, so an interaction reaches at most one
//! binding, and so at most one intent. It is not DOM bubbling: there are
//! no phases, and nothing an application writes changes where it stops.
//!
//! This is the reference implementation. A renderer implements the same
//! rule for its target, and checks itself against the conformance vectors
//! in `examples/conformance/events/`; it doesn't need this code.

use crate::tree::{Node, Tree, TreeChild};
use std::collections::BTreeMap;

/// One interaction, in the only terms MESH resolves it by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interaction {
    /// The key of the innermost node, or text run, the interaction is on.
    /// A text run's interaction is its parent node's.
    pub target: String,
    /// For each primitive component, the one event of that component the
    /// interaction constitutes, by the primitive's definition (§9.9). A
    /// component with no entry has no applicable event.
    pub applicable: BTreeMap<String, String>,
}

/// The binding an interaction resolved to.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Resolved {
    /// The key of the node that received the interaction.
    pub node: String,
    /// The event of that node's primitive it received it as.
    pub event: String,
    /// The binding's handler identifier: what a renderer reports, and a
    /// host dispatches.
    pub handler: String,
}

/// Why an interaction can't be resolved against a tree.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResolveError {
    /// The target key names no node or text run in the tree.
    UnknownTarget,
}

/// Resolves `interaction` against `tree` (§9.9). `Ok(None)` means that no
/// node on the path qualifies, so nothing is reported or dispatched.
pub fn resolve(tree: &Tree, interaction: &Interaction) -> Result<Option<Resolved>, ResolveError> {
    let mut path = Vec::new();
    if !find(&tree.root, &interaction.target, &mut path) {
        return Err(ResolveError::UnknownTarget);
    }
    // `path` runs from the root to the innermost interacted node.
    for node in path.iter().rev() {
        let Some(event) = interaction.applicable.get(&node.component) else {
            continue;
        };
        if let Some(handler) = node.events.get(event) {
            return Ok(Some(Resolved {
                node: node.key.clone(),
                event: event.clone(),
                handler: handler.clone(),
            }));
        }
    }
    Ok(None)
}

/// Pushes the nodes from `node` down to the one `target` names, or whose
/// text run it names; false, with `path` unchanged, if it isn't below.
fn find<'t>(node: &'t Node, target: &str, path: &mut Vec<&'t Node>) -> bool {
    path.push(node);
    if node.key == target {
        return true;
    }
    for child in &node.children {
        match child {
            TreeChild::Text { key, .. } if key == target => return true,
            TreeChild::Text { .. } => {}
            TreeChild::Node(child) => {
                if find(child, target, path) {
                    return true;
                }
            }
        }
    }
    path.pop();
    false
}
