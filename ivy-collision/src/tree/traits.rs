use crate::{body::BodyIndex, BoundingBox};

use super::NodeIndex;

// TODO: remove
pub trait CollisionTreeNode: 'static + Sized + Send + Sync {
    /// Returns the objects contained in the node
    fn bodies(&self) -> &[BodyIndex];

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
