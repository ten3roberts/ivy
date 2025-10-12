use async_std::stream::StreamExt;
use flax::World;
use glam::Quat;
use ivy_assets::{stored::DynamicStore, AssetCache, AssetPath};
use ivy_core::{
    palette::Srgb,
    profiling::ProfilingLayer,
    update_layer::{FixedTimeStep, Plugin, PluginLayer, ScheduleSetBuilder},
    App, EngineLayer,
};
use ivy_editable::Editable;
use ivy_engine::engine;
use ivy_game::orbit_camera::OrbitCameraPlugin;
use ivy_input::layer::InputLayer;
use ivy_postprocessing::preconfigured::{
    pbr::{PbrRenderGraphConfig, SkyboxConfig},
    SurfacePbrPipelineDesc, SurfacePbrRenderer,
};
use ivy_ui::{
    layer::{UiLayer, UiUpdateLayer},
    screens::{screen_state, Screen},
};
use ivy_wgpu::{driver::WinitDriver, effect_desc::RenderEffectDesc, layer::GraphicsLayer};
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
use violet::{
    core::{
        state::{StateExt, StateStream},
        style::{base_colors::EMERALD_400, SizeExt},
        unit::Unit,
        widget::{bold, card, col, label, maximized, StreamWidget},
        Widget,
    },
    futures_signals::signal::Mutable,
    palette::WithAlpha,
};
use wgpu::TextureFormat;
use winit::{dpi::LogicalSize, window::WindowAttributes};

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
            PluginLayer::new(FixedTimeStep::new(0.02))
                .with_plugin(GameUiPlugin)
                .with_plugin(OrbitCameraPlugin),
        )
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
        _: &AssetCache,
        _: &mut DynamicStore,
        _: &mut ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        world.get(engine(), screen_state())?.open(MainUI {});

        Ok(())
    }
}

struct MainUI {}

#[derive(Clone, Debug, Editable)]
enum MyEnum {
    Variant1 {
        name: String,
        #[editable(default = 5)]
        value: i32,
    },
    Variant2 {
        #[editable(default = 6.4)]
        #[editable(range(0.0, 10.0))]
        value: f32,
        #[editable(default = EMERALD_400.without_alpha())]
        favorite_color: Srgb,
    },
    Variant3(#[editable(default = "Hello".into())] String),
    Variant4,
}

#[derive(Clone, Debug, Editable)]
struct MyStruct {
    rotation: Quat,
    #[editable(default = "Hello".into())]
    name: String,
    kind: Option<MyEnum>,
    material: Option<RenderEffectDesc>,
}

impl Screen for MainUI {
    fn create(self, scope: &mut violet::core::Scope<'_>, _: ivy_ui::screens::ScreenLifetimeToken) {
        let value = Mutable::new(None);

        maximized(
            col((
                card(MyStruct::create_editor(
                    value.clone().lower_option(),
                    Default::default(),
                )),
                card(StreamWidget::new(value.stream().map(|v| {
                    v.map(|v| label(format!("{v:#?}")))
                        .unwrap_or(bold("No Value"))
                }))),
            ))
            .with_max_size(Unit::px2(600.0, 600.0)),
        )
        .mount(scope);
    }
}
