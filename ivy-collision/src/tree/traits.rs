use hecs::{Column, Entity};
use ivy_base::Events;
use slotmap::SlotMap;

use crate::{BoundingBox, Collider, Nodes, Object, ObjectData, ObjectIndex};

use super::NodeIndex;

pub trait CollisionTreeNode: 'static + Sized + Send + Sync {
    /// Returns the objects contained in the node
    fn objects(&self) -> &[Object];

    fn insert(
        index: NodeIndex,
        nodes: &mut Nodes<Self>,
        object: Object,
        data: &SlotMap<ObjectIndex, ObjectData>,
    );
    /// Removes an object entity from the node
    fn remove(&mut self, entity: Entity) -> Option<Object>;

    /// Returns the node bounds
    fn bounds(&self) -> BoundingBox;

    /// Returns the node's children. If the node is a leaf node, an empty slice
    /// is returned
    fn children(&self) -> &[NodeIndex];

    fn is_leaf(&self) -> bool {
        self.children().is_empty()
    }

    fn update(
        index: NodeIndex,
        nodes: &mut Nodes<Self>,
        data: &SlotMap<ObjectIndex, ObjectData>,
        to_refit: &mut Vec<Object>,
    );

    fn check_collisions(
        colliders: &Column<Collider>,
        events: &Events,
        index: NodeIndex,
        nodes: &Nodes<Self>,
        data: &SlotMap<ObjectIndex, ObjectData>,
    );
}
