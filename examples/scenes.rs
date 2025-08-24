use std::{
    any::type_name,
    f32::consts::{PI, TAU},
};

use flax::{components::child_of, Entity, FetchExt, Query, System, World};
use glam::{vec3, EulerRot, Quat, Vec2, Vec3};
use image::Rgba;
use ivy_assets::loadable::Loadable;
use ivy_assets::{stored::DynamicStore, Asset, AssetCache, AssetPath, AsyncAssetExt};
use ivy_core::{
    palette::Srgb,
    profiling::ProfilingLayer,
    transforms::TransformUpdatePlugin,
    update_layer::{FixedTimeStep, Plugin, PluginLayer, ScheduleSetBuilder},
    App, ColorExt, EngineLayer, EntityBuilderExt,
};
use ivy_engine::{
    async_commandbuffer, elapsed_time, engine, is_static, rotation, scale, RigidBodyBundle,
    TransformBundle,
};
use ivy_game::{
    orbit_camera::OrbitCameraPlugin,
    standalone_camera::StandaloneCameraPlugin,
    viewport_camera::{CameraSettings, ViewportCameraLayer},
};
use ivy_gltf::{
    animation::{
        player::{AnimationPlayer, Animator},
        plugin::AnimationPlugin,
        AnimationDesc,
    },
    Document,
};
use ivy_graphics::{
    mesh::MeshData,
    texture::{TextureData, TextureDesc},
};
use ivy_input::layer::InputLayer;
use ivy_physics::{components::collider_builder, ColliderBundle, PhysicsPlugin, RigidBodyKind};
use ivy_postprocessing::preconfigured::{
    pbr::{PbrRenderGraphConfig, SkyboxConfig},
    SurfacePbrPipelineDesc, SurfacePbrRenderer,
};
use ivy_scene::{
    ui::SceneView, viewport_provider::SceneViewportProvider, GltfNodeExt, NodeMountOptions, Scene,
    SceneLayer,
};
use ivy_ui::{
    layer::{UiLayer, UiLayerOptions, UiUpdateLayer},
    screens::{screen_state, Screen},
    streamed::StreamedUiPlugin,
};
use ivy_wgpu::{
    components::{forward_pass, light_kind, light_params, shadow_pass, transparent_pass},
    driver::WinitDriver,
    effect_desc::{
        PbrEmissiveRenderEffectDesc, PbrRenderEffect, PbrRenderEffectDesc, RenderEffect,
        RenderEffectDesc,
    },
    layer::GraphicsLayer,
    light::{LightBundle, LightKind, LightParams},
    material::{EffectPass, Material, MaterialBundle},
    mesh_desc::MeshDesc,
    primitives::{
        generate_plane, CapsulePrimitive, CubePrimitive, PrimitiveBundle, UvSpherePrimitive,
    },
    renderer::{EnvironmentData, MeshBundle, RenderObjectBundle},
};
use rapier3d::prelude::{ColliderBuilder, SharedShape};
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
use violet::{
    core::{
        components::LayoutAlignment,
        layout::Align,
        style::{base_colors::AMBER_400, spacing_medium, text_large, SizeExt},
        widget::{
            bold, card, col, interactive::tooltip::Tooltip, label, maximized, panel, raised_card,
            row, subtitle, Collapsible, LabeledSlider,
        },
        Widget,
    },
    futures_signals::signal::Mutable,
    lucide::icons::LUCIDE_LAYERS_2,
    palette::{rgb::Rgb, Hsl, IntoColor, Srgba},
};
use wgpu::TextureFormat;
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
                .with_title("Ivy"),
        ))
        .with_layer(EngineLayer::new())
        .with_layer(GraphicsLayer::new(
            move |world, assets, store, gpu, surface| {
                Ok(SurfacePbrRenderer::new(
                    world,
                    assets,
                    store,
                    gpu,
                    surface,
                    SurfacePbrPipelineDesc {
                        pbr_config: PbrRenderGraphConfig {
                            label: "basic".into(),
                            skybox: Some(SkyboxConfig {
                                hdri: Box::new(AssetPath::new(
                                    "hdris/kloofendal_48d_parly_cloudy_puresky_2k.hdr",
                                )),
                                format: TextureFormat::Rgba16Float,
                            }),
                            ..Default::default()
                        },
                    },
                ))
            },
        ))
        .with_layer(ProfilingLayer::new())
        .with_layer(UiLayer::new())
        .with_layer(InputLayer::new())
        .with_layer(
            PluginLayer::new(FixedTimeStep::new(0.02))
                .with_plugin(SceneUiPlugin)
                .with_plugin(StandaloneCameraPlugin),
        )
        .with_layer(
            SceneLayer::new().with_scene(
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
                    .with_layer(ViewportCameraLayer::new(CameraSettings {
                        environment_data: EnvironmentData::new(
                            Srgb::new(0.2, 0.2, 0.3),
                            0.001,
                            if ENABLE_SKYBOX { 0.0 } else { 1.0 },
                        ),
                        fov: 1.0,
                    }))
                    .with_layer(
                        PluginLayer::new(FixedTimeStep::new(0.02))
                            .with_plugin(StreamedUiPlugin)
                            .with_plugin(SetupPlugin)
                            .with_plugin(GameUiPlugin)
                            .with_plugin(OrbitCameraPlugin)
                            .with_plugin(AnimationPlugin)
                            .with_plugin(PhysicsPlugin::new())
                            .with_plugin(TransformUpdatePlugin),
                    )
                    .with_layer(UiUpdateLayer::new()),
            ),
        )
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

