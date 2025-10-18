use anyhow::Context;
use flax::{entity_ids, Entity, Query, World};
use glam::{vec3, EulerRot, Quat, Vec3};
use ivy_assets::{stored::DynamicStore, AssetCache};
use ivy_core::components::main_camera;
use ivy_core::template::Template;
use ivy_core::{
    palette::{Srgb, Srgba},
    plugin::{Plugin, PluginContext},
    profiling::ProfilingLayer,
    transforms::TransformUpdatePlugin,
    update_layer::{FixedTimeStep, PluginLayer, ScheduleSetBuilder},
    App, Color, ColorExt, EngineLayer, EntityBuilderExt,
};
use ivy_engine::{is_static, RigidBodyBundle, TransformBundle};
use ivy_game::standalone_camera::StandaloneCameraBundle;
use ivy_game::{
    controllers::{
        camera_controller::{camera_target, CameraTrackingPlugin},
        character_controller::{CharacterControllerBundle, CharacterControllerPlugin},
    },
    navigation::{
        MovementConfiguration, MovementConstraint, MovementMode, MoverBundle, MoverPlugin,
    },
    standalone_camera::StandaloneCameraPlugin,
    viewport_camera::CameraViewportPlugin,
};
use ivy_graphics::texture::TextureData;
use ivy_input::layer::InputLayer;
use ivy_physics::{ColliderBundle, PhysicsPlugin};
use ivy_postprocessing::preconfigured::{
    pbr::PbrRenderGraphConfig, SurfacePbrPipelineDesc, SurfacePbrRenderer,
};
use ivy_wgpu::{
    components::*,
    driver::WinitDriver,
    effect_desc::{PbrRenderEffect, RenderEffect},
    layer::GraphicsLayer,
    light::{LightKind, LightParams},
    mesh_desc::MeshDesc,
    primitives::{CapsulePrimitive, CubePrimitive},
    renderer::RenderObjectBundle,
};
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
use winit::{dpi::LogicalSize, window::WindowAttributes};

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
                .with_title("Ivy Character Controller"),
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
        .with_layer(
            PluginLayer::new(FixedTimeStep::new(0.02))
                .with_plugin(LogicPlugin)
                .with_plugin(CameraViewportPlugin)
                .with_plugin(StandaloneCameraPlugin)
                .with_plugin(CameraTrackingPlugin)
                .with_plugin(CharacterControllerPlugin)
                .with_plugin(MoverPlugin)
                .with_plugin(
                    PhysicsPlugin::new()
                        .with_gizmos(ivy_physics::GizmoSettings { rigidbody: true })
                        .with_gravity(-Vec3::Y),
                )
                .with_plugin(TransformUpdatePlugin),
        )
        .run()
    {
        Err(err)
    } else {
        Ok(())
    }
}

fn setup_objects(world: &mut World, assets: AssetCache) -> anyhow::Result<()> {
    let white_material = RenderEffect::Pbr(
        PbrRenderEffect::new()
            .with_roughness_factor(1.0)
            .with_metallic_factor(0.0)
            .with_albedo(TextureData::srgba(Srgba::new(1.0, 1.0, 1.0, 1.0))),
    );

    let red_material = RenderEffect::Pbr(
        PbrRenderEffect::new()
            .with_roughness_factor(0.1)
            .with_metallic_factor(0.0)
            .with_albedo(TextureData::srgba(Color::from_hsla(0.0, 0.7, 0.7, 1.0))),
    );

    let cube_mesh = MeshDesc::Content(assets.load(&CubePrimitive));

    const RESTITUTION: f32 = 0.0;
    const FRICTION: f32 = 0.8;
    const MASS: f32 = 20.0;
    const INERTIA_TENSOR: f32 = 10.0;

    const MOVEMENT_CONFIG: MovementConfiguration = MovementConfiguration {
        max_speed: 5.0,
        max_acceleration: 20.0,
        deceleration: 10.0,
        constraint: MovementConstraint::None,
        movement_mode: MovementMode::ProportionalFalloff,
        kinematic: false,
    };

    let capsule = |position: Vec3, rotation: Quat| {
        let mesh = MeshDesc::Content(assets.load(&CapsulePrimitive::default()));

        let mut builder = Entity::builder();
        builder
            .mount(
                TransformBundle::default()
                    .with_position(position)
                    .with_rotation(rotation),
            )
            .mount(CharacterControllerBundle {})
            .mount(RigidBodyBundle::dynamic())
            .mount(MoverBundle {
                conf: MOVEMENT_CONFIG,
            })
            .mount(
                ColliderBundle::new(rapier3d::prelude::SharedShape::capsule_y(1.0, 1.0))
                    .with_friction(FRICTION)
                    .with_restitution(RESTITUTION),
            )
            .mount(RenderObjectBundle::new(
                mesh.clone(),
                &[
                    (forward_pass(), white_material.clone()),
                    (shadow_pass(), RenderEffect::OpaqueShadow),
                ],
            ));

        builder
    };

    let character_entity = capsule(
        vec3(0.0, 2.0, 0.0),
        Quat::from_scaled_axis(vec3(0.0, 0.0, 0.1)),
    )
    .set(forward_pass(), red_material.clone())
    .spawn(world);

    let camera_entity = Template::new()
        .with_bundle(StandaloneCameraBundle)
        .build()
        .spawn(world);

    world
        .entity_mut(character_entity)
        .unwrap()
        .set(camera_target(camera_entity), ());

    Entity::builder()
        .mount(
            TransformBundle::default()
                .with_scale(vec3(5.0, 0.1, 5.0))
                .with_rotation(Quat::from_scaled_axis(Vec3::Z * 0.1)),
        )
        .mount(RigidBodyBundle::fixed().with_mass(1.0))
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
                (shadow_pass(), RenderEffect::OpaqueShadow),
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
        .mount(RigidBodyBundle::fixed().with_mass(1.0))
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
                (shadow_pass(), RenderEffect::OpaqueShadow),
            ],
        ))
        .spawn(world);

    Entity::builder()
        .mount(TransformBundle::default().with_rotation(Quat::from_euler(
            EulerRot::YXZ,
            -2.0,
            -1.0,
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

struct LogicPlugin;

impl Plugin for LogicPlugin {
    fn install(&self, ctx: &mut PluginContext) -> anyhow::Result<()> {
        setup_objects(ctx.world, ctx.assets.clone())
    }
}
