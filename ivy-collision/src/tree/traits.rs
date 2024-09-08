use slotmap::SlotMap;

use crate::{Body, BodyIndex, BoundingBox, Nodes};

use super::NodeIndex;

pub trait CollisionTreeNode: 'static + Sized + Send + Sync {
    /// Returns the objects contained in the node
    fn objects(&self) -> &[BodyIndex];

    fn insert(
        index: NodeIndex,
        nodes: &mut Nodes<Self>,
        object: BodyIndex,
        nodes: &mut SlotMap<BodyIndex, Body>,
    );

    /// Removes an object entity from the node
    fn remove(&mut self, object: BodyIndex) -> Option<BodyIndex>;

    /// Returns the node bounds
    fn bounds(&self) -> BoundingBox;

    /// Returns the node's children. If the node is a leaf node, an empty slice
    /// is returned
    fn children(&self) -> &[NodeIndex];

    fn is_leaf(&self) -> bool {
        self.children().is_empty()
    }
}
