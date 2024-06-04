use glam::Vec3;
use ivy_base::Events;
use slotmap::SlotMap;

use crate::{BoundingBox, Nodes, Object, ObjectData, ObjectIndex};

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
    fn remove(&mut self, object: Object) -> Option<Object>;

    /// Returns the node bounds
    fn bounds(&self) -> BoundingBox;

    fn locate(
        index: NodeIndex,
        nodes: &Nodes<Self>,
        object: Object,
        object_data: &ObjectData,
    ) -> Option<NodeIndex> {
        let node = &nodes[index];
        if node.bounds().contains(object_data.bounds) {
            let children = node.children();
            if children.is_empty() {
                node.objects()
                    .iter()
                    .find(|val| **val == object)
                    .map(|_| index)
            } else {
                children
                    .iter()
                    .find_map(|val| Self::locate(*val, nodes, object, object_data))
            }
        } else {
            None
        }
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
        to_refit: &mut Vec<Object>,
        despawned: &mut usize,
    );

    fn check_collisions(
        events: &Events,
        index: NodeIndex,
        nodes: &Nodes<Self>,
        data: &SlotMap<ObjectIndex, ObjectData>,
    );
}

/// Dummy collision tree which does nothing
impl CollisionTreeNode for () {
    fn objects(&self) -> &[Object] {
        &[]
    }

    fn insert(_: NodeIndex, _: &mut Nodes<Self>, _: Object, _: &SlotMap<ObjectIndex, ObjectData>) {}

    fn remove(&mut self, _: Object) -> Option<Object> {
        None
    }

    fn bounds(&self) -> BoundingBox {
        BoundingBox::new(Vec3::ZERO, Vec3::ZERO)
    }

    fn children(&self) -> &[NodeIndex] {
        todo!()
    }

    fn update(
        _: NodeIndex,
        _: &mut Nodes<Self>,
        _: &SlotMap<ObjectIndex, ObjectData>,
        _: &mut Vec<Object>,
        _: &mut usize,
    ) {
    }

    fn check_collisions(
        _: &Events,
        _: NodeIndex,
        _: &Nodes<Self>,
        _: &SlotMap<ObjectIndex, ObjectData>,
    ) {
    }
}