struct SceneUiPlugin;

impl Plugin for SceneUiPlugin {
    fn install(
        &self,
        world: &mut World,
        _: &AssetCache,
        _: &mut DynamicStore,
        _: &mut ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        world.get(engine(), screen_state())?.open(MainUI);

        Ok(())
    }
}

struct GameUiPlugin;

impl Plugin for GameUiPlugin {
    fn install(
        &self,
        world: &mut World,
        _: &AssetCache,
        _: &mut DynamicStore,
        _: &mut ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        tracing::info!("Opening game UI");
        world.get(engine(), screen_state())?.open(GameUI);

        Ok(())
    }

    fn after(&self) -> Vec<&str> {
        vec![type_name::<StreamedUiPlugin>()]
    }
}

struct MainUI;

impl Screen for MainUI {
    fn create(self, scope: &mut violet::core::Scope<'_>, _: ivy_ui::screens::ScreenLifetimeToken) {
        maximized(panel(col((
            raised_card(
                row((
                    Tooltip::label(
                        bold(LUCIDE_LAYERS_2)
                            .with_font_size(text_large())
                            .with_color(AMBER_400)
                            .with_margin(spacing_medium()),
                        "An icon of a stack of layers, alluding to represents the concept of subscenes",
                    ),
                    col((
                        subtitle("Scenes"),
                        label("Allows nesting and decoupling multiple worlds from the core engine"),
                    )),
                ))
                .with_maximize(Vec2::X)
                .with_cross_align(Align::Center),
            ),
            SceneView::new(),
        ))))
        .mount(scope);
    }
}

struct GameUI;

impl Screen for GameUI {
    fn create(self, scope: &mut violet::core::Scope<'_>, _: ivy_ui::screens::ScreenLifetimeToken) {
        maximized((
            card(Collapsible::label(
                "Game",
                col((LabeledSlider::new(Mutable::new(50), 0, 100),)),
            )),
            card(label("Scene"))
                .with_maximize(Vec2::X)
                .with_item_align(LayoutAlignment::bottom_left()),
        ))
        .mount(scope);
    }
}

struct SetupPlugin;

impl Plugin for SetupPlugin {
    fn install(
        &self,
        world: &mut World,
        assets: &AssetCache,
        _: &mut DynamicStore,
        _: &mut ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        Self::setup_objects(world, assets)
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
