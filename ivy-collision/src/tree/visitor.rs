use crate::body::BodyMap;

use super::BvhNode;

pub trait TreeVisitor<'a> {
    /// The visitor for the accepted node
    type Output;
    /// Acceptance function to visit this node. Returns Some<Output> if the node
    /// was accepted
    fn accept(&self, node: &'a BvhNode, data: &'a BodyMap) -> Option<Self::Output>;
}
