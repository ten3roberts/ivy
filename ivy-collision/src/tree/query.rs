use smallvec::SmallVec;

use crate::{CollisionTree, CollisionTreeNode, NodeIndex, Visitor};

pub struct TreeQuery<'a, N, V> {
    visitor: V,
    tree: &'a CollisionTree<N>,
    stack: SmallVec<[NodeIndex; 16]>,
}

impl<'a, N, V> TreeQuery<'a, N, V> {
    pub fn new(visitor: V, tree: &'a CollisionTree<N>, root: NodeIndex) -> Self {
        Self {
            visitor,
            tree,
            stack: SmallVec::from_slice(&[root]),
        }
    }
}

impl<'a, N, V> Iterator for TreeQuery<'a, N, V>
where
    N: CollisionTreeNode,
    V: Visitor<'a, N>,
    V::Output: 'a,
{
    type Item = V::Output;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(current) = self.stack.pop() {
            let node = self.tree.node(current).unwrap();
            // If the visitor wants to visit this node, push all children to the
            // stack and visit the node
            if let Some(output) = self.visitor.accept(node, self.tree.objects()) {
                self.stack.extend(node.children().iter().cloned());
                return Some(output);
            }
        }

        None
    }
}
