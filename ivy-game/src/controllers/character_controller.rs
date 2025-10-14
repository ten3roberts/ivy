use flax::{component, system};
use glam::{vec3, Quat, Vec2, Vec3};
use ivy_core::{components::rotation, math::Axis2D, update_layer::Plugin, Bundle};
use ivy_input::{
    components::input_state,
    types::{Key, NamedKey},
    Action, CursorMoveBinding, InputState, KeyBinding,
};
use ivy_physics::{
    rapier3d::prelude::{CoefficientCombineRule, ColliderBuilder, SharedShape},
    ColliderBundle,
};

use crate::{
    behavior_tree::{BehaviorTree, BehaviorTreeNode},
    controllers::character_controller,
};

component! {
    character_controller: CharacterController,
    movement_input: Vec2,
    yaw_input: f32,
    jump_input: bool,
    interact_input: bool,

}

pub struct CharacterControllerInput {
    move_dir: Vec2,
}

pub struct CharacterControllerState {
    position: Vec3,
    velocity: Vec3,
    grounded: bool,
}

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
}

#[derive(Clone)]
pub struct CharacterControllerBundle {}

impl Bundle for CharacterControllerBundle {
    fn mount(&self, entity: &mut flax::EntityBuilder) {
        let character_height = 1.75;

        let input = PlayerInputConfiguration::new();

        entity
            .set(character_controller(), self.controller)
            .set_default(movement_input())
            .set_default(yaw_input())
            .set_default(jump_input())
            .set_default(interact_input())
            .set(
                input_state(),
                InputState::new()
                    .with_action(movement_input(), input.movement_input_action)
                    .with_action(yaw_input(), input.yaw_input_action)
                    .with_action(jump_input(), input.jump_input)
                    .with_action(interact_input(), input.interact_input),
            )
            .mount(self.rb_bundle)
            .mount(ColliderBundle::from_builder(
                ColliderBuilder::new(SharedShape::capsule_y(
                    (character_height) / 2.0 - 0.2 * 2.0,
                    0.2,
                ))
                .friction_combine_rule(CoefficientCombineRule::Min)
                .restitution_combine_rule(CoefficientCombineRule::Min)
                .restitution(0.0)
                .friction(0.0),
            ));
    }
}

impl CharacterController {
    #[system]
    fn update_inputs_system(
        self: &mut CharacterController,
        movement_input: Vec2,
        rotation: Quat,
        yaw_input: f32,
        jump_input: bool,
    ) {
        self.yaw += yaw_input;
        self.jump |= jump_input;
        self.input.move_dir = movement_input.normalize();
    }
}

pub struct CharacterControllerPlugin;

impl Plugin for CharacterControllerPlugin {
    fn install(
        &self,
        world: &mut flax::World,
        assets: &ivy_assets::AssetCache,
        store: &mut ivy_assets::stored::DynamicStore,
        schedules: &mut ivy_core::update_layer::ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        schedules
            .per_tick_mut()
            .with_system(CharacterController::update_inputs_system())
    }
}

pub struct PlayerInputConfiguration {
    pub movement_input_action: Action<Vec2>,
    pub yaw_input_action: Action<f32>,
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
                .amplitude(0.001),
        );

        let jump_input =
            Action::new().with_binding(KeyBinding::new(Key::Named(NamedKey::Space)).rising_edge());

        // let interact_input =
        //     Action::new().with_binding(KeyBinding::new(Key::Character("f".into())).rising_edge());
        //
        // let inventory_action =
        //     Action::new().with_binding(KeyBinding::new(Key::Character("e".into())).falling_edge());

        Self {
            movement_input_action,
            yaw_input_action,
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

struct WalkAction {}

const WALK_SPEED: f32 = 3.0;

impl BehaviorTreeNode<CharacterControllerContext> for WalkAction {
    fn execute(
        &mut self,
        ctx: &mut CharacterControllerContext,
    ) -> crate::behavior_tree::NodeStatus {
        if ctx.input.move_dir.length_squared() > 0.0 {
            ctx.output.move_velocity =
                vec3(ctx.input.move_dir.x, 0.0, ctx.input.move_dir.y) * WALK_SPEED;
            crate::behavior_tree::NodeStatus::Success
        } else {
            ctx.output.move_velocity = Vec3::ZERO;
            crate::behavior_tree::NodeStatus::Failure
        }
    }
}
