use hecs::Entity;
use ivy_base::Position;
use ultraviolet::Vec3;

use crate::{Cube, Object};

use super::NodeIndex;

// pub trait NodeArena<N> {
//     fn get(&self, index: &NodeIndex) -> &N;
//     fn get_mut(&mut self, index: &NodeIndex) -> &mut N;
//     fn insert(&self, node: N) -> NodeIndex;
// }

pub trait Node {
    type SplitOutput: IntoIterator<Item = Self>;
    /// Returns the objects contained in the node
    fn objects(&self) -> &[Object];

    fn entity_count(&self) -> usize;
    /// Returns true if the object is considered contained in node
    fn contains(&self, object: &Object) -> bool;
    /// Returns true if a point is contained within the node
    fn contains_point(&self, point: Vec3) -> bool;
    /// Sets/updates the value of the object already stored within
    fn set(&mut self, object: Object, iteration: usize);
    /// Adds a new object entity to the node.
    /// If it was not possible to add the object, the object is returned as an Err
    /// variant.
    fn try_add(&mut self, object: Object) -> Result<(), Object>;
    /// Removes an object entity from the node
    fn remove(&mut self, entity: Entity) -> Option<Object>;
    /// Returns the node origin
    fn origin(&self) -> Position;
    /// Returns the node bounds
    fn bounds(&self) -> Cube;

    /// Returns the node's children. If the node is a leaf node, and empty slice
    /// is returned
    fn children(&self) -> &[NodeIndex];

    fn is_leaf(&self) -> bool {
        self.children().is_empty()
    }

    /// Set the children of the node. Is called after [`split`] to assign the
    /// generated node indices.
    fn set_children(&mut self, children: &[NodeIndex]);

    /// Splits the node returning the new children and pushed the objects in
    /// need of reallocation to the [`popped`] list. The children will then be
    /// inserted into the arena and then set to the node by [`Node::set_children`]
    fn split(&mut self, popped: &mut Vec<Object>) -> Self::SplitOutput;
}
