use smallvec::{Array, SmallVec};

use crate::{NodeIndex, Nodes, Object, Visitor};

pub struct TreeQuery<'a, T, V>
where
    T: Array<Item = Object>,
{
    visitor: V,
    nodes: &'a Nodes<T>,
    stack: SmallVec<[NodeIndex; 16]>,
}

impl<'a, T, V> TreeQuery<'a, T, V>
where
    T: Array<Item = Object>,
{
    pub fn new(visitor: V, nodes: &'a Nodes<T>, root: NodeIndex) -> Self {
        Self {
            visitor,
            nodes,
            stack: SmallVec::from_slice(&[root]),
        }
    }
}

impl<'a, T, V> Iterator for TreeQuery<'a, T, V>
where
    T: Array<Item = Object>,
    V: Visitor<'a>,
{
    type Item = V::Output;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(current) = self.stack.pop() {
            let node = &self.nodes[current];
            // If the visitor wants to visit this node, push all children to the
            // stack and visit the node
            if self.visitor.accept(&node.bounds, node.origin, node.depth) {
                self.stack.extend(node.children_iter());
                return Some(self.visitor.visit(node));
            }
        }

        None
    }
}
