use smallvec::SmallVec;

use crate::{Node, NodeIndex, Nodes, Visitor};

pub struct TreeQuery<'a, N, V> {
    visitor: V,
    nodes: &'a Nodes<N>,
    stack: SmallVec<[NodeIndex; 16]>,
}

impl<'a, N, V> TreeQuery<'a, N, V> {
    pub fn new(visitor: V, nodes: &'a Nodes<N>, root: NodeIndex) -> Self {
        Self {
            visitor,
            nodes,
            stack: SmallVec::from_slice(&[root]),
        }
    }
}

impl<'a, N, V> Iterator for TreeQuery<'a, N, V>
where
    N: Node,
    V: Visitor<'a, N>,
{
    type Item = V::Output;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(current) = self.stack.pop() {
            let node = &self.nodes[current];
            // If the visitor wants to visit this node, push all children to the
            // stack and visit the node
            if let Some(output) = self.visitor.accept(node) {
                self.stack.extend(node.children().iter().cloned());
                return Some(output);
            }
        }

        None
    }
}
