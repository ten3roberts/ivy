//! # Character Controller Example
//!
//! Demonstrates a Behavior Tree–driven character controller.
//!
//! This file is referenced by the mdBook documentation through include anchors.

use glam::{vec3, Vec3};
use rand::random;

// ANCHOR:setup ----------------------------------------------------------------

/// Shared node status type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeStatus {
    Success,
    Failure,
    Running,
}

/// Trait for behavior tree nodes.
pub trait BehaviorTreeNode<T> {
    fn execute(&mut self, ctx: &mut T) -> NodeStatus;
    fn reset(&mut self) {}
}

/// Simple wrapper to hold the root node.
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

// ANCHOR_END:setup -------------------------------------------------------------

// ANCHOR:composites -----------------------------------------------------------

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

// ANCHOR_END:composites --------------------------------------------------------

// ANCHOR:leaves ---------------------------------------------------------------

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

// ANCHOR_END:leaves ------------------------------------------------------------

// ANCHOR:controller_context ---------------------------------------------------

/// ---8<--- [start:context]
#[derive(Default, Debug)]
pub struct Input {
    pub move_dir: Vec3,
    pub run: bool,
    pub jump: bool,
}

#[derive(Debug, Default)]
pub struct CharacterController {
    pub pos: Vec3,
    pub vel: Vec3,
    pub grounded: bool,
    pub speed: f32,
    pub jump_cooldown: f32,
}

#[derive(Default, Debug)]
pub struct ControllerCtx {
    pub ctrl: CharacterController,
    pub input: Input,
}
/// ---8<--- [end:context]

// ANCHOR_END:controller_context -----------------------------------------------

// ANCHOR:engine_stubs ---------------------------------------------------------

fn delta_time() -> f32 {
    1.0 / 60.0
}

fn do_ground_raycast(_pos: Vec3) -> bool {
    random::<f32>() > 0.5
}

// ANCHOR_END:engine_stubs ------------------------------------------------------

// ANCHOR:actions --------------------------------------------------------------

/// ---8<--- [start:actions]
fn ground_check(ctx: &mut ControllerCtx) -> NodeStatus {
    ctx.ctrl.grounded = do_ground_raycast(ctx.ctrl.pos);
    if ctx.ctrl.grounded {
        ctx.ctrl.vel.y = 0.0;
    }
    NodeStatus::Success
}

fn move_action(ctx: &mut ControllerCtx) -> NodeStatus {
    if !ctx.ctrl.grounded {
        return NodeStatus::Failure;
    }
    if ctx.input.move_dir.length_squared() < 1e-4 {
        return NodeStatus::Success;
    }
    ctx.ctrl.vel = ctx.input.move_dir.normalize_or_zero() * ctx.ctrl.speed;
    ctx.ctrl.pos += ctx.ctrl.vel * delta_time();
    println!("Moving → pos {:?}", ctx.ctrl.pos);
    NodeStatus::Success
}

fn run_action(ctx: &mut ControllerCtx) -> NodeStatus {
    if !ctx.input.run {
        ctx.ctrl.speed = 5.0;
        return NodeStatus::Failure;
    }
    ctx.ctrl.speed = 8.0;
    println!("Running!");
    NodeStatus::Success
}

fn jump_action(ctx: &mut ControllerCtx) -> NodeStatus {
    if !ctx.input.jump || !ctx.ctrl.grounded {
        return NodeStatus::Failure;
    }
    println!("Jump!");
    ctx.ctrl.vel.y = 5.0;
    ctx.ctrl.grounded = false;
    ctx.ctrl.jump_cooldown = 0.5;
    NodeStatus::Success
}

fn fall_action(ctx: &mut ControllerCtx) -> NodeStatus {
    if ctx.ctrl.grounded {
        return NodeStatus::Failure;
    }
    ctx.ctrl.vel.y -= 9.81 * delta_time();
    ctx.ctrl.pos += ctx.ctrl.vel * delta_time();
    println!("Falling... pos {:?}", ctx.ctrl.pos);
    NodeStatus::Running
}
/// ---8<--- [end:actions]

// ANCHOR_END:actions -----------------------------------------------------------

// ANCHOR:tree_build -----------------------------------------------------------

/// ---8<--- [start:tree]
pub fn build_character_tree() -> BehaviorTree<ControllerCtx> {
    let jump = Box::new(Action::new(jump_action));
    let fall = Box::new(Action::new(fall_action));
    let ground = Box::new(Action::new(ground_check));
    let run = Box::new(Action::new(run_action));
    let mv = Box::new(Action::new(move_action));

    let move_seq = Box::new(Sequence::new(vec![run, mv]));
    let air_or_ground = Box::new(Selector::new(vec![fall, move_seq]));
    let body = Box::new(Sequence::new(vec![ground, air_or_ground]));
    let root = Box::new(Selector::new(vec![jump, body]));

    BehaviorTree::new(root)
}
/// ---8<--- [end:tree]

// endregion:tree_build --------------------------------------------------------

// ANCHOR:loop -----------------------------------------------------------------

/// ---8<--- [start:loop]
pub fn simulate_controller() {
    let mut ctx = ControllerCtx::default();
    ctx.input.move_dir = vec3(1.0, 0.0, 0.0);
    let mut tree = build_character_tree();

    for frame in 0..10 {
        println!("Frame {}", frame);
        let status = tree.execute(&mut ctx);
        println!("→ {:?}\n{:?}", status, ctx.ctrl);

        if ctx.ctrl.jump_cooldown > 0.0 {
            ctx.ctrl.jump_cooldown -= delta_time();
        }

        ctx.input.jump = frame == 2;
        ctx.input.run = frame >= 5;
    }
}
/// ---8<--- [end:loop]

// endregion:loop --------------------------------------------------------------

fn main() {
    simulate_controller();
}
