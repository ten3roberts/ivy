use slotmap::SlotMap;

use crate::{BoundingBox, Collision, Nodes, ObjectData, ObjectIndex};

use super::NodeIndex;

pub trait CollisionTreeNode: 'static + Sized + Send + Sync {
    /// Returns the objects contained in the node
    fn objects(&self) -> &[ObjectIndex];

    fn insert(
        index: NodeIndex,
        nodes: &mut Nodes<Self>,
        object: ObjectIndex,
        nodes: &mut SlotMap<ObjectIndex, ObjectData>,
    );

    /// Removes an object entity from the node
    fn remove(&mut self, object: ObjectIndex) -> Option<ObjectIndex>;

    /// Returns the node bounds
    fn bounds(&self) -> BoundingBox;

    /// Returns the node's children. If the node is a leaf node, an empty slice
    /// is returned
    fn children(&self) -> &[NodeIndex];

    fn is_leaf(&self) -> bool {
        self.children().is_empty()
    }
}
