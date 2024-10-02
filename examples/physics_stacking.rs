use flax::{
    component, components::child_of, fetch::entity_refs, BoxedSystem, Entity, FetchExt, Query,
    System, World,
};
use glam::{vec2, vec3, vec4, EulerRot, Mat4, Quat, Vec2, Vec3, Vec4Swizzles};
use ivy_assets::AssetCache;
use ivy_collision::{components::collider, Ray, RayCaster};
use ivy_core::{
    app::InitEvent,
    gizmos::{Line, Sphere, DEFAULT_RADIUS, DEFAULT_THICKNESS},
    layer::events::EventRegisterContext,
    palette::{Srgb, Srgba},
    profiling::ProfilingLayer,
    update_layer::{FixedTimeStep, PerTick, Plugin, ScheduledLayer},
    App, Color, ColorExt, EngineLayer, EntityBuilderExt, Layer,
};
use ivy_engine::{
    engine, gizmos, gravity_influence, is_static, main_camera, restitution, rotation, scale,
    world_transform, Collider, RigidBodyBundle, TransformBundle,
};
use ivy_game::free_camera::{setup_camera, CameraInputPlugin};
use ivy_graphics::texture::TextureDesc;
use ivy_input::{
    components::input_state, layer::InputLayer, Action, CursorMoveBinding, InputState,
    MouseButtonBinding,
};
use ivy_physics::{components::physics_state, PhysicsPlugin};
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
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
use wgpu::naga::RayQueryFunction;
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

    const RESTITUTION: f32 = 0.0;
    const FRICTION: f32 = 0.5;
    const MASS: f32 = 100.0;
    const INERTIA_TENSOR: f32 = 200.0;

    let body = || {
        let mut builder = Entity::builder();
        builder
            .mount(TransformBundle::default())
            .mount(
                RigidBodyBundle::default()
                    .with_mass(MASS)
                    .with_angular_mass(INERTIA_TENSOR)
                    .with_restitution(RESTITUTION)
                    .with_friction(FRICTION),
            )
            .set(
                collider(),
                Collider::cube_from_center(Vec3::ZERO, Vec3::ONE),
            )
            .mount(RenderObjectBundle::new(
                MeshDesc::Content(assets.load(&CubePrimitive)),
                white_material.clone(),
                shader.clone(),
            ))
            .set(shadow_pass(), shadow.clone());

        builder
    };

    let cube = |pos: Vec3| {
        let mut builder = body();
        builder.set(ivy_core::components::position(), pos);
        builder
    };

    let sphere = |pos: Vec3| {
        let mut builder = body();
        builder
            .set(ivy_core::components::position(), pos)
            .set(
                mesh(),
                MeshDesc::Content(assets.load(&UvSpherePrimitive::default())),
            )
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
            .set(collider(), Collider::capsule(1.0, 1.0));
        builder
    };

    cube(Vec3::ZERO)
        .set(scale(), vec3(50.0, 1.0, 50.0))
        .set(is_static(), ())
        .spawn(world);

    let drop_height = 10.0;

    cube(vec3(0.0, drop_height, 0.0))
        .set(rotation(), Quat::from_scaled_axis(vec3(1.0, 1.0, 0.0)))
        .set(gravity_influence(), 1.0)
        .set(restitution(), 1.0)
        .set(material(), red_material.clone())
        .spawn(world);

    cube(vec3(5.0, drop_height, 0.0))
        .set(rotation(), Quat::from_scaled_axis(vec3(1.0, 0.0, 0.0)))
        .set(gravity_influence(), 1.0)
        .set(restitution(), 1.0)
        .set(material(), red_material.clone())
        .spawn(world);

    sphere(vec3(10.0, drop_height, 0.0))
        .set(rotation(), Quat::from_scaled_axis(vec3(0.0, 0.0, 0.0)))
        .set(gravity_influence(), 1.0)
        .set(restitution(), 1.0)
        .set(material(), red_material.clone())
        .spawn(world);

    capsule(vec3(-5.0, drop_height, 0.0))
        .set(rotation(), Quat::from_scaled_axis(vec3(0.1, 0.0, 0.0)))
        .set(gravity_influence(), 1.0)
        .set(restitution(), 1.0)
        .set(material(), red_material.clone())
        .spawn(world);

    capsule(vec3(-10.0, drop_height, 0.0))
        .set(rotation(), Quat::from_scaled_axis(vec3(0.0, 0.0, 1.0)))
        .set(gravity_influence(), 1.0)
        .set(restitution(), 1.0)
        .set(material(), red_material.clone())
        .spawn(world);

    for i in 0..3 {
        cube(vec3(0.0, 2.0 + i as f32 * 2.0, -8.0))
            .set(rotation(), Quat::from_scaled_axis(vec3(0.0, 0.0, 0.0)))
            .set(gravity_influence(), 1.0)
            .set(restitution(), 0.0)
            .set(material(), red_material.clone())
            .spawn(world);
    }

    Entity::builder()
        .mount(TransformBundle::default().with_rotation(Quat::from_euler(
            EulerRot::YXZ,
            -2.0,
            1.0,
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

component! {
    pick_ray_action: f32,
    mouse_position: Vec2,
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

        let input_listener = Entity::builder()
            .set(
                input_state(),
                InputState::new().with_action(left_click_action),
            )
            .set_default(pick_ray_action())
            .spawn(world);

        schedule.with_system(pick_ray_system(input_listener));

        Ok(())
    }
}

pub fn pick_ray_system(input_listener: Entity) -> BoxedSystem {
    System::builder()
        .with_query(Query::new((
            (physics_state(), gizmos()).source(engine()),
            (main_window(), window_size(), window_cursor_position()).source(()),
            pick_ray_action().source(input_listener),
            (
                entity_refs(),
                main_camera(),
                world_transform(),
                projection_matrix(),
            ),
        )))
        .try_for_each(
            |(
                (physics_state, gizmos),
                (_, window_size, cursor_pos),
                pick_ray_activation,
                (entity, _, camera_transform, camera_projection),
            )| {
                let world = entity.world();
                let mut gizmos = gizmos.begin_section("pick_ray_system");

                if *pick_ray_activation < 1.0 {
                    return Ok(());
                }

                let _span = tracing::info_span!("pick").entered();

                let mouse_position =
                    vec2(cursor_pos.x, cursor_pos.y) / vec2(window_size.width, window_size.height);

                let mouse_position = vec2(
                    mouse_position.x * 2.0 - 1.0,
                    -(mouse_position.y * 2.0 - 1.0),
                );

                let ray_eye = camera_projection.inverse()
                    * vec4(mouse_position.x, mouse_position.y, 1.0, 1.0);
                let ray_eye = vec4(ray_eye.x, ray_eye.y, -1.0, 0.0);

                let world_ray = (*camera_transform * ray_eye).xyz().normalize();

                let origin = camera_transform.transform_point3(Vec3::ZERO);

                // gizmos.draw(Line::new(
                //     origin - camera_transform.transform_vector3(Vec3::Y) * 0.01,
                //     world_ray,
                //     0.001,
                //     Color::red(),
                // ));

                let result = physics_state.query(RayCaster::new(Ray::new(origin, world_ray)));

                for v in result.into_iter().flatten() {
                    // tracing::info!(?v);
                    let entity = world.entity(v.id)?;

                    let point = v.point();

                    gizmos.draw(Sphere::new(
                        entity
                            .get_copy(world_transform())
                            .unwrap_or_default()
                            .transform_point3(Vec3::ZERO),
                        0.1,
                        Color::cyan(),
                    ));

                    gizmos.draw(Sphere::new(point, 0.1, Color::cyan()));
                }

                anyhow::Ok(())
            },
        )
        .boxed()
}
