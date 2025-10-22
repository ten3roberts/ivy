use anyhow::Context;
use flax::{entity_ids, Entity, Query, World};
use glam::BVec3;
use glam::{vec3, EulerRot, Quat, Vec3};
use ivy_assets::{stored::DynamicStore, AssetCache};
use ivy_core::components::main_camera;
use ivy_core::components::position;
use ivy_core::template::Template;
use ivy_core::{
    palette::{Srgb, Srgba},
    plugin::{Plugin, PluginContext},
    transforms::TransformUpdatePlugin,
    update_layer::{FixedTimeStep, PluginLayer, ScheduleSetBuilder},
    Color, ColorExt, EntityBuilderExt,
};
use ivy_engine::scale;
use ivy_engine::{is_static, RigidBodyBundle, TransformBundle};
use ivy_game::standalone_camera::StandaloneCameraBundle;
use ivy_game::{
    controllers::{
        camera_controller::{camera_target, CameraControllerPlugin},
        character_controller::{CharacterControllerBundle, CharacterControllerPlugin},
    },
    navigation::{
        MovementConfiguration, MovementConstraint, MovementMode, MoverBundle, MoverPlugin,
    },
    viewport_camera::CameraViewportPlugin,
};
use ivy_graphics::texture::TextureData;
use ivy_input::layer::InputLayer;
use ivy_physics::AxisContraints;
use ivy_physics::{components::collider_builder, ColliderBundle, PhysicsPlugin, RigidBodyKind};
use ivy_postprocessing::preconfigured::pbr::PbrRenderGraphConfig;
use ivy_wgpu::{
    components::*,
    effect_desc::{PbrRenderEffect, RenderEffect},
    light::{LightKind, LightParams},
    mesh_desc::MeshDesc,
    primitives::{CapsulePrimitive, CubePrimitive},
    renderer::RenderObjectBundle,
};
use rapier3d::prelude::ColliderBuilder;
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;

mod common;

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

    if let Err(err) = common::base_app_builder("Ivy Character Controller")
        .with_layer(common::graphics_layer_with_config(|| {
            PbrRenderGraphConfig::default()
        }))
        .with_layer(InputLayer::new())
        .with_layer(
            PluginLayer::new(FixedTimeStep::new(0.02))
                .with_plugin(LogicPlugin)
                .with_plugin(CameraViewportPlugin)
                .with_plugin(CameraControllerPlugin)
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
            .mount(
                RigidBodyBundle::dynamic()
                    .with_axis_constraints(AxisContraints::new(BVec3::FALSE, BVec3::TRUE)),
            )
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
        .mount(TransformBundle::default())
        .set(position(), Vec3::ZERO)
        .set(scale(), vec3(100.0, 1.0, 100.0))
        .mount(RigidBodyBundle::new(RigidBodyKind::Fixed))
        .set(
            collider_builder(),
            ColliderBuilder::cuboid(100.0, 1.0, 100.0),
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
