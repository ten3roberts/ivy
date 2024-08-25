use ivy_core::gizmos::GizmosSection;
use ivy_core::Color;
use ivy_core::ColorExt;
use slotmap::new_key_type;
use slotmap::Key;
use slotmap::SlotMap;
use std::collections::HashSet;
use std::fmt::Debug;
use std::ops::DerefMut;

use crate::CollisionTreeNode;

use super::BvhNode;
use super::Nodes;
use super::ObjectData;
use super::ObjectIndex;

new_key_type!(
    pub struct NodeIndex;
);

impl NodeIndex {
    /// Creates a new handle that is always invalid and distinct from any non-null
    /// handle. A null key can only be created through this method (or default
    /// initialization of handles made with `new_key_type!`, which calls this
    /// method).
    ///
    /// A null handle is always invalid, but an invalid key (that is, a key that
    /// has been removed from the slot map) does not become a null handle. A null
    /// is safe to use with any safe method of any slot map instance.
    pub fn null() -> Self {
        Key::null()
    }

    /// Checks if a handle is null. There is only a single null key, that is
    /// `a.is_null() && b.is_null()` implies `a == b`.
    pub fn is_null(&self) -> bool {
        Key::is_null(self)
    }
}

pub(crate) struct DebugNode<'a, N> {
    index: NodeIndex,
    nodes: &'a Nodes<N>,
}

impl<'a, N> DebugNode<'a, N> {
    pub(crate) fn new(index: NodeIndex, nodes: &'a Nodes<N>) -> Self {
        Self { index, nodes }
    }
}

impl<'a, N: CollisionTreeNode> Debug for DebugNode<'a, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let node = &self.nodes[self.index];

        let mut children = f.debug_list();
        children.entries(node.children().iter().map(|val| {
            DebugNode::new(*val, self.nodes);
        }));

        let children = children.finish();
        let mut dbg = f.debug_struct("Node");
        dbg.field("children: ", &children);

        dbg.finish()
    }
}
