use flax::{
    component,
    fetch::{entity_refs, EntityRefs, Source},
    BoxedSystem, CommandBuffer, Component, Entity, Fetch, FetchExt, Mutable, Query, QueryBorrow,
    System, World,
};
use glam::{vec2, vec4, Mat4, Vec2, Vec3, Vec4Swizzles};
use ivy_assets::AssetCache;
use ivy_core::{
    components::{engine, main_camera, position, rotation, world_transform, TransformBundle},
    update_layer::{FixedTimeStep, Plugin},
    EntityBuilderExt,
};
use ivy_input::{
    components::input_state,
    types::{Key, MouseButton, NamedKey},
    Action, BindingExt, CursorPositionBinding, InputState, KeyBinding, MouseButtonBinding,
};
use ivy_physics::{
    components::{impulse_joint, physics_state},
    rapier3d::{
        self,
        math::Isometry,
        prelude::{FixedJointBuilder, QueryFilter, RigidBodyType},
    },
    state::PhysicsState,
    RigidBodyBundle,
};
use ivy_wgpu::components::projection_matrix;

pub struct PickingState {
    picked_object: Option<(Entity, Vec3, f32)>,
    manipulator: Entity,
}

impl PickingState {
    pub fn update(
        &mut self,
        world: &World,
        cmd: &mut CommandBuffer,
        physics_state: &PhysicsState,
        origin: Vec3,
        ray_dir: Vec3,
    ) -> anyhow::Result<()> {
        if self.picked_object.is_some() {
            self.move_manipulator(world, origin, ray_dir)
        } else {
            self.start_manipulating(world, cmd, physics_state, origin, ray_dir)?;
            self.move_manipulator(world, origin, ray_dir)
        }
    }

    pub fn move_manipulator(
        &mut self,
        world: &World,
        origin: Vec3,
        ray_dir: Vec3,
    ) -> anyhow::Result<()> {
        if let Some((_, _, distance)) = self.picked_object {
            let new_pos = ray_dir * distance + origin;

            let manipulator = world.entity(self.manipulator)?;

            manipulator.update_dedup(position(), new_pos);
        }

        Ok(())
    }

    pub fn start_manipulating(
        &mut self,
        world: &World,
        cmd: &mut CommandBuffer,
        physics_state: &PhysicsState,
        origin: Vec3,
        ray_dir: Vec3,
    ) -> anyhow::Result<()> {
        let ray = rapier3d::prelude::Ray::new(origin.into(), ray_dir.into());
        let result = physics_state.cast_ray(&ray, 1e3, true, QueryFilter::exclude_fixed());

        if let Some((id, _, intersection)) = result {
            let entity = world.entity(id)?;

            let point: Vec3 = ray.point_at(intersection.time_of_impact).into();

            let pos = entity.get_copy(position()).unwrap_or_default();
            let rotation = entity.get_copy(rotation()).unwrap_or_default();
            let anchor = point - pos;
            let distance = intersection.time_of_impact;

            self.stop_manipulating(cmd);

            let joint = FixedJointBuilder::new()
                .local_frame2(Isometry::new(
                    (rotation.inverse() * anchor).into(),
                    rotation.inverse().to_scaled_axis().into(),
                ))
                .build();

            cmd.set(self.manipulator, impulse_joint(id), joint.into());

            self.picked_object = Some((id, anchor, distance));
        }

        Ok(())
    }

    pub fn stop_manipulating(&mut self, cmd: &mut CommandBuffer) {
        if let Some((id, _, _)) = self.picked_object.take() {
            cmd.remove(self.manipulator, impulse_joint(id));
        }
    }
}

component! {
    pick_ray_action: f32,
    cursor_position_action: Vec2,
    picking_state: PickingState,
    ray_distance_modifier: f32,
}

pub struct RayPickingPlugin;

