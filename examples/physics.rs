use flax::{
    fetch::Source, BoxedSystem, Component, Entity, FetchExt, Query, QueryBorrow, System, World,
};
use glam::{vec3, EulerRot, Mat4, Quat, Vec3};
use ivy_assets::AssetCache;
use ivy_collision::components::collider;
use ivy_core::{
    app::InitEvent,
    gizmos::{self, DEFAULT_RADIUS},
    layer::events::EventRegisterContext,
    palette::{Srgb, Srgba},
    profiling::ProfilingLayer,
    update_layer::{FixedTimeStep, PerTick, ScheduledLayer},
    App, Color, ColorExt, EngineLayer, EntityBuilderExt, Layer, DEG_180, DEG_45,
};
use ivy_engine::{
    delta_time, engine, main_camera, world_transform, Collider, RigidBodyBundle, TransformBundle,
};
use ivy_game::free_camera::{setup_camera, CameraInputPlugin};
use ivy_gltf::components::animator;
use ivy_graphics::texture::TextureDesc;
use ivy_input::layer::InputLayer;
use ivy_physics::PhysicsPlugin;
use ivy_postprocessing::preconfigured::{SurfacePbrPipeline, SurfacePbrPipelineDesc};
use ivy_wgpu::{
    components::{cast_shadow, environment_data, light_data, light_kind, projection_matrix},
    driver::WinitDriver,
    events::ResizedEvent,
    layer::GraphicsLayer,
    light::{LightData, LightKind},
    material_desc::{MaterialData, MaterialDesc},
    mesh_desc::MeshDesc,
    primitives::CapsulePrimitive,
    renderer::{EnvironmentData, RenderObjectBundle},
    shaders::PbrShaderDesc,
};
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
use winit::{dpi::LogicalSize, window::WindowAttributes};

const ENABLE_SKYBOX: bool = true;

pub fn main() -> anyhow::Result<()> {
    registry()
        .with(EnvFilter::from_default_env())
        .with(
            HierarchicalLayer::default()
                .with_indent_lines(true)
                .with_deferred_spans(true)
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
                    hdri: Some(Box::new("hdris/HDR_artificial_planet_close.hdr")),
                },
            ))
        }))
        .with_layer(InputLayer::new())
        .with_layer(LogicLayer)
        .with_layer(ScheduledLayer::new(PerTick).with_plugin(CameraInputPlugin))
        .with_layer(ScheduledLayer::new(FixedTimeStep::new(0.02)).with_plugin(
            PhysicsPlugin::new().with_gizmos(ivy_physics::GizmoSettings {
                bvh_tree: false,
                island_graph: true,
                rigidbody: true,
                contacts: true,
            }),
        ))
        .run()
    {
        tracing::error!("{err:?}");
        Err(err)
    } else {
        Ok(())
    }
}

fn setup_objects(world: &mut World, assets: AssetCache) -> anyhow::Result<()> {
    let material = MaterialDesc::Content(
        MaterialData::new()
            .with_roughness_factor(0.1)
            .with_metallic_factor(0.0)
            .with_albedo(TextureDesc::srgba(Srgba::new(1.0, 1.0, 1.0, 1.0))),
    );

    let cube_mesh = MeshDesc::Content(assets.load(&CapsulePrimitive::default()));

    let shader = assets.load(&PbrShaderDesc);

    let simulate = true;

    let mut cube = |mut position: Vec3, rotation: Quat| {
        let mut velocity = Vec3::ZERO;
        if simulate {
            position.z = position.z.signum() * 4.0;

            velocity = Vec3::Z * -position.z.signum() * 1.0;
        }

        Entity::builder()
            .mount(
                TransformBundle::default()
                    .with_position(position + Vec3::Z)
                    .with_rotation(rotation),
            )
            .mount(
                RigidBodyBundle::default()
                    .with_velocity(velocity)
                    .with_mass(5.0)
                    .with_angular_mass(10.0)
                    .with_restitution(0.1)
                    .with_friction(0.7),
            )
            .set(
                collider(),
                // Collider::cube_from_center(Vec3::ZERO, Vec3::ONE),
                Collider::capsule(1.0, 1.0),
            )
            .mount(RenderObjectBundle::new(
                cube_mesh.clone(),
                material.clone(),
                shader.clone(),
            ))
            .spawn(world);
    };

    // Twisted and offset
    cube(vec3(0.2, 0.0, 0.99), Quat::IDENTITY);
    cube(
        vec3(0.0, 0.0, -0.99),
        Quat::from_scaled_axis(vec3(0.0, 0.0, 0.5)),
    );

    // Twisted
    cube(vec3(4.0, 0.0, 0.99), Quat::IDENTITY);
    cube(
        vec3(4.0, 0.0, -0.99),
        Quat::from_scaled_axis(vec3(0.0, 0.0, 0.5)),
    );

    // Offset
    cube(vec3(8.4, 0.0, 0.99), Quat::IDENTITY);
    cube(
        vec3(8.0, 0.0, -0.99),
        Quat::from_scaled_axis(vec3(0.0, 0.0, 0.0)),
    );

    // edge-face
    cube(
        vec3(-4.0, 0.5, 0.9),
        Quat::from_scaled_axis(vec3(0.0, 0.0, DEG_180)),
    );
    cube(
        vec3(-4.0, 0.0, -1.0),
        Quat::from_scaled_axis(vec3(0.0, 0.5, 0.0)),
    );

    // edge-edge
    cube(
        vec3(-8.0, 0.5, 0.8),
        Quat::from_scaled_axis(vec3(DEG_45, 0.0, 0.0)),
    );
    cube(
        vec3(-8.0, 0.0, -0.8),
        Quat::from_scaled_axis(vec3(0.0, 0.5, 0.0)),
    );

    // point-face
    cube(vec3(-12.0, 0.0, 1.3), Quat::IDENTITY);
    cube(
        vec3(-12.0, 0.0, -1.3),
        Quat::from_scaled_axis(vec3(0.5, 0.5, 0.0)),
    );

    Entity::builder()
        .mount(TransformBundle::default().with_rotation(Quat::from_euler(
            EulerRot::YXZ,
            1.0,
            1.0,
            0.0,
        )))
        .set(light_data(), LightData::new(Srgb::new(1.0, 1.0, 1.0), 0.4))
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
                tracing::info!(%aspect);
                *main_camera = Mat4::perspective_rh(1.0, aspect, 0.1, 1000.0);
            }

            Ok(())
        });

        setup_camera()
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

fn animate_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new((
            animator().as_mut(),
            delta_time()
                .source(engine())
                .expect("delta_time must be present"),
        )))
        .par_for_each(move |(animator, dt)| {
            animator.step(dt.as_secs_f32());
        })
        .boxed()
}

fn gizmos_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new(ivy_core::components::gizmos().source(engine())))
        .with_query(Query::new(world_transform()))
        .build(
            |mut gizmos: QueryBorrow<Source<Component<gizmos::Gizmos>, Entity>>,
             mut query: QueryBorrow<Component<Mat4>>| {
                let mut gizmos = gizmos.first().unwrap().begin_section("gizmos_system");

                gizmos.draw(gizmos::Sphere::new(
                    Vec3::ZERO,
                    DEFAULT_RADIUS,
                    Color::red(),
                ));

                for transform in query.iter() {
                    gizmos.draw(gizmos::Sphere::new(
                        transform.transform_point3(Vec3::ZERO),
                        DEFAULT_RADIUS,
                        Color::red(),
                    ));
                }
            },
        )
        .boxed()
}