use flax::{Entity, World};
use glam::{vec3, EulerRot, Quat, Vec3};
use ivy_assets::AssetCache;
use ivy_core::{
    app::PostInitEvent,
    layer::events::EventRegisterContext,
    palette::{Srgb, Srgba},
    profiling::ProfilingLayer,
    update_layer::{FixedTimeStep, ScheduledLayer},
    App, Color, ColorExt, EngineLayer, EntityBuilderExt, Layer,
};
use ivy_engine::{is_static, RigidBodyBundle, TransformBundle};
use ivy_game::{
    fly_camera::FlyCameraPlugin,
    viewport_camera::{CameraSettings, ViewportCameraLayer},
};
use ivy_graphics::texture::TextureData;
use ivy_input::layer::InputLayer;
use ivy_physics::{
    components::{angular_velocity, friction, gravity_influence},
    ColliderBundle, PhysicsPlugin,
};
use ivy_postprocessing::preconfigured::{
    pbr::PbrRenderGraphConfig, SurfacePbrPipelineDesc, SurfacePbrRenderer,
};
use ivy_wgpu::{
    components::*,
    driver::WinitDriver,
    layer::GraphicsLayer,
    light::{LightKind, LightParams},
    material_desc::{MaterialData, PbrMaterialData},
    mesh_desc::MeshDesc,
    primitives::{CapsulePrimitive, CubePrimitive},
    renderer::{EnvironmentData, RenderObjectBundle},
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
        .with_layer(GraphicsLayer::new(|world, assets, store, gpu, surface| {
            Ok(SurfacePbrRenderer::new(
                world,
                assets,
                store,
                gpu,
                surface,
                SurfacePbrPipelineDesc {
                    pbr_config: PbrRenderGraphConfig {
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ))
        }))
        .with_layer(InputLayer::new())
        .with_layer(LogicLayer)
        .with_layer(
            ScheduledLayer::new(FixedTimeStep::new(0.02))
                .with_plugin(FlyCameraPlugin)
                .with_plugin(
                    PhysicsPlugin::new()
                        .with_gizmos(ivy_physics::GizmoSettings { rigidbody: true })
                        .with_gravity(-Vec3::Y),
                ),
        )
        .with_layer(ViewportCameraLayer::new(CameraSettings {
            environment_data: EnvironmentData::new(
                Srgb::new(0.2, 0.2, 0.3),
                0.001,
                if ENABLE_SKYBOX { 0.0 } else { 1.0 },
            ),
            fov: 1.0,
        }))
        .run()
    {
        tracing::error!("{err:?}");
        Err(err)
    } else {
        Ok(())
    }
}

fn setup_objects(world: &mut World, assets: AssetCache) -> anyhow::Result<()> {
    let white_material = MaterialData::PbrMaterial(
        PbrMaterialData::new()
            .with_roughness_factor(1.0)
            .with_metallic_factor(0.0)
            .with_albedo(TextureData::srgba(Srgba::new(1.0, 1.0, 1.0, 1.0))),
    );

    let red_material = MaterialData::PbrMaterial(
        PbrMaterialData::new()
            .with_roughness_factor(0.1)
            .with_metallic_factor(0.0)
            .with_albedo(TextureData::srgba(Color::from_hsla(0.0, 0.7, 0.7, 1.0))),
    );

    let cube_mesh = MeshDesc::Content(assets.load(&CubePrimitive));

    const RESTITUTION: f32 = 0.0;
    const FRICTION: f32 = 0.8;
    const MASS: f32 = 20.0;
    const INERTIA_TENSOR: f32 = 10.0;

    let capsule = |position: Vec3, rotation: Quat| {
        let mesh = MeshDesc::Content(assets.load(&CapsulePrimitive::default()));

        let mut builder = Entity::builder();
        builder
            .mount(
                TransformBundle::default()
                    .with_position(position)
                    .with_rotation(rotation),
            )
            .mount(
                RigidBodyBundle::dynamic()
                    .with_mass(MASS)
                    .with_angular_mass(INERTIA_TENSOR),
            )
            .mount(
                ColliderBundle::new(rapier3d::prelude::SharedShape::capsule_y(1.0, 1.0))
                    .with_friction(FRICTION)
                    .with_restitution(RESTITUTION),
            )
            .mount(RenderObjectBundle::new(
                mesh.clone(),
                &[
                    (forward_pass(), white_material.clone()),
                    (shadow_pass(), MaterialData::ShadowMaterial),
                ],
            ));

        builder
    };

    capsule(
        vec3(0.0, 2.0, 0.0),
        Quat::from_scaled_axis(vec3(0.0, 0.0, 0.1)),
    )
    .set(forward_pass(), red_material.clone())
    .set(friction(), 0.8)
    .set(angular_velocity(), Vec3::Y * 10.0)
    .set(gravity_influence(), 1.0)
    .spawn(world);

    Entity::builder()
        .mount(
            TransformBundle::default()
                .with_scale(vec3(5.0, 0.1, 5.0))
                .with_rotation(Quat::from_scaled_axis(Vec3::Z * 0.1)),
        )
        .mount(RigidBodyBundle::dynamic().with_mass(1.0))
        .mount(
            ColliderBundle::new(rapier3d::prelude::SharedShape::cuboid(1.0, 1.0, 1.0))
                .with_friction(FRICTION)
                .with_restitution(RESTITUTION),
        )
        .set(is_static(), ())
        .mount(RenderObjectBundle::new(
            cube_mesh.clone(),
            &[
                (forward_pass(), white_material.clone()),
                (shadow_pass(), MaterialData::ShadowMaterial),
            ],
        ))
        .spawn(world);

    Entity::builder()
        .mount(
            TransformBundle::default()
                .with_position(vec3(-7.0, -3.0, 0.0))
                .with_scale(vec3(20.0, 0.1, 20.0))
                .with_rotation(Quat::from_scaled_axis(Vec3::Z * -0.2)),
        )
        .mount(RigidBodyBundle::dynamic().with_mass(1.0))
        .mount(
            ColliderBundle::new(rapier3d::prelude::SharedShape::cuboid(1.0, 1.0, 1.0))
                .with_friction(FRICTION)
                .with_restitution(RESTITUTION),
        )
        .set(is_static(), ())
        .mount(RenderObjectBundle::new(
            cube_mesh.clone(),
            &[
                (forward_pass(), white_material),
                (shadow_pass(), MaterialData::ShadowMaterial),
            ],
        ))
        .spawn(world);

    Entity::builder()
        .mount(TransformBundle::default().with_rotation(Quat::from_euler(
            EulerRot::YXZ,
            -2.0,
            1.0,
            0.0,
        )))
        .set(
            light_params(),
            LightParams::new(Srgb::new(1.0, 1.0, 1.0), 1.0),
        )
        .set(light_kind(), LightKind::Directional)
        .set_default(cast_shadow())
        .spawn(world);

    Ok(())
}

struct LogicLayer;

impl Layer for LogicLayer {
    fn register(
        &mut self,
        _: &mut World,
        _: &AssetCache,
        mut events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()> {
        events.subscribe(|_, ctx, _: &PostInitEvent| {
            setup_objects(ctx.world, ctx.assets.clone())?;

            Ok(())
        });

        Ok(())
    }
}
