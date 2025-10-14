use crate::{BehaviorTreeNode, NodeStatus};

/// Maps a behavior tree context from `T` to `U`
pub struct MapContext<T, U> {
    pub child: Box<dyn BehaviorTreeNode<U>>,
    pub map_fn: fn(&mut T) -> &mut U,
}

impl<T, U> MapContext<T, U> {
    pub fn new(child: impl BehaviorTreeNode<U> + 'static, map_fn: fn(&mut T) -> &mut U) -> Self {
        Self {
            child: Box::new(child),
            map_fn,
        }
    }
}

impl<T, U> BehaviorTreeNode<T> for MapContext<T, U> {
    fn execute(&mut self, ctx: &mut T) -> NodeStatus {
        let sub_ctx = (self.map_fn)(ctx);
        self.child.execute(sub_ctx)
    }

    fn reset_state(&mut self) {
        self.child.reset();
    }
}
