use std::any::type_name;

use flax::{Entity, World};
use glam::{vec3, EulerRot, Quat, Vec2, Vec3};
use ivy_assets::{stored::DynamicStore, AssetCache, AssetPath};
use ivy_core::{
    palette::Srgb,
    plugin::{Plugin, PluginContext},
    profiling::ProfilingLayer,
    transforms::TransformUpdatePlugin,
    update_layer::{FixedTimeStep, PluginLayer, ScheduleSetBuilder},
    App, ColorExt, EngineLayer, EntityBuilderExt,
};
use ivy_editor::{
    host::EditorHostPlugin,
    plugin::EditorPlugin,
    tools::{physics_tool::PhysicsToolPlugin, transform_tool::TransformToolPlugin},
    tools_controller::ToolsControllerPlugin,
};
use ivy_engine::{engine, is_static, rotation, scale, RigidBodyBundle, TransformBundle};
use ivy_game::{fly_camera::FlyCameraPlugin, viewport_camera::CameraViewportPlugin};
use ivy_gltf::animation::plugin::AnimationPlugin;
use ivy_graphics::texture::TextureData;
use ivy_input::layer::InputLayer;
use ivy_physics::{components::collider_builder, ColliderBundle, PhysicsPlugin, RigidBodyKind};
use ivy_postprocessing::{
    effects::SkyboxConfig,
    preconfigured::{pbr::PbrRenderGraphConfig, SurfacePbrPipelineDesc, SurfacePbrRenderer},
};
use ivy_scene::{
    ray_picker::RayPickingPlugin, ser::SceneData, viewport_provider::SceneViewportProvider, Scene,
    SceneLayer,
};
use ivy_ui::{
    layer::{UiLayer, UiLayerOptions, UiUpdateLayer},
    streamed::StreamedUiPlugin,
    toast::ToastPlugin,
};
use ivy_wgpu::{
    driver::WinitDriver,
    effect_desc::{PbrRenderEffect, RenderEffect},
    light::{LightBundle, LightKind, LightParams},
    material::{EffectPass, Material, MaterialBundle},
    primitives::{CapsulePrimitive, CubePrimitive, PrimitiveBundle, UvSpherePrimitive},
};
use rapier3d::prelude::{ColliderBuilder, SharedShape};
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
use violet::palette::Srgba;
use winit::{dpi::LogicalSize, window::WindowAttributes};

mod common;

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

    let scene_constructor = || {
        Scene::builder()
            .with_layer(EngineLayer::new())
            .with_layer(
                UiLayer::with_options(UiLayerOptions {
                    label: "scene_ui".into(),
                    follow_window_size: false,
                    ..Default::default()
                })
                .with_label("scene_ui"),
            )
            .with_layer(InputLayer::new())
            .with_layer(
                PluginLayer::new(FixedTimeStep::new(0.02))
                    .with_plugin(StreamedUiPlugin)
                    .with_plugin(SetupPlugin)
                    .with_plugin(FlyCameraPlugin)
                    .with_plugin(AnimationPlugin)
                    .with_plugin(CameraViewportPlugin)
                    .with_plugin(PhysicsPlugin::new())
                    .with_plugin(TransformToolPlugin)
                    .with_plugin(PhysicsToolPlugin)
                    .with_plugin(ToolsControllerPlugin)
                    .with_plugin(RayPickingPlugin)
                    .with_plugin(EditorPlugin)
                    .with_plugin(TransformUpdatePlugin),
            )
            .with_layer(UiUpdateLayer::new())
    };

    if let Err(err) = App::builder()
        .with_driver(WinitDriver::new(
            WindowAttributes::default()
                .with_inner_size(LogicalSize::new(1920, 1080))
                .with_title("Ivy"),
        ))
        .with_layer(EngineLayer::new())
        .with_layer(common::graphics_layer_with_config(|| {
            PbrRenderGraphConfig::default()
        }))
        .with_layer(ProfilingLayer::new())
        .with_layer(UiLayer::new())
        .with_layer(InputLayer::new())
        .with_layer(
            PluginLayer::new(FixedTimeStep::new(0.02))
                .with_plugin(
                    EditorHostPlugin::new(scene_constructor).with_scene(SceneData {
                        world: World::new(),
                    }),
                )
                .with_plugin(ToastPlugin)
                .with_plugin(StreamedUiPlugin),
        )
        .with_layer(SceneLayer::new())
        .with_layer(SceneViewportProvider::new())
        .with_layer(UiUpdateLayer::new())
        .run()
    {
        tracing::error!("{err:?}");
        Err(err)
    } else {
        Ok(())
    }
}

struct SetupPlugin;

impl Plugin for SetupPlugin {
    fn install(&self, ctx: &mut PluginContext) -> anyhow::Result<()> {
        Self::setup_objects(ctx.world, ctx.assets)
    }

    fn after(&self) -> Vec<&str> {
        vec![
            type_name::<PhysicsPlugin>(),
            type_name::<TransformUpdatePlugin>(),
        ]
    }
}

