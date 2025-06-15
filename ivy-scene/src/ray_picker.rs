use std::time::Duration;

use flax::{
    component, components::child_of, entity::EntityKind, system, CommandBuffer, ComponentMut, Entity, EntityRef, FetchExt, Query, QueryBorrow, World
};
use glam::{Vec2, Vec3};
use ivy_assets::{stored::DynamicStore, AssetCache};
use ivy_core::{
    components::{delta_time, engine, gizmos, main_camera, position, rotation, TransformBundle}, gizmos::{Gizmos, SphereGizmo}, math::Axis2D, update_layer::{Plugin, ScheduleSetBuilder}, Bundle, Color, ColorExt, EntityBuilderExt
};
use ivy_input::{
    components::{cursor_position, input_state},
    types::{Key, MouseButton, NamedKey},
    Action, BindingExt, InputState, KeyBinding, MouseButtonBinding, ScrollBinding,
};
use ivy_physics::{
    components::{impulse_joint, physics_state},
    rapier3d::{
        math::Isometry,
        prelude::{FixedJointBuilder, QueryFilter, RigidBodyType},
    },
    shapes::Ray,
    state::PhysicsState,
    RigidBodyBundle,
};

use crate::camera::{screen_to_world_ray, CameraQuery, CameraQueryItem};

pub struct RayPickingTool {
    picked_object: Option<(Entity, Vec3, f32)>,
    manipulator: Option<Entity>,
}

impl RayPickingTool {
    pub fn move_manipulator(
        &mut self,
        manipulator: EntityRef,
        ray:Ray,
    ) -> anyhow::Result<()> {
        if let Some((_, _, distance)) = self.picked_object {
            let new_pos = ray.at(distance);


            manipulator.update_dedup(position(), new_pos);
        }

        Ok(())
    }

    pub fn start_manipulating(
        &mut self,
        manipulator: EntityRef,
        world: &World,
        cmd: &mut CommandBuffer,
        physics_state: &PhysicsState,
        ray:Ray,
    ) -> anyhow::Result<()> {
        let result = physics_state.cast_ray(ray, 1e3, true, QueryFilter::exclude_fixed());

        if let Some(hit) = result {
            let entity = world.entity(hit.collider_id)?;

            let point: Vec3 = ray.at(hit.intersection.time_of_impact).into();

            let pos = entity.get_copy(position()).unwrap_or_default();
            let rotation = entity.get_copy(rotation()).unwrap_or_default();
            let anchor = point - pos;
            let distance = hit.intersection.time_of_impact;

            self.stop_manipulating(manipulator, cmd);

            let joint = FixedJointBuilder::new()
                .local_frame2(Isometry::new(
                    (rotation.inverse() * anchor).into(),
                    rotation.inverse().to_scaled_axis().into(),
                ))
                .build();

            cmd.set(
                manipulator.id(),
                impulse_joint(hit.collider_id),
                joint.into(),
            );

            self.picked_object = Some((hit.collider_id, anchor, distance));
        }

        Ok(())
    }

    pub fn stop_manipulating(&mut self, manipulator: EntityRef, cmd: &mut CommandBuffer) {
        if let Some((id, _, _)) = self.picked_object.take() {
            cmd.remove(manipulator.id(), impulse_joint(id));
        }
    }


    #[system(args(dt=delta_time().source(engine())))]
    fn ray_distance_system(
        self: &mut RayPickingTool,
        dt: &Duration,
        ray_distance_modifier: &f32,
    ) {
        if let Some((_, _, distance)) = &mut self.picked_object {
            *distance = (*distance + ray_distance_modifier * 5.0 * dt.as_secs_f32()).max(2.0);
        }

    }

