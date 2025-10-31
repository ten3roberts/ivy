use std::f32::consts::TAU;

use anyhow::Context;
use async_std;
use flax::{components::child_of, entity_ids, Entity, Query, World};
use glam::BVec3;
use glam::{vec3, EulerRot, Quat, Vec3};
use image::Rgba;
use ivy_assets::{stored::DynamicStore, Asset, AssetCache, AssetPath, AsyncAssetExt};
use ivy_core::components::main_camera;
use ivy_core::components::position;
use ivy_core::template::Template;
use ivy_core::DEG_90;
use ivy_core::{
    palette::{Srgb, Srgba},
    plugin::{Plugin, PluginContext},
    transforms::TransformUpdatePlugin,
    update_layer::{FixedTimeStep, PluginLayer, ScheduleSetBuilder},
    AsyncCommandBuffer, Color, ColorExt, EntityBuilderExt,
};
use ivy_engine::scale;
use ivy_engine::{
    async_commandbuffer, elapsed_time, engine, is_static, RigidBodyBundle, TransformBundle,
};
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
use ivy_gltf::Document;
use ivy_graphics::texture::TextureData;
use ivy_input::layer::InputLayer;
use ivy_physics::AxisContraints;
use ivy_physics::{components::collider_builder, ColliderBundle, PhysicsPlugin, RigidBodyKind};
use ivy_postprocessing::effects::SkyboxConfig;
use ivy_postprocessing::preconfigured::pbr::PbrRenderGraphConfig;
use ivy_scene::{GltfNodeExt, NodeMountOptions};
use ivy_wgpu::effect_desc::PbrEmissiveRenderEffect;
use ivy_wgpu::{
    components::*,
    effect_desc::{PbrRenderEffect, RenderEffect},
    light::{LightBundle, LightKind, LightParams},
    material::{EffectPass, Material, MaterialBundle},
    mesh_desc::MeshDesc,
    primitives::{CapsulePrimitive, CubePrimitive, UvSpherePrimitive},
    renderer::MeshBundle,
};
use rand::{rngs::StdRng, Rng, SeedableRng};
use rapier3d::prelude::ColliderBuilder;
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
use violet::palette::{Hsl, Hsv, IntoColor, WithAlpha};

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
            PbrRenderGraphConfig {
                skybox: Some(SkyboxConfig {
                    hdri: Box::new(AssetPath::new("hdris/HDR_artificial_planet_close.hdr")),
                    format: wgpu::TextureFormat::Rgba16Float,
                }),
                ..PbrRenderGraphConfig::default()
            }
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
                        .with_gravity(-Vec3::Y * 9.81),
                )
                .with_plugin(SpawnSpotlightPlugin)
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
            .with_albedo(TextureData::srgba(Color::from_hsla(173.0, 0.7, 0.7, 1.0))),
    );

    let cube_mesh = MeshDesc::Content(assets.load(&CubePrimitive));

    const MOVEMENT_CONFIG: MovementConfiguration = MovementConfiguration {
        max_speed: 6.0,
        max_acceleration: 50.0,
        deceleration: 10.0,
        constraint: MovementConstraint::None,
        movement_mode: MovementMode::ProportionalFalloff,
        kinematic: false,
    };

    let character_height = 1.85;
    let character_radius = 0.3;
    let capsule_halfheight = character_height / 2.0 - character_radius;

    let mesh = MeshDesc::Content(
        assets.load(&CapsulePrimitive::new(character_radius, capsule_halfheight)),
    );

    let material = assets.insert(
        Material::new()
            .with_effect(EffectPass::Forward, red_material.clone())
            .with_effect(EffectPass::Shadow, RenderEffect::OpaqueShadow),
    );

    let character_entity = Template::new()
        .with_bundle(
            TransformBundle::default()
                .with_position(vec3(0.0, 2.0, 0.0))
                .with_rotation(Quat::IDENTITY),
        )
        .with_bundle(CharacterControllerBundle {})
        .with_bundle(
            RigidBodyBundle::dynamic()
                .with_axis_constraints(AxisContraints::new(BVec3::FALSE, BVec3::TRUE)),
        )
        .with_bundle(MoverBundle {
            conf: MOVEMENT_CONFIG,
        })
        .with_bundle(
            ColliderBundle::new(rapier3d::prelude::SharedShape::capsule_y(
                capsule_halfheight,
                character_radius,
            ))
            .with_friction(0.0)
            .with_restitution(0.0),
        )
        .with_bundle(MeshBundle::new(mesh.clone()))
        .with_bundle(MaterialBundle::new(material))
        .build()
        .spawn(world);

    let camera_entity = Template::new()
        .with_bundle(StandaloneCameraBundle)
        .build()
        .spawn(world);

    world
        .entity_mut(character_entity)
        .unwrap()
        .set(camera_target(camera_entity), ());

    let ground_material = assets.insert(
        Material::new()
            .with_effect(EffectPass::Forward, white_material.clone())
            .with_effect(EffectPass::Shadow, RenderEffect::OpaqueShadow),
    );

    let ground_template = Template::new()
        .with_bundle(TransformBundle::default())
        .with_bundle(RigidBodyBundle::new(RigidBodyKind::Fixed))
        .with_bundle(MeshBundle::new(cube_mesh.clone()))
        .with_bundle(MaterialBundle::new(ground_material));

    ground_template
        .build()
        .set(position(), Vec3::ZERO)
        .set(scale(), vec3(100.0, 1.0, 100.0))
        .set(
            collider_builder(),
            ColliderBuilder::cuboid(100.0, 1.0, 100.0).friction(0.3),
        )
        .set(is_static(), ())
        .spawn(world);

    let light_template = Template::new().with_bundle(
        TransformBundle::default().with_rotation(Quat::from_euler(EulerRot::YXZ, -2.0, -1.0, 0.0)),
    );

    light_template
        .build()
        .set(
            light_params(),
            LightParams::new(Srgb::new(1.0, 1.0, 1.0), 1.0),
        )
        .set(light_kind(), LightKind::Directional)
        .set_default(cast_shadow())
        .spawn(world);

    Ok(())
}