impl SetupPlugin {
    fn setup_objects(world: &mut World, assets: &AssetCache) -> anyhow::Result<()> {
        const RESTITUTION: f32 = 0.1;
        const FRICTION: f32 = 0.8;

        let red_material = assets.insert(
            Material::new()
                .with_effect(
                    EffectPass::Forward,
                    RenderEffect::Pbr(
                        PbrRenderEffect::new()
                            .with_roughness_factor(1.0)
                            .with_metallic_factor(0.0)
                            .with_albedo(TextureData::srgba(Srgba::from_hsla(1.0, 0.7, 0.7, 1.0))),
                    ),
                )
                .with_effect(EffectPass::Shadow, RenderEffect::OpaqueShadow),
        );

        let white_material = assets.insert(
            Material::new()
                .with_effect(
                    EffectPass::Forward,
                    RenderEffect::Pbr(
                        PbrRenderEffect::new()
                            .with_roughness_factor(1.0)
                            .with_metallic_factor(0.0)
                            .with_albedo(TextureData::srgba(Srgba::from_hsla(1.0, 1.0, 1.0, 1.0))),
                    ),
                )
                .with_effect(EffectPass::Shadow, RenderEffect::OpaqueShadow),
        );

        let body = || {
            let mut builder = Entity::builder();
            builder
                .mount(TransformBundle::default())
                .mount(RigidBodyBundle::new(RigidBodyKind::Dynamic))
                .mount(
                    ColliderBundle::new(SharedShape::cuboid(1.0, 1.0, 1.0))
                        .with_restitution(RESTITUTION)
                        .with_friction(FRICTION),
                )
                .mount(PrimitiveBundle::new(assets.load(&CubePrimitive)))
                .mount(MaterialBundle::new(red_material.clone()));

            builder
        };

        let cube = |pos: Vec3, size: Vec3| {
            let mut builder = body();
            builder.set(ivy_core::components::position(), pos).set(
                collider_builder(),
                ColliderBuilder::cuboid(size.x, size.y, size.z),
            );
            builder
        };

        let sphere = |pos: Vec3, size: f32| {
            let mut builder = body();
            builder
                .set(ivy_core::components::position(), pos)
                .mount(PrimitiveBundle::new(
                    assets.load(&UvSpherePrimitive::default()),
                ))
                .set(collider_builder(), ColliderBuilder::ball(size));
            builder
        };

        let capsule = |pos: Vec3| {
            let mut builder = body();
            builder
                .set(ivy_core::components::position(), pos)
                .mount(PrimitiveBundle::new(
                    assets.load(&CapsulePrimitive::default()),
                ))
                .set(collider_builder(), ColliderBuilder::capsule_y(1.0, 1.0));
            builder
        };

        let ground_size = 25.0;
        cube(Vec3::ZERO, vec3(ground_size, 1.0, ground_size))
            .mount(RigidBodyBundle::new(RigidBodyKind::Fixed))
            .set(scale(), vec3(ground_size, 1.0, ground_size))
            .set(is_static(), ())
            .mount(MaterialBundle::new(white_material))
            .spawn(world);

        let drop_height = 10.0;

        cube(vec3(0.0, drop_height, 0.0), Vec3::ONE)
            .set(rotation(), Quat::from_scaled_axis(vec3(1.0, 1.0, 0.0)))
            .spawn(world);

        cube(vec3(5.0, drop_height, 0.0), Vec3::ONE)
            .set(rotation(), Quat::from_scaled_axis(vec3(1.0, 0.0, 0.0)))
            .spawn(world);

        sphere(vec3(10.0, drop_height, 0.0), 1.0)
            .set(rotation(), Quat::from_scaled_axis(vec3(0.0, 0.0, 0.0)))
            .spawn(world);

        capsule(vec3(-5.0, drop_height, 0.0))
            .set(rotation(), Quat::from_scaled_axis(vec3(0.1, 0.0, 0.0)))
            .spawn(world);

        capsule(vec3(-10.0, drop_height, 0.0))
            .set(rotation(), Quat::from_scaled_axis(vec3(0.0, 0.0, 1.0)))
            .spawn(world);

        for i in 0..4 {
            cube(
                vec3(0.0 + i as f32 * 0.0, 2.0 + i as f32 * 2.0, -8.0),
                Vec3::ONE,
            )
            .set(rotation(), Quat::from_scaled_axis(vec3(0.0, 0.0, 0.0)))
            .spawn(world);
        }

        Entity::builder()
            .mount(TransformBundle::default().with_rotation(Quat::from_euler(
                EulerRot::YXZ,
                -2.0,
                -1.0,
                0.0,
            )))
            .mount(LightBundle {
                params: LightParams::new(Srgb::new(1.0, 1.0, 1.0), 1.0),
                kind: LightKind::Directional,
                cast_shadow: true,
            })
            .spawn(world);

        Ok(())
    }
}
