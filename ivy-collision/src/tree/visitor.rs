use slotmap::SlotMap;

use crate::body::{Body, BodyIndex, BodyMap};

use super::BvhNode;

pub trait Visitor<'a> {
    /// The visitor for the accepted node
    type Output;
    /// Acceptance function to visit this node. Returns Some<Output> if the node
    /// was accepted
    fn accept(&self, node: &'a BvhNode, data: &'a BodyMap) -> Option<Self::Output>;
}
