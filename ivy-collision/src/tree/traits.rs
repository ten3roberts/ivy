use ivy_core::Events;
use slotmap::SlotMap;

use crate::{BoundingBox, CollisionTreeObject, Nodes, ObjectData, ObjectIndex};

use super::NodeIndex;

pub trait CollisionTreeNode: 'static + Sized + Send + Sync {
    /// Returns the objects contained in the node
    fn objects(&self) -> &[ObjectIndex];

    fn insert(
        index: NodeIndex,
        nodes: &mut Nodes<Self>,
        object: ObjectIndex,
        nodes: &SlotMap<ObjectIndex, ObjectData>,
    );

    /// Removes an object entity from the node
    fn remove(&mut self, object: ObjectIndex) -> Option<ObjectIndex>;

    /// Returns the node bounds
    fn bounds(&self) -> BoundingBox;

    fn locate(
        index: NodeIndex,
        nodes: &Nodes<Self>,
        object: CollisionTreeObject,
        object_data: &ObjectData,
    ) -> Option<NodeIndex> {
        unimplemented!()
    }

    /// Returns the node's children. If the node is a leaf node, an empty slice
    /// is returned
    fn children(&self) -> &[NodeIndex];

    fn is_leaf(&self) -> bool {
        self.children().is_empty()
    }

    /// `despawned` is set to the number of entities despawned from the world
    ///  Indicates that it has also been removed from data, and should be removed
    ///  from the node.
    ///  The count shall be decremented to account for the detection of removed
    ///  entities.
    fn update(
        index: NodeIndex,
        nodes: &mut Nodes<Self>,
        data: &SlotMap<ObjectIndex, ObjectData>,
        to_refit: &mut Vec<ObjectIndex>,
        despawned: &mut usize,
    );

    fn check_collisions(
        events: &Events,
        index: NodeIndex,
        nodes: &Nodes<Self>,
        data: &SlotMap<ObjectIndex, ObjectData>,
    );
}
