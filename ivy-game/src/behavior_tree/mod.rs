pub mod map_context;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeStatus {
    Success,
    Failure,
    Running,
}

/// Trait for a Behavior Tree node.
/// Each node mutably borrows the shared game/AI context `T`.
pub trait BehaviorTreeNode<T>: Send + Sync {
    fn execute(&mut self, ctx: &mut T) -> NodeStatus;
    fn reset(&mut self) {}
}

/// A top-level tree wrapper. (Optional — could just use the root node directly.)
pub struct BehaviorTree<T> {
    pub root: Box<dyn BehaviorTreeNode<T>>,
}

impl<T> BehaviorTree<T> {
    pub fn new(root: impl BehaviorTreeNode<T> + 'static) -> Self {
        Self {
            root: Box::new(root),
        }
    }

    pub fn execute(&mut self, ctx: &mut T) -> NodeStatus {
        self.root.execute(ctx)
    }
}

//
// ─── COMPOSITE NODES ───────────────────────────────────────────────────────────
//

/// Sequence: run children in order, stop on first failure or running.
pub struct Sequence<T> {
    pub children: Vec<Box<dyn BehaviorTreeNode<T>>>,
    pub current: usize,
}

impl<T> Sequence<T> {
    pub fn new(children: Vec<Box<dyn BehaviorTreeNode<T>>>) -> Self {
        Self {
            children,
            current: 0,
        }
    }
}

impl<T> BehaviorTreeNode<T> for Sequence<T> {
    fn execute(&mut self, ctx: &mut T) -> NodeStatus {
        while self.current < self.children.len() {
            let status = self.children[self.current].execute(ctx);
            match status {
                NodeStatus::Success => self.current += 1,
                NodeStatus::Failure => {
                    self.current = 0;
                    return NodeStatus::Failure;
                }
                NodeStatus::Running => return NodeStatus::Running,
            }
        }
        self.current = 0;
        NodeStatus::Success
    }

    fn reset(&mut self) {
        self.current = 0;
        for child in &mut self.children {
            child.reset();
        }
    }
}

/// Selector: run children in order, stop on first success or running.
pub struct Selector<T> {
    pub children: Vec<Box<dyn BehaviorTreeNode<T>>>,
    pub current: usize,
}

impl<T> Selector<T> {
    pub fn new(children: Vec<Box<dyn BehaviorTreeNode<T>>>) -> Self {
        Self {
            children,
            current: 0,
        }
    }
}

impl<T> BehaviorTreeNode<T> for Selector<T> {
    fn execute(&mut self, ctx: &mut T) -> NodeStatus {
        while self.current < self.children.len() {
            let status = self.children[self.current].execute(ctx);
            match status {
                NodeStatus::Failure => self.current += 1,
                NodeStatus::Success => {
                    self.current = 0;
                    return NodeStatus::Success;
                }
                NodeStatus::Running => return NodeStatus::Running,
            }
        }
        self.current = 0;
        NodeStatus::Failure
    }

    fn reset(&mut self) {
        self.current = 0;
        for child in &mut self.children {
            child.reset();
        }
    }
}

/// Parallel: run all children; configurable success condition.
pub struct Parallel<T> {
    pub children: Vec<Box<dyn BehaviorTreeNode<T>>>,
    pub success_threshold: usize,
}

impl<T> Parallel<T> {
    pub fn new(children: Vec<Box<dyn BehaviorTreeNode<T>>>, success_threshold: usize) -> Self {
        Self {
            children,
            success_threshold,
        }
    }
}

impl<T> BehaviorTreeNode<T> for Parallel<T> {
    fn execute(&mut self, ctx: &mut T) -> NodeStatus {
        let mut successes = 0;
        let mut failures = 0;

        for child in &mut self.children {
            match child.execute(ctx) {
                NodeStatus::Success => successes += 1,
                NodeStatus::Failure => failures += 1,
                NodeStatus::Running => {}
            }
        }

        if successes >= self.success_threshold {
            NodeStatus::Success
        } else if (self.children.len() - failures) < self.success_threshold {
            NodeStatus::Failure
        } else {
            NodeStatus::Running
        }
    }

    fn reset(&mut self) {
        for c in &mut self.children {
            c.reset();
        }
    }
}

//
// ─── DECORATORS ───────────────────────────────────────────────────────────────
//

pub struct Inverter<T> {
    pub child: Box<dyn BehaviorTreeNode<T>>,
}

impl<T> Inverter<T> {
    pub fn new(child: impl BehaviorTreeNode<T> + 'static) -> Self {
        Self {
            child: Box::new(child),
        }
    }
}

impl<T> BehaviorTreeNode<T> for Inverter<T> {
    fn execute(&mut self, ctx: &mut T) -> NodeStatus {
        match self.child.execute(ctx) {
            NodeStatus::Success => NodeStatus::Failure,
            NodeStatus::Failure => NodeStatus::Success,
            NodeStatus::Running => NodeStatus::Running,
        }
    }

    fn reset(&mut self) {
        self.child.reset();
    }
}

pub struct Succeeder<T> {
    pub child: Box<dyn BehaviorTreeNode<T>>,
}

impl<T> Succeeder<T> {
    pub fn new(child: impl BehaviorTreeNode<T> + 'static) -> Self {
        Self {
            child: Box::new(child),
        }
    }
}

impl<T> BehaviorTreeNode<T> for Succeeder<T> {
    fn execute(&mut self, ctx: &mut T) -> NodeStatus {
        let _ = self.child.execute(ctx);
        NodeStatus::Success
    }

    fn reset(&mut self) {
        self.child.reset();
    }
}

//
// ─── LEAF NODES (ACTIONS & CONDITIONS) ─────────────────────────────────────────
//

pub struct Condition<T> {
    pub predicate: fn(&T) -> bool,
}

impl<T> Condition<T> {
    pub fn new(predicate: fn(&T) -> bool) -> Self {
        Self { predicate }
    }
}

impl<T> BehaviorTreeNode<T> for Condition<T> {
    fn execute(&mut self, ctx: &mut T) -> NodeStatus {
        if (self.predicate)(ctx) {
            NodeStatus::Success
        } else {
            NodeStatus::Failure
        }
    }
}

pub struct Action<T> {
    pub func: fn(&mut T) -> NodeStatus,
}

impl<T> Action<T> {
    pub fn new(func: fn(&mut T) -> NodeStatus) -> Self {
        Self { func }
    }
}

impl<T> BehaviorTreeNode<T> for Action<T> {
    fn execute(&mut self, ctx: &mut T) -> NodeStatus {
        (self.func)(ctx)
    }
}

pub struct WaitForFrames {
    pub remaining: u32,
}

impl WaitForFrames {
    pub fn new(frames: u32) -> Self {
        Self { remaining: frames }
    }
}

impl<T> BehaviorTreeNode<T> for WaitForFrames {
    fn execute(&mut self, _ctx: &mut T) -> NodeStatus {
        if self.remaining == 0 {
            NodeStatus::Success
        } else {
            self.remaining -= 1;
            NodeStatus::Running
        }
    }
}
