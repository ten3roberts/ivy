use std::f32::consts::{PI, TAU};

use anyhow::Context;
use bevy_reflect::{DynamicTyped, PartialReflect, Reflect, Typed};
use flax::{
    components::child_of, BoxedSystem, Component, Entity, FetchExt, Query, QueryBorrow, System,
    World,
};
use glam::{vec3, EulerRot, Mat4, Quat, Vec3};
use image::Rgba;
use itertools::{Either, Itertools};
use ivy_assets::{
    fs::AssetPath, loadable::ResourceDesc, stored::DynamicStore, Asset, AssetCache, AsyncAssetExt,
};
use ivy_core::{
    app::PostInitEvent,
    gizmos,
    layer::events::EventRegisterContext,
    math::Vec3Ext,
    palette::{Srgb, WithAlpha},
    profiling::ProfilingLayer,
    transforms::TransformUpdatePlugin,
    update_layer::{FixedTimeStep, Plugin, ScheduleSetBuilder, ScheduledLayer},
    App, AsyncCommandBuffer, EngineLayer, EntityBuilderExt, Layer,
};
use ivy_engine::{
    async_commandbuffer, elapsed_time, engine, rotation, world_transform, RigidBodyBundle,
    TransformBundle,
};
use ivy_game::{
    debug::AssetTimelinesWidget,
    orbit_camera::OrbitCameraPlugin,
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
use ivy_graphics::texture::{ColorChannel, MetallicRoughnessProcessor, TextureData, TextureDesc};
use ivy_input::layer::InputLayer;
use ivy_physics::{ColliderBundle, GizmoSettings, PhysicsPlugin};
use ivy_postprocessing::preconfigured::{
    pbr::{PbrRenderGraphConfig, SkyboxConfig},
    SurfacePbrPipelineDesc, SurfacePbrRenderer,
};
use ivy_scene::{
    editor::editable::{edit_reflect, Project},
    GltfNodeExt, NodeMountOptions,
};
use ivy_ui::{
    layer::{UiLayer, UiUpdateLayer},
    screens::{screen_state, Screen},
};
use ivy_wgpu::{
    components::{forward_pass, light_kind, light_params, shadow_pass, transparent_pass},
    driver::WinitDriver,
    layer::GraphicsLayer,
    light::{LightBundle, LightKind, LightParams},
    material_desc::{
        MaterialData, MaterialDesc, PbrEmissiveMaterialDesc, PbrMaterialData, PbrMaterialDesc,
    },
    mesh_desc::MeshDesc,
    primitives::{generate_plane, UvSpherePrimitive},
    renderer::{EnvironmentData, RenderObjectBundle},
};
use rapier3d::prelude::SharedShape;
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
use violet::{
    core::{
        style::SizeExt,
        unit::Unit,
        widget::{card, maximized},
        Widget,
    },
    futures_signals::signal::Mutable,
    palette::{rgb::Rgb, Hsl, IntoColor},
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

    let ui_input_layer = UiLayer::new();
    let ui_layer = UiUpdateLayer::new();

    if let Err(err) = App::builder()
        .with_driver(WinitDriver::new(
            WindowAttributes::default()
                .with_inner_size(LogicalSize::new(1920, 1080))
                .with_title("Ivy"),
        ))
        .with_layer(EngineLayer::new())
        .with_layer(ProfilingLayer::new())
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
                                    "hdris/kloofendal_48d_partly_cloudy_puresky_2k.hdr",
                                )),
                                format: TextureFormat::Rgba16Float,
                            }),
                            ..Default::default()
                        },
                    },
                ))
            },
        ))
        .with_layer(ui_input_layer)
        .with_layer(InputLayer::new())
        .with_layer(
            ScheduledLayer::new(FixedTimeStep::new(0.02))
                .with_plugin(GameUiPlugin)
                .with_plugin(OrbitCameraPlugin),
        )
        .with_layer(ViewportCameraLayer::new(CameraSettings {
            environment_data: EnvironmentData::new(
                Srgb::new(0.2, 0.2, 0.3),
                0.001,
                if ENABLE_SKYBOX { 0.0 } else { 1.0 },
            ),
            fov: 1.0,
        }))
        .with_layer(ui_layer)
        .run()
    {
        tracing::error!("{err:?}");
        Err(err)
    } else {
        Ok(())
    }
}

struct GameUiPlugin;

impl Plugin for GameUiPlugin {
    fn install(
        &self,
        world: &mut World,
        assets: &AssetCache,
        _: &mut DynamicStore,
        _: &mut ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        world.get(engine(), screen_state())?.open(MainUI {
            assets: assets.clone(),
        });

        Ok(())
    }
}

struct MainUI {
    assets: AssetCache,
}

#[derive(Reflect)]
struct ExampleStruct {
    a: i32,
    name: String,
    inner: InnerStruct,
}

#[derive(Reflect)]
struct InnerStruct {
    position: Vec3,
    rotation: Quat,
    values: Vec<f32>,
}

impl Screen for MainUI {
    fn create(self, scope: &mut violet::core::Scope<'_>, _: ivy_ui::screens::ScreenLifetimeToken) {
        let value = Mutable::new(Box::new(ExampleStruct {
            a: 42,
            name: "Example".to_string(),
            inner: InnerStruct {
                position: vec3(0.5, -7.8, 3.7),
                rotation: Quat::from_euler(EulerRot::YXZ, -PI / 4.0, -PI / 6.0, 0.0),
                values: vec![1.0, 5.7, 3.14, 2.718],
            },
        }) as Box<dyn PartialReflect>);

        maximized(card(edit_reflect(
            ExampleStruct::type_info(),
            Project::new(value.clone()),
        )))
        .mount(scope);
    }
}