impl Plugin<FixedTimeStep> for RayPickingPlugin {
    fn install(
        &self,
        world: &mut World,
        _: &AssetCache,
        schedule: &mut flax::ScheduleBuilder,
        step: &FixedTimeStep,
    ) -> anyhow::Result<()> {
        let mut left_click_action = Action::new(pick_ray_action());
        left_click_action.add(MouseButtonBinding::new(MouseButton::Left));

        let mut cursor_position = Action::new(cursor_position_action());
        cursor_position.add(CursorPositionBinding::new(true));

        let mut ray_distance_action = Action::new(ray_distance_modifier());
        ray_distance_action.add(KeyBinding::new(Key::Named(NamedKey::ArrowUp)));
        ray_distance_action.add(KeyBinding::new(Key::Named(NamedKey::ArrowDown)).amplitude(-1.0));

        let manipulator = Entity::builder()
            .mount(TransformBundle::default())
            .mount(RigidBodyBundle::new(RigidBodyType::Dynamic).with_can_sleep(false))
            .spawn(world);

        Entity::builder()
            .set(
                input_state(),
                InputState::new()
                    .with_action(left_click_action)
                    .with_action(cursor_position)
                    .with_action(ray_distance_action),
            )
            .set_default(pick_ray_action())
            .set_default(cursor_position_action())
            .set_default(ray_distance_modifier())
            .set(
                picking_state(),
                PickingState {
                    picked_object: None,
                    manipulator,
                },
            )
            .spawn(world);

        schedule
            .with_system(pick_ray_system())
            .with_system(ray_distance_system(step.delta_time() as _));

        Ok(())
    }
}

#[derive(Fetch)]
pub struct CameraQuery {
    transform: Component<Mat4>,
    projection: Component<Mat4>,
}

impl CameraQuery {
    pub fn new() -> Self {
        Self {
            transform: world_transform(),
            projection: projection_matrix(),
        }
    }
}

impl Default for CameraQuery {
    fn default() -> Self {
        Self::new()
    }
}

type PickingQuery = (
    EntityRefs,
    Component<f32>,
    Component<Vec2>,
    Mutable<PickingState>,
);

type PickRaySystemQuery = (
    Source<Component<PhysicsState>, Entity>,
    Source<(Component<()>, CameraQuery), ()>,
    PickingQuery,
);

pub fn ray_distance_system(dt: f32) -> BoxedSystem {
    System::builder()
        .with_query(Query::new((
            picking_state().as_mut(),
            ray_distance_modifier(),
        )))
        .for_each(move |(state, change_distance)| {
            if let Some((_, _, distance)) = &mut state.picked_object {
                *distance = (*distance + change_distance * 5.0 * dt).max(2.0);
            }
        })
        .boxed()
}

pub fn pick_ray_system() -> BoxedSystem {
    System::builder()
        .with_cmd_mut()
        .with_query(Query::new((
            physics_state().source(engine()),
            (main_camera(), CameraQuery::new()).source(()),
            (
                entity_refs(),
                pick_ray_action(),
                cursor_position_action(),
                picking_state().as_mut(),
            ),
        )))
        .build(
            |cmd: &mut CommandBuffer, mut query: QueryBorrow<'_, PickRaySystemQuery>| {
                for (
                    physics_state,
                    (_, camera),
                    (entity, pick_ray_activation, cursor_pos, state),
                ) in query.iter()
                {
                    let world = entity.world();

                    if *pick_ray_activation < 1.0 {
                        state.stop_manipulating(cmd);
                        return Ok(());
                    }

                    let cursor_pos = vec2(cursor_pos.x * 2.0 - 1.0, -(cursor_pos.y * 2.0 - 1.0));

                    let ray_eye =
                        camera.projection.inverse() * vec4(cursor_pos.x, cursor_pos.y, 1.0, 1.0);
                    let ray_eye = vec4(ray_eye.x, ray_eye.y, -1.0, 0.0);

                    let world_ray = (*camera.transform * ray_eye).xyz().normalize();

                    let origin = camera.transform.transform_point3(Vec3::ZERO);

                    state.update(world, cmd, physics_state, origin, world_ray)?;
                }

                anyhow::Ok(())
            },
        )
        .boxed()
}