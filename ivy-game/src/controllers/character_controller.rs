use flax::{component, system};
use glam::{vec3, Vec2, Vec3};
use ivy_core::{
    math::Axis2D,
    plugin::{Plugin, PluginContext},
    Bundle,
};
use ivy_input::{
    components::input_state,
    types::{Key, NamedKey},
    Action, BindingExt, CursorMoveBinding, InputState, KeyBinding,
};

use crate::behavior_tree::{BehaviorTree, BehaviorTreeNode, Selector};
use crate::controllers::camera_controller::{camera_look_data, CameraLookData};
use crate::navigation::movement_direction;
use ivy_core::components::{position, rotation};
use ivy_physics::components::velocity;

component! {
    character_controller: CharacterController,
    movement_input: Vec2,
    yaw_input: f32,
    pitch_input: f32,
    shift_input: bool,
    jump_input: bool,
    interact_input: bool,

}

#[derive(Clone, Default, Debug)]
pub struct CharacterControllerInput {
    pub move_dir: Vec2,
    pub shift: bool,
    pub jump: bool,
}

pub struct CharacterControllerState {
    position: Vec3,
    velocity: Vec3,
    grounded: bool,
}

#[derive(Default)]
pub struct CharacterControllerOutput {
    move_velocity: Vec3,
    jump: bool,
}

pub struct CharacterControllerContext {
    input: CharacterControllerInput,
    state: CharacterControllerState,
    output: CharacterControllerOutput,
}

pub struct CharacterController {
    behavior_tree: BehaviorTree<CharacterControllerContext>,
    input: CharacterControllerInput,
    pub yaw: f32,
    pub pitch: f32,
    pub jump: bool,
}

#[derive(Clone)]
pub struct CharacterControllerBundle {}

impl Bundle for CharacterControllerBundle {
    fn mount(&self, entity: &mut flax::EntityBuilder) {
        let input = PlayerInputConfiguration::new();

        let controller = CharacterController {
            behavior_tree: BehaviorTree::new(Selector::new(vec![
                Box::new(JumpAction {}),
                Box::new(SprintSelector {
                    walk: WalkAction {},
                    sprint: SprintAction {},
                }),
            ])),
            input: CharacterControllerInput::default(),
            yaw: 0.0,
            pitch: 0.0,
            jump: false,
        };

        entity
            .set(character_controller(), controller)
            .set_default(camera_look_data())
            .set_default(movement_input())
            .set_default(yaw_input())
            .set_default(pitch_input())
            .set_default(shift_input())
            .set_default(jump_input())
            .set_default(interact_input())
            .set(
                input_state(),
                InputState::new()
                    .with_action(movement_input(), input.movement_input_action)
                    .with_action(yaw_input(), input.yaw_input_action)
                    .with_action(pitch_input(), input.pitch_input_action)
                    .with_action(shift_input(), input.shift_input_action)
                    .with_action(jump_input(), input.jump_input)
                    .with_action(interact_input(), input.interact_input),
            );
    }
}

impl CharacterController {
    #[system]
    fn update_inputs_system(
        self: &mut CharacterController,
        movement_input: Vec2,
        yaw_input: f32,
        pitch_input: f32,
        shift_input: bool,
        jump_input: bool,
    ) {
        self.yaw += yaw_input;
        self.pitch += pitch_input;
        self.input.move_dir = movement_input.normalize_or_zero();
        self.input.shift = shift_input;
        self.input.jump = self.jump;
        self.jump = false;
    }

    #[system]
    fn update_movement_system(
        self: &mut CharacterController,
        position: Vec3,
        velocity: &mut Vec3,
        rotation: &mut glam::Quat,
        movement_direction: &mut Vec3,
    ) {
        let grounded = velocity.y.abs() < 0.01;
        let state = CharacterControllerState {
            position,
            velocity: *velocity,
            grounded,
        };

        let mut ctx = CharacterControllerContext {
            input: self.input.clone(),
            state,
            output: CharacterControllerOutput::default(),
        };

        self.behavior_tree.execute(&mut ctx);

        *rotation = glam::Quat::from_rotation_y(self.yaw);
        *movement_direction = *rotation * ctx.output.move_velocity;

        if ctx.output.jump {
            velocity.y = 5.0;
        }
    }

    #[system(args(camera_look_data=camera_look_data().as_mut()))]
    fn update_look_data_system(self: &CharacterController, camera_look_data: &mut CameraLookData) {
        camera_look_data.pitch = self.pitch;
    }
}

pub struct CharacterControllerPlugin;