    #[system(args(camera=(CameraQuery::new(), main_camera()).source(()),
        physics_state=physics_state().source(engine()), 
        cursor_position=cursor_position().source(engine())), with_world, with_cmd_mut)]
    pub fn update_system(
        self: &mut RayPickingTool,
        id: Entity,
        pick_ray_action: bool,
        physics_state: &PhysicsState,
        cursor_position: &Vec2,
        camera: (CameraQueryItem, &()),
        world: &World,
        cmd: &mut CommandBuffer,
    ) -> anyhow::Result<()> {
        let manipulator = match self.manipulator {
            Some(v) => world.entity(v)?,
            None => {
                let manipulator = world.reserve_one(EntityKind::empty());

                let mut manipulator_entity = Entity::builder();
                manipulator_entity
                    .mount(TransformBundle::default())
                    .mount(RigidBodyBundle::new(RigidBodyType::Dynamic).with_can_sleep(false))
                    .set_default(child_of(id));

                cmd.append_to(manipulator, manipulator_entity);

                self.manipulator = Some(manipulator);
                return Ok(());

            }
        };
        
        let ray = screen_to_world_ray(*cursor_position, camera.0);

        if pick_ray_action && self.picked_object.is_none() {
            self.start_manipulating(manipulator, world, cmd, physics_state, ray)?;
            self.move_manipulator(manipulator, ray)?;
        } else if pick_ray_action {
            self.move_manipulator(manipulator, ray)?;
        } else if !pick_ray_action {
            self.stop_manipulating( manipulator, cmd);
        }

        Ok(())

    }

    #[system(with_world, with_query(Query::new(gizmos().as_mut())))]
    pub fn draw_gizmos_system(
        self: &mut RayPickingTool,
        world: &World,
        gizmos: &mut QueryBorrow<ComponentMut<Gizmos>>,
    ) {
        let gizmos = gizmos.first().unwrap();
        let mut gizmos = gizmos.begin_section("PickingState::gizmos");

        if self.picked_object.is_some() {
            let manipulator = world.entity(self.manipulator.unwrap()).unwrap();

            gizmos.draw(SphereGizmo::new(
                manipulator.get_copy(position()).unwrap(),
                0.1,
                Color::red(),
            ));
        }
    }
}

component! {
    pick_ray_action: bool,
    ray_picking_tool: RayPickingTool,
    ray_distance_modifier: f32,
}

pub struct RayPickerBundle {}

impl RayPickerBundle {
    pub fn new() -> Self {
        Self {  }
    }
}

impl Bundle for RayPickerBundle {
    fn mount(&self, entity: &mut flax::EntityBuilder) {
        let mut left_click_action = Action::new();
        left_click_action.add(MouseButtonBinding::new(MouseButton::Left));

        let mut ray_distance_action = Action::new();
        ray_distance_action.add(KeyBinding::new(Key::Named(NamedKey::ArrowUp)).analog()).add(ScrollBinding::new().decompose(Axis2D::Y).amplitude(2.0));
        ray_distance_action.add(
            KeyBinding::new(Key::Named(NamedKey::ArrowDown))
                .analog()
                .amplitude(-1.0),
        );

        entity
            .set(
                input_state(),
                InputState::new()
                    .with_action(pick_ray_action(), left_click_action)
                    .with_action(ray_distance_modifier(), ray_distance_action),
            )
            .set_default(pick_ray_action())
            .set_default(ray_distance_modifier())
            .set(
                ray_picking_tool(),
                RayPickingTool {
                    picked_object: None,
                    manipulator: None,
                },
            );
    }
}

pub struct RayPickingPlugin;

impl Plugin for RayPickingPlugin {
    fn install(
        &self,
        _: &mut World,
        _: &AssetCache,
        _: &mut DynamicStore,
        schedules: &mut ScheduleSetBuilder,
    ) -> anyhow::Result<()> {


        schedules
            .per_tick_mut()
            .with_system(RayPickingTool::update_system())
            .with_system(RayPickingTool::ray_distance_system())
            .with_system(RayPickingTool::draw_gizmos_system());

        Ok(())
    }
}

