use flax::{
    component,
    fetch::{entity_refs, EntityRefs, Source},
    system, BoxedSystem, CommandBuffer, Component, ComponentMut, Entity, FetchExt, Query,
    QueryBorrow, System, World,
};
use glam::{Vec2, Vec3};
use ivy_assets::AssetCache;
use ivy_core::{
    components::{engine, gizmos, main_camera, position, rotation, TransformBundle},
    gizmos::{Gizmos, Sphere},
    update_layer::{Plugin, ScheduleSetBuilder},
    Color, ColorExt, EntityBuilderExt,
};
use ivy_input::{
    components::{cursor_position, input_state},
    types::{Key, MouseButton, NamedKey},
    Action, BindingExt, InputState, KeyBinding, MouseButtonBinding,
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

use crate::camera::{screen_to_world_ray, CameraQuery};

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

        if let Some(hit) = result {
            let entity = world.entity(hit.collider_id)?;

            let point: Vec3 = ray.point_at(hit.intersection.time_of_impact).into();

            let pos = entity.get_copy(position()).unwrap_or_default();
            let rotation = entity.get_copy(rotation()).unwrap_or_default();
            let anchor = point - pos;
            let distance = hit.intersection.time_of_impact;

            self.stop_manipulating(cmd);

            let joint = FixedJointBuilder::new()
                .local_frame2(Isometry::new(
                    (rotation.inverse() * anchor).into(),
                    rotation.inverse().to_scaled_axis().into(),
                ))
                .build();

            cmd.set(
                self.manipulator,
                impulse_joint(hit.collider_id),
                joint.into(),
            );

            self.picked_object = Some((hit.collider_id, anchor, distance));
        }

        Ok(())
    }

    pub fn stop_manipulating(&mut self, cmd: &mut CommandBuffer) {
        if let Some((id, _, _)) = self.picked_object.take() {
            cmd.remove(self.manipulator, impulse_joint(id));
        }
    }

    #[system(with_world, with_query(Query::new(gizmos().as_mut())))]
    pub fn draw_gizmos_system(
        self: &mut PickingState,
        world: &World,
        gizmos: &mut QueryBorrow<ComponentMut<Gizmos>>,
    ) {
        let gizmos = gizmos.first().unwrap();
        let mut gizmos = gizmos.begin_section("PickingState::gizmos");

        if self.picked_object.is_some() {
            let manipulator = world.entity(self.manipulator).unwrap();

            gizmos.draw(Sphere::new(
                manipulator.get_copy(position()).unwrap(),
                0.1,
                Color::red(),
            ));
        }
    }
}

component! {
    pick_ray_action: bool,
    picking_state: PickingState,
    ray_distance_modifier: f32,
}

pub struct RayPickingPlugin;

impl Plugin for RayPickingPlugin {
    fn install(
        &self,
        world: &mut World,
        _: &AssetCache,
        schedules: &mut ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        let mut left_click_action = Action::new();
        left_click_action.add(MouseButtonBinding::new(MouseButton::Left));

        let mut ray_distance_action = Action::new();
        ray_distance_action.add(KeyBinding::new(Key::Named(NamedKey::ArrowUp)).analog());
        ray_distance_action.add(
            KeyBinding::new(Key::Named(NamedKey::ArrowDown))
                .analog()
                .amplitude(-1.0),
        );

        let manipulator = Entity::builder()
            .mount(TransformBundle::default())
            .mount(RigidBodyBundle::new(RigidBodyType::Dynamic).with_can_sleep(false))
            .spawn(world);

        Entity::builder()
            .set(
                input_state(),
                InputState::new()
                    .with_action(pick_ray_action(), left_click_action)
                    .with_action(ray_distance_modifier(), ray_distance_action),
            )
            .set_default(pick_ray_action())
            .set_default(ray_distance_modifier())
            .set(
                picking_state(),
                PickingState {
                    picked_object: None,
                    manipulator,
                },
            )
            .spawn(world);

        let dt = schedules.fixed_mut().time_step().delta_time() as _;
        schedules
            .fixed_mut()
            .with_system(pick_ray_system())
            .with_system(ray_distance_system(dt))
            .with_system(PickingState::draw_gizmos_system());

        Ok(())
    }
}

type PickingQuery = (EntityRefs, Component<bool>, ComponentMut<PickingState>);

type PickRaySystemQuery = (
    Source<(Component<PhysicsState>, Component<Vec2>), Entity>,
    Source<(Component<()>, CameraQuery), ()>,
    PickingQuery,
);

pub fn ray_distance_system(dt: f32) -> BoxedSystem {
    System::builder()
        .with_query(Query::new((
            picking_state().as_mut(),
            ray_distance_modifier(),
        )))
        .for_each(move |(state, &change_distance)| {
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
            (physics_state(), cursor_position()).source(engine()),
            (main_camera(), CameraQuery::new()).source(()),
            (entity_refs(), pick_ray_action(), picking_state().as_mut()),
        )))
        .build(
            |cmd: &mut CommandBuffer, mut query: QueryBorrow<'_, PickRaySystemQuery>| {
                for (
                    (physics_state, &cursor_pos),
                    (_, camera),
                    (entity, pick_ray_activation, state),
                ) in query.iter()
                {
                    let world = entity.world();

                    if !pick_ray_activation {
                        state.stop_manipulating(cmd);
                        return Ok(());
                    }

                    let ray = screen_to_world_ray(cursor_pos, camera);
                    state.update(world, cmd, physics_state, ray.origin.into(), ray.dir.into())?;
                }

                anyhow::Ok(())
            },
        )
        .boxed()
}
