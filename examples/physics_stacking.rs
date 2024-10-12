use flax::{
    component,
    fetch::{entity_refs, EntityRefs, Source},
    BoxedSystem, CommandBuffer, Component, Entity, Fetch, FetchExt, Mutable, Query, QueryBorrow,
    System, World,
};
use glam::{vec2, vec3, vec4, EulerRot, Mat4, Quat, Vec2, Vec3, Vec4Swizzles};
use ivy_assets::AssetCache;
use ivy_collision::components::collider;
use ivy_core::{
    app::InitEvent,
    layer::events::EventRegisterContext,
    palette::{Srgb, Srgba},
    profiling::ProfilingLayer,
    update_layer::{FixedTimeStep, PerTick, Plugin, ScheduledLayer},
    App, Color, ColorExt, EngineLayer, EntityBuilderExt, Layer,
};
use ivy_engine::{
    engine, is_static, main_camera, position, rotation, scale, world_transform, Collider,
    RigidBodyBundle, TransformBundle,
};
use ivy_game::free_camera::{setup_camera, CameraInputPlugin};
use ivy_graphics::texture::TextureDesc;
use ivy_input::{
    components::input_state, layer::InputLayer, Action, CursorPositionBinding, InputState,
    MouseButtonBinding,
};
use ivy_physics::{
    components::{collider_shape, impulse_joint, physics_state, rigid_body_type},
    state::PhysicsState,
    ColliderBundle, PhysicsPlugin,
};
use ivy_postprocessing::preconfigured::{SurfacePbrPipeline, SurfacePbrPipelineDesc};
use ivy_wgpu::{
    components::*,
    driver::WinitDriver,
    events::ResizedEvent,
    layer::GraphicsLayer,
    light::{LightData, LightKind},
    material_desc::{MaterialData, MaterialDesc},
    mesh_desc::MeshDesc,
    primitives::{CapsulePrimitive, CubePrimitive, UvSpherePrimitive},
    renderer::{EnvironmentData, RenderObjectBundle},
    shaders::{PbrShaderDesc, ShadowShaderDesc},
};
use rapier3d::{
    math::Isometry,
    prelude::{FixedJointBuilder, QueryFilter, RigidBodyType, SharedShape},
};
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
use winit::{dpi::LogicalSize, window::WindowAttributes};

const ENABLE_SKYBOX: bool = true;

pub fn main() -> anyhow::Result<()> {
    color_backtrace::install();

    registry()
        .with(EnvFilter::from_default_env())
        .with(
            HierarchicalLayer::default()
                .with_indent_lines(true)
                .with_span_retrace(true),
        )
        .init();

    if let Err(err) = App::builder()
        .with_driver(WinitDriver::new(
            WindowAttributes::default()
                .with_inner_size(LogicalSize::new(1920, 1080))
                .with_title("Ivy Physics"),
        ))
        .with_layer(EngineLayer::new())
        .with_layer(ProfilingLayer::new())
        .with_layer(GraphicsLayer::new(|world, assets, gpu, surface| {
            Ok(SurfacePbrPipeline::new(
                world,
                assets,
                gpu,
                surface,
                SurfacePbrPipelineDesc {
                    // hdri: None,
                    hdri: Some(Box::new(
                        "hdris/kloofendal_48d_partly_cloudy_puresky_2k.hdr",
                    )),
                },
            ))
        }))
        .with_layer(InputLayer::new())
        .with_layer(LogicLayer)
        .with_layer(
            ScheduledLayer::new(PerTick)
                .with_plugin(CameraInputPlugin)
                .with_plugin(RayPickingPlugin),
        )
        .with_layer(
            ScheduledLayer::new(FixedTimeStep::new(0.02)).with_plugin(
                PhysicsPlugin::new()
                    .with_gizmos(ivy_physics::GizmoSettings {
                        bvh_tree: false,
                        island_graph: true,
                        rigidbody: true,
                        contacts: true,
                    })
                    .with_gravity(-Vec3::Y * 9.81),
            ),
        )
        .run()
    {
        tracing::error!("{err:?}");
        Err(err)
    } else {
        Ok(())
    }
}

