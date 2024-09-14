use flax::{Entity, Query, World};
use glam::{vec3, EulerRot, Mat4, Quat, Vec3};
use ivy_assets::AssetCache;
use ivy_collision::components::collider;
use ivy_core::{
    app::InitEvent,
    layer::events::EventRegisterContext,
    palette::{
        num::{Sqrt, Trigonometry},
        Srgb, Srgba,
    },
    profiling::ProfilingLayer,
    update_layer::{FixedTimeStep, PerTick, ScheduledLayer},
    App, Color, ColorExt, EngineLayer, EntityBuilderExt, Layer, DEG_45,
};
use ivy_engine::{
    angular_velocity, friction, gravity_influence, is_static, main_camera, restitution, scale,
    velocity, Collider, RigidBodyBundle, TransformBundle,
};
use ivy_game::free_camera::{setup_camera, CameraInputPlugin};
use ivy_graphics::texture::TextureDesc;
use ivy_input::layer::InputLayer;
use ivy_physics::PhysicsPlugin;
use ivy_postprocessing::preconfigured::{SurfacePbrPipeline, SurfacePbrPipelineDesc};
use ivy_wgpu::{
    components::*,
    driver::WinitDriver,
    events::ResizedEvent,
    layer::GraphicsLayer,
    light::{LightData, LightKind},
    material_desc::{MaterialData, MaterialDesc},
    mesh_desc::MeshDesc,
    primitives::{CapsulePrimitive, CubePrimitive},
    renderer::{EnvironmentData, RenderObjectBundle},
    shaders::{PbrShaderDesc, ShadowShaderDesc},
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
                SurfacePbrPipelineDesc { hdri: None },
            ))
        }))
        .with_layer(InputLayer::new())
        .with_layer(LogicLayer)
        .with_layer(ScheduledLayer::new(PerTick).with_plugin(CameraInputPlugin))
        .with_layer(
            ScheduledLayer::new(FixedTimeStep::new(0.02)).with_plugin(
                PhysicsPlugin::new()
                    .with_gizmos(ivy_physics::GizmoSettings {
                        bvh_tree: false,
                        island_graph: true,
                        rigidbody: true,
                        contacts: true,
                    })
                    .with_gravity(-Vec3::Y * 1.0),
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
            .with_roughness_factor(0.1)
            .with_metallic_factor(0.0)
            .with_albedo(TextureDesc::srgba(Color::from_hsla(0.0, 0.7, 0.7, 1.0))),
    );

    let cube_mesh = MeshDesc::Content(assets.load(&CubePrimitive));
    let shader = assets.load(&PbrShaderDesc);
    let shadow = assets.load(&ShadowShaderDesc);

    const RESTITUTION: f32 = 1.0;
    const FRICTION: f32 = 0.5;
    const MASS: f32 = 50.0;
    const INERTIA_TENSOR: f32 = 10.0;

    let cube = |position: Vec3, rotation: Quat| {
        let mesh = MeshDesc::Content(assets.load(&CubePrimitive));

        let mut builder = Entity::builder();
        builder
            .mount(
                TransformBundle::default()
                    .with_position(position)
                    .with_rotation(rotation),
            )
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
                mesh.clone(),
                white_material.clone(),
                shader.clone(),
            ))
            .set(shadow_pass(), shadow.clone());

        builder
    };

    cube(Vec3::ZERO, Quat::IDENTITY)
        .set(scale(), vec3(10.0, 1.0, 10.0))
        .set(is_static(), ())
        .spawn(world);

    cube(Vec3::Y * 8.0, Quat::from_scaled_axis(vec3(1.0, 0.0, 0.0)))
        .set(gravity_influence(), 1.0)
        .set(restitution(), 1.0)
        .spawn(world);

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