impl Plugin for CharacterControllerPlugin {
    fn install(&self, ctx: &mut PluginContext) -> anyhow::Result<()> {
        ctx.schedules
            .per_tick_mut()
            .with_system(CharacterController::update_inputs_system())
            .with_system(CharacterController::update_movement_system())
            .with_system(CharacterController::update_look_data_system());

        Ok(())
    }
}

pub struct PlayerInputConfiguration {
    pub movement_input_action: Action<Vec2>,
    pub yaw_input_action: Action<f32>,
    pub pitch_input_action: Action<f32>,
    pub shift_input_action: Action<bool>,
    pub jump_input: Action<bool>,
    pub interact_input: Action<bool>,
}

impl PlayerInputConfiguration {
    pub fn new() -> Self {
        let movement_input_action = Action::new()
            .with_binding(
                KeyBinding::new(Key::Character("w".into()))
                    .analog()
                    .compose(Axis2D::Y),
            )
            .with_binding(
                KeyBinding::new(Key::Character("s".into()))
                    .analog()
                    .compose(Axis2D::Y)
                    .amplitude(-1.0),
            )
            .with_binding(
                KeyBinding::new(Key::Character("a".into()))
                    .analog()
                    .compose(Axis2D::X)
                    .amplitude(-1.0),
            )
            .with_binding(
                KeyBinding::new(Key::Character("d".into()))
                    .analog()
                    .compose(Axis2D::X),
            );

        let yaw_input_action = Action::new().with_binding(
            CursorMoveBinding::new()
                .decompose(Axis2D::X)
                .amplitude(-0.001),
        );

        let pitch_input_action = Action::new().with_binding(
            CursorMoveBinding::new()
                .decompose(Axis2D::Y)
                .amplitude(-0.001),
        );

        let shift_input_action =
            Action::new().with_binding(KeyBinding::new(Key::Named(NamedKey::Shift)));

        let jump_input =
            Action::new().with_binding(KeyBinding::new(Key::Named(NamedKey::Space)).rising_edge());

        let interact_input =
            Action::new().with_binding(KeyBinding::new(Key::Character("f".into())).rising_edge());

        Self {
            movement_input_action,
            yaw_input_action,
            pitch_input_action,
            shift_input_action,
            jump_input,
            interact_input,
        }
    }
}

impl Default for PlayerInputConfiguration {
    fn default() -> Self {
        Self::new()
    }
}

struct JumpAction {}

struct WalkAction {}

struct SprintAction {}

struct SprintSelector {
    walk: WalkAction,
    sprint: SprintAction,
}

const WALK_SPEED: f32 = 2.0;
const SPRINT_SPEED: f32 = 6.0;

impl BehaviorTreeNode<CharacterControllerContext> for WalkAction {
    fn execute(
        &mut self,
        ctx: &mut CharacterControllerContext,
    ) -> crate::behavior_tree::NodeStatus {
        if ctx.input.move_dir.length_squared() > 0.0 {
            ctx.output.move_velocity =
                vec3(ctx.input.move_dir.x, 0.0, -ctx.input.move_dir.y) * WALK_SPEED;
            crate::behavior_tree::NodeStatus::Success
        } else {
            ctx.output.move_velocity = Vec3::ZERO;
            crate::behavior_tree::NodeStatus::Failure
        }
    }
}

impl BehaviorTreeNode<CharacterControllerContext> for SprintAction {
    fn execute(
        &mut self,
        ctx: &mut CharacterControllerContext,
    ) -> crate::behavior_tree::NodeStatus {
        if ctx.input.move_dir.length_squared() > 0.0 {
            ctx.output.move_velocity =
                vec3(ctx.input.move_dir.x, 0.0, -ctx.input.move_dir.y) * SPRINT_SPEED;
            crate::behavior_tree::NodeStatus::Success
        } else {
            ctx.output.move_velocity = Vec3::ZERO;
            crate::behavior_tree::NodeStatus::Failure
        }
    }
}

impl BehaviorTreeNode<CharacterControllerContext> for JumpAction {
    fn execute(
        &mut self,
        ctx: &mut CharacterControllerContext,
    ) -> crate::behavior_tree::NodeStatus {
        if ctx.input.jump && ctx.state.grounded {
            ctx.output.jump = true;
            crate::behavior_tree::NodeStatus::Success
        } else {
            crate::behavior_tree::NodeStatus::Failure
        }
    }
}

impl BehaviorTreeNode<CharacterControllerContext> for SprintSelector {
    fn execute(
        &mut self,
        ctx: &mut CharacterControllerContext,
    ) -> crate::behavior_tree::NodeStatus {
        if ctx.input.shift {
            self.sprint.execute(ctx)
        } else {
            self.walk.execute(ctx)
        }
    }
}