fn setup_objects(world: &mut World, assets: AssetCache) -> anyhow::Result<()> {
    let white_material = MaterialDesc::Content(
        MaterialData::new()
            .with_roughness_factor(1.0)
            .with_metallic_factor(0.0)
            .with_albedo(TextureDesc::srgba(Srgba::new(1.0, 1.0, 1.0, 1.0))),
    );

    let red_material = MaterialDesc::Content(
        MaterialData::new()
            .with_roughness_factor(1.0)
            .with_metallic_factor(0.0)
            .with_albedo(TextureDesc::srgba(Color::from_hsla(1.0, 0.7, 0.7, 1.0))),
    );

    let shader = assets.load(&PbrShaderDesc);
    let shadow = assets.load(&ShadowShaderDesc);

    const RESTITUTION: f32 = 0.1;
    const FRICTION: f32 = 0.8;

    let body = || {
        let mut builder = Entity::builder();
        builder
            .mount(TransformBundle::default())
            .mount(RigidBodyBundle::new(RigidBodyType::Dynamic))
            .mount(
                ColliderBundle::new(SharedShape::cuboid(1.0, 1.0, 1.0))
                    .with_restitution(RESTITUTION)
                    .with_friction(FRICTION),
            )
            .mount(RenderObjectBundle::new(
                MeshDesc::Content(assets.load(&CubePrimitive)),
                white_material.clone(),
                shader.clone(),
            ))
            .set(shadow_pass(), shadow.clone());

        builder
    };

    let cube = |pos: Vec3, size: Vec3| {
        let mut builder = body();
        builder.set(ivy_core::components::position(), pos).set(
            collider_shape(),
            SharedShape::cuboid(size.x, size.y, size.z),
        );
        builder
    };

    let sphere = |pos: Vec3, size: f32| {
        let mut builder = body();
        builder
            .set(ivy_core::components::position(), pos)
            .set(
                mesh(),
                MeshDesc::Content(assets.load(&UvSpherePrimitive::default())),
            )
            .set(collider_shape(), SharedShape::ball(size))
            .set(collider(), Collider::sphere(1.0));
        builder
    };

    let capsule = |pos: Vec3| {
        let mut builder = body();
        builder
            .set(ivy_core::components::position(), pos)
            .set(
                mesh(),
                MeshDesc::Content(assets.load(&CapsulePrimitive::default())),
            )
            .set(collider_shape(), SharedShape::capsule_y(1.0, 1.0));
        builder
    };

    cube(Vec3::ZERO, vec3(100.0, 1.0, 100.0))
        .set(rigid_body_type(), RigidBodyType::Fixed)
        .set(scale(), vec3(100.0, 1.0, 100.0))
        .set(is_static(), ())
        .spawn(world);

    let drop_height = 10.0;

    cube(vec3(0.0, drop_height, 0.0), Vec3::ONE)
        .set(rotation(), Quat::from_scaled_axis(vec3(1.0, 1.0, 0.0)))
        .set(material(), red_material.clone())
        .spawn(world);

    cube(vec3(5.0, drop_height, 0.0), Vec3::ONE)
        .set(rotation(), Quat::from_scaled_axis(vec3(1.0, 0.0, 0.0)))
        .set(material(), red_material.clone())
        .spawn(world);

    sphere(vec3(10.0, drop_height, 0.0), 1.0)
        .set(rotation(), Quat::from_scaled_axis(vec3(0.0, 0.0, 0.0)))
        .set(material(), red_material.clone())
        .spawn(world);

    capsule(vec3(-5.0, drop_height, 0.0))
        .set(rotation(), Quat::from_scaled_axis(vec3(0.1, 0.0, 0.0)))
        .set(material(), red_material.clone())
        .spawn(world);

    capsule(vec3(-10.0, drop_height, 0.0))
        .set(rotation(), Quat::from_scaled_axis(vec3(0.0, 0.0, 1.0)))
        .set(material(), red_material.clone())
        .spawn(world);

    for i in 0..4 {
        cube(
            vec3(0.0 + i as f32 * 0.0, 2.0 + i as f32 * 2.0, -8.0),
            Vec3::ONE,
        )
        .set(rotation(), Quat::from_scaled_axis(vec3(0.0, 0.0, 0.0)))
        .set(material(), red_material.clone())
        .spawn(world);
    }

    Entity::builder()
        .mount(TransformBundle::default().with_rotation(Quat::from_euler(
            EulerRot::YXZ,
            -2.0,
            -1.0,
            0.0,
        )))
        .set(light_data(), LightData::new(Srgb::new(1.0, 1.0, 1.0), 1.0))
        .set(light_kind(), LightKind::Directional)
        .set_default(cast_shadow())
        .spawn(world);

    Ok(())
}

