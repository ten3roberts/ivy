use ivy_core::Position;
use smallvec::Array;

use crate::{Cube, Node, Object};

pub trait Visitor<'a> {
    type Output;
    /// Acceptance function to visit this node
    fn accept(&self, bounds: &Cube, origin: Position, depth: usize) -> bool;

    /// Function to be called for each node to be visited, returning result
    fn visit<T: Array<Item = Object>>(&self, node: &'a Node<T>) -> Self::Output;
}