impl LogicPlugin {
    fn setup_assets(&self, world: &mut World, assets: &AssetCache) -> anyhow::Result<()> {
        let cmd = world.get(engine(), async_commandbuffer()).unwrap().clone();
        let assets = assets.clone();

        async fn load_additional_objects(
            assets: AssetCache,
            cmd: AsyncCommandBuffer,
        ) -> anyhow::Result<()> {
            let cube_mesh = MeshDesc::Content(assets.load(&CubePrimitive));
            let sphere_mesh = MeshDesc::Content(assets.load(&UvSpherePrimitive::default()));

            // Create materials with different colors
            let red_material = assets.insert(
                Material::new()
                    .with_effect(
                        EffectPass::Forward,
                        RenderEffect::Pbr(
                            PbrRenderEffect::new()
                                .with_roughness_factor(1.0)
                                .with_metallic_factor(0.0)
                                .with_albedo(TextureData::srgba(Color::from_hsla(
                                    0.0, 0.7, 0.7, 1.0,
                                ))),
                        ),
                    )
                    .with_effect(EffectPass::Shadow, RenderEffect::OpaqueShadow),
            );

            let green_material = assets.insert(
                Material::new()
                    .with_effect(
                        EffectPass::Forward,
                        RenderEffect::Pbr(
                            PbrRenderEffect::new()
                                .with_roughness_factor(1.0)
                                .with_metallic_factor(0.0)
                                .with_albedo(TextureData::srgba(Color::from_hsla(
                                    120.0, 0.7, 0.7, 1.0,
                                ))),
                        ),
                    )
                    .with_effect(EffectPass::Shadow, RenderEffect::OpaqueShadow),
            );

            let blue_material = assets.insert(
                Material::new()
                    .with_effect(
                        EffectPass::Forward,
                        RenderEffect::Pbr(
                            PbrRenderEffect::new()
                                .with_roughness_factor(1.0)
                                .with_metallic_factor(0.0)
                                .with_albedo(TextureData::srgba(Color::from_hsla(
                                    240.0, 0.7, 0.7, 1.0,
                                ))),
                        ),
                    )
                    .with_effect(EffectPass::Shadow, RenderEffect::OpaqueShadow),
            );

            let yellow_material = assets.insert(
                Material::new()
                    .with_effect(
                        EffectPass::Forward,
                        RenderEffect::Pbr(
                            PbrRenderEffect::new()
                                .with_roughness_factor(1.0)
                                .with_metallic_factor(0.0)
                                .with_albedo(TextureData::srgba(Color::from_hsla(
                                    60.0, 0.7, 0.7, 1.0,
                                ))),
                        ),
                    )
                    .with_effect(EffectPass::Shadow, RenderEffect::OpaqueShadow),
            );

            let purple_material = assets.insert(
                Material::new()
                    .with_effect(
                        EffectPass::Forward,
                        RenderEffect::Pbr(
                            PbrRenderEffect::new()
                                .with_roughness_factor(1.0)
                                .with_metallic_factor(0.0)
                                .with_albedo(TextureData::srgba(Color::from_hsla(
                                    300.0, 0.7, 0.7, 1.0,
                                ))),
                        ),
                    )
                    .with_effect(EffectPass::Shadow, RenderEffect::OpaqueShadow),
            );

            let materials = [
                red_material,
                green_material,
                blue_material,
                yellow_material,
                purple_material,
            ];

            let mut rng = StdRng::from_seed([42; 32]);
            const SPAWN_RANGE: f32 = 50.0;

            // Add some random cubes
            for i in 0..50 {
                let x: f32 = rng.random_range(-SPAWN_RANGE..SPAWN_RANGE);
                let z: f32 = rng.random_range(-SPAWN_RANGE..SPAWN_RANGE);
                let y: f32 = rng.random_range(1.0..3.0);

                let material = materials[i % materials.len()].clone();

                Template::new()
                    .with_bundle(TransformBundle::default())
                    .with_bundle(RigidBodyBundle::dynamic())
                    .with_bundle(
                        ColliderBundle::new(rapier3d::prelude::SharedShape::cuboid(1.0, 1.0, 1.0))
                            .with_friction(0.0)
                            .with_restitution(1.0),
                    )
                    .with_bundle(MeshBundle::new(cube_mesh.clone()))
                    .with_bundle(MaterialBundle::new(material))
                    .build()
                    .set(ivy_core::components::position(), vec3(x, y, z))
                    .spawn_into(&mut cmd.lock());
            }

            // Add some spheres
            for i in 0..5 {
                let x: f32 = rng.random_range(-SPAWN_RANGE..SPAWN_RANGE);
                let z: f32 = rng.random_range(-SPAWN_RANGE..SPAWN_RANGE);
                let y: f32 = rng.random_range(1.0..3.0);

                let material = materials[i % materials.len()].clone();

                Template::new()
                    .with_bundle(TransformBundle::default())
                    .with_bundle(RigidBodyBundle::dynamic())
                    .with_bundle(
                        ColliderBundle::new(rapier3d::prelude::SharedShape::ball(1.0))
                            .with_friction(0.8)
                            .with_restitution(1.0),
                    )
                    .with_bundle(MeshBundle::new(sphere_mesh.clone()))
                    .with_bundle(MaterialBundle::new(material))
                    .build()
                    .set(ivy_core::components::position(), vec3(x, y, z))
                    .spawn_into(&mut cmd.lock());
            }

            anyhow::Ok(())
        }

        async fn load_crates(assets: AssetCache, cmd: AsyncCommandBuffer) -> anyhow::Result<()> {
            // Assuming a crate GLTF model exists, e.g., "models/Crate.glb"
            // If not, this will fail, but for the example, we'll try
            let document: Asset<Document> = AssetPath::new("models/Crate.glb")
                .load_async(&assets)
                .await?;

            for node in document.nodes() {
                node.mount(
                    &mut Entity::builder(),
                    &NodeMountOptions {
                        skip_empty_children: true,
                        material_overrides: &Default::default(),
                    },
                )
                .mount(TransformBundle::new(
                    vec3(5.0, 1.0, -5.0), // Position for the crate
                    Quat::IDENTITY,
                    Vec3::ONE,
                ))
                .spawn_into(&mut cmd.lock());
            }

            anyhow::Ok(())
        }

        async_std::task::spawn(load_additional_objects(assets.clone(), cmd.clone()));
        async_std::task::spawn(load_crates(assets, cmd));

        Ok(())
    }
}