struct LogicLayer;

impl Layer for LogicLayer {
    fn register(
        &mut self,
        world: &mut World,
        _: &AssetCache,
        mut events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()> {
        events.subscribe(|_, world, assets, InitEvent| {
            setup_objects(world, assets.clone())?;

            Ok(())
        });

        events.subscribe(|_, world, _, resized: &ResizedEvent| {
            if let Some(main_camera) = Query::new(projection_matrix().as_mut())
                .with(main_camera())
                .borrow(world)
                .first()
            {
                let aspect =
                    resized.physical_size.width as f32 / resized.physical_size.height as f32;
                *main_camera = Mat4::perspective_rh(1.0, aspect, 0.01, 1000.0);
            }

            Ok(())
        });

        setup_camera()
            .mount(TransformBundle::new(
                vec3(0.0, 20.0, 20.0),
                Quat::IDENTITY,
                Vec3::ONE,
            ))
            .set(
                environment_data(),
                EnvironmentData::new(
                    Srgb::new(0.2, 0.2, 0.3),
                    0.001,
                    if ENABLE_SKYBOX { 0.0 } else { 1.0 },
                ),
            )
            .spawn(world);

        Ok(())
    }
}

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
            self.start_manipulating(world, cmd, physics_state, origin, ray_dir)
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
}

pub struct RayPickingPlugin;

impl Plugin<PerTick> for RayPickingPlugin {
    fn install(
        &self,
        world: &mut World,
        _: &AssetCache,
        schedule: &mut flax::ScheduleBuilder,
        _: &PerTick,
    ) -> anyhow::Result<()> {
        let mut left_click_action = Action::new(pick_ray_action());
        left_click_action.add(MouseButtonBinding::new(winit::event::MouseButton::Left));

        let mut cursor_position = Action::new(cursor_position_action());
        cursor_position.add(CursorPositionBinding::new(true));

        let manipulator = Entity::builder()
            .mount(TransformBundle::default())
            .mount(RigidBodyBundle::new(RigidBodyType::Dynamic).with_can_sleep(false))
            .spawn(world);

        Entity::builder()
            .set(
                input_state(),
                InputState::new()
                    .with_action(left_click_action)
                    .with_action(cursor_position),
            )
            .set_default(pick_ray_action())
            .set_default(cursor_position_action())
            .set(
                picking_state(),
                PickingState {
                    picked_object: None,
                    manipulator,
                },
            )
            .spawn(world);

        schedule.with_system(pick_ray_system());

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
            |cmd: &mut CommandBuffer,
             mut query: QueryBorrow<
                '_,
                (
                    Source<Component<PhysicsState>, Entity>,
                    Source<(Component<()>, CameraQuery), ()>,
                    (
                        EntityRefs,
                        Component<f32>,
                        Component<Vec2>,
                        Mutable<PickingState>,
                    ),
                ),
            >| {
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
