use slotmap::SlotMap;

use crate::{ObjectData, ObjectIndex};

pub trait Visitor<'a, N> {
    /// The visitor for the accepted node
    type Output;
    /// Acceptance function to visit this node. Returns Some<Output> if the node
    /// was accepted
    fn accept(
        &self,
        node: &'a N,
        data: &'a SlotMap<ObjectIndex, ObjectData>,
    ) -> Option<Self::Output>;
}