struct LogicPlugin;

impl Plugin for LogicPlugin {
    fn install(&self, ctx: &mut PluginContext) -> anyhow::Result<()> {
        setup_objects(ctx.world, ctx.assets.clone())?;
        self.setup_assets(ctx.world, ctx.assets)
    }
}

struct SpawnSpotlightPlugin;

impl Plugin for SpawnSpotlightPlugin {
    fn install(&self, ctx: &mut PluginContext) -> anyhow::Result<()> {
        let mut rng = StdRng::from_seed([123; 32]); // Different seed for lights

        let count = 4; // Several spotlights for scattered illumination

        let sphere_mesh = MeshDesc::content(ctx.assets.load(&UvSpherePrimitive::default()));

        // Scattered spotlights like streetlamps
        for i in 0..count {
            let radius = 20.0;
            let theta = (i as f32 / count as f32) * TAU;
            let x: f32 = radius * -theta.cos();
            let z: f32 = radius * theta.sin();
            let y: f32 = 8.0;

            let color = Hsv::new(theta.to_degrees(), 1.0, 1.0);

            let grey_material = ctx.assets.insert(
                Material::new()
                    .with_effect(
                        EffectPass::Forward,
                        RenderEffect::Emissive(PbrEmissiveRenderEffect::new(
                            PbrRenderEffect::new().with_roughness_factor(0.0),
                            TextureData::srgba(color.with_alpha(1.0).into_color()),
                            5.0,
                        )),
                    )
                    .with_effect(EffectPass::Shadow, RenderEffect::OpaqueShadow),
            );

            Entity::builder()
                .mount(
                    TransformBundle::default()
                        .with_position(vec3(x, y, z))
                        .with_scale(Vec3::splat(0.4)), // Small sphere
                )
                .mount(MeshBundle::new(sphere_mesh.clone()))
                .mount(MaterialBundle::new(grey_material.clone()))
                .spawn(ctx.world);

            Entity::builder()
                .mount(
                    TransformBundle::default()
                        .with_position(vec3(x, y, z))
                        .with_rotation(Quat::from_rotation_x(-DEG_90)),
                )
                .mount(LightBundle {
                    params: LightParams::new(color.into_color(), 80.0)
                        .with_angular_cutoffs(0.7, 0.8), // Focused beam
                    kind: LightKind::Spotlight,
                    cast_shadow: true,
                })
                .spawn(ctx.world);
        }

        Ok(())
    }
}
