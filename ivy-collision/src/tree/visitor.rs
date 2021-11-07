pub trait Visitor<'a, N> {
    type Output;
    /// Acceptance function to visit this node. Returns Some<Output> if the node
    /// was accepted
    fn accept(&self, node: &'a N) -> Option<Self::Output>;
}
