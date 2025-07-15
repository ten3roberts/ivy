use async_std::stream::StreamExt;
use flax::World;
use ivy_assets::{stored::DynamicStore, AssetCache, AssetPath};
use ivy_core::{
    palette::Srgb,
    profiling::ProfilingLayer,
    update_layer::{FixedTimeStep, Plugin, ScheduleSetBuilder, ScheduledLayer},
    App, EngineLayer,
};
use ivy_editable::Editable;
use ivy_engine::{engine, TransformBundleDesc};
use ivy_game::{
    orbit_camera::OrbitCameraPlugin,
    viewport_camera::{CameraSettings, ViewportCameraLayer},
};
use ivy_input::layer::InputLayer;
use ivy_postprocessing::preconfigured::{
    pbr::{PbrRenderGraphConfig, SkyboxConfig},
    SurfacePbrPipelineDesc, SurfacePbrRenderer,
};
use ivy_ui::{
    layer::{UiLayer, UiUpdateLayer},
    screens::{screen_state, Screen},
};
use ivy_wgpu::{driver::WinitDriver, layer::GraphicsLayer, renderer::EnvironmentData};
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
use violet::{
    core::{
        state::{StateExt, StateStream},
        style::SizeExt,
        unit::Unit,
        widget::{bold, card, col, label, maximized, StreamWidget},
        Widget,
    },
    futures_signals::signal::Mutable,
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
    Variant1 { name: String, value: i32 },
    Variant2 { value: f32 },
}
impl ivy_editable::Editable for MyEnum {
    const INLINE: bool = false;
    fn create_editor<
        S: 'static
            + Send
            + Sync
            + ivy_editable::__private::violet::core::state::StateDuplex<Item = Self>,
    >(
        state: S,
    ) -> Box<dyn Send + ivy_editable::__private::violet::core::widget::Widget> {
        use ::std::sync::Arc;
        use ivy_editable::__private::violet::core::state::StateExt;
        use ivy_editable::__private::violet::core::style::SizeExt;
        use ivy_editable::__private::violet::core::widget::{
            col, label, row, Selectable, StreamWidget, Widget,
        };
        let state = ::std::sync::Arc::new(state);
        let discriminant = Arc::new(
            state
                .clone()
                .filter_map(
                    |v| {
                        Some(Some(match v {
                            Self::Variant1 { .. } => "Variant1",
                            Self::Variant2 { .. } => "Variant2",
                        }))
                    },
                    |_| None,
                )
                .memo(None)
                .dedup()
                .lower_option(),
        );
        let kind_selection = row((
            Selectable::new_value(label("Variant1"), discriminant.clone(), "Variant1"),
            Selectable::new_value(label("Variant2"), discriminant.clone(), "Variant2"),
        ));
        let value_editor = discriminant
            .stream()
            .map(move |disc| {
                match disc {
                    "Variant1" => {
                        let state = Arc::new(
                            state
                                .clone()
                                .filter_map(
                                    |v| {
                                        if let Self::Variant1 { .. } = v {
                                            Some(Some((name, value)))
                                        } else {
                                            None
                                        }
                                    },
                                    |(name, value)| Some(Self::Variant1 {}),
                                )
                                .memo((None, None)),
                        );
                        let name = state
                            .clone()
                            .project_ref(|v| &v.0, |v| &mut v.0)
                            .lower_option();
                        let value = state
                            .clone()
                            .project_ref(|v| &v.1, |v| &mut v.1)
                            .lower_option();
                        Box::new(
                            ivy_editable::__private::violet::core::widget::col((
                                |
                                    scope: &mut ivy_editable::__private::violet::core::Scope<
                                        '_,
                                    >|
                                {
                                    if <String as ivy_editable::Editable>::INLINE {
                                        ivy_editable::__private::violet::core::widget::row((
                                                ivy_editable::__private::violet::core::widget::Stack::new(
                                                        ivy_editable::__private::violet::core::widget::interactive::base::InteractiveWidget::new(
                                                                ivy_editable::__private::violet::core::widget::label("name"),
                                                            )
                                                            .with_tooltip_text("String"),
                                                    )
                                                    .with_maximize(
                                                        ivy_editable::__private::violet::glam::Vec2::X,
                                                    ),
                                                <String as ivy_editable::Editable>::create_editor(name),
                                            ))
                                            .with_cross_align(
                                                ivy_editable::__private::violet::core::layout::Align::Center,
                                            )
                                            .mount(scope);
                                    } else {
                                        ivy_editable::__private::violet::core::widget::Collapsible::new(
                                                ivy_editable::__private::violet::core::widget::interactive::base::InteractiveWidget::new(
                                                        ivy_editable::__private::violet::core::widget::label("name"),
                                                    )
                                                    .with_tooltip_text("String"),
                                                <String as ivy_editable::Editable>::create_editor(name),
                                            )
                                            .indent(true)
                                            .mount(scope);
                                    }
                                },
                                |
                                    scope: &mut ivy_editable::__private::violet::core::Scope<
                                        '_,
                                    >|
                                {
                                    if <i32 as ivy_editable::Editable>::INLINE {
                                        ivy_editable::__private::violet::core::widget::row((
                                                ivy_editable::__private::violet::core::widget::Stack::new(
                                                        ivy_editable::__private::violet::core::widget::interactive::base::InteractiveWidget::new(
                                                                ivy_editable::__private::violet::core::widget::label(
                                                                    "value",
                                                                ),
                                                            )
                                                            .with_tooltip_text("i32"),
                                                    )
                                                    .with_maximize(
                                                        ivy_editable::__private::violet::glam::Vec2::X,
                                                    ),
                                                <i32 as ivy_editable::Editable>::create_editor(value),
                                            ))
                                            .with_cross_align(
                                                ivy_editable::__private::violet::core::layout::Align::Center,
                                            )
                                            .mount(scope);
                                    } else {
                                        ivy_editable::__private::violet::core::widget::Collapsible::new(
                                                ivy_editable::__private::violet::core::widget::interactive::base::InteractiveWidget::new(
                                                        ivy_editable::__private::violet::core::widget::label(
                                                            "value",
                                                        ),
                                                    )
                                                    .with_tooltip_text("i32"),
                                                <i32 as ivy_editable::Editable>::create_editor(value),
                                            )
                                            .indent(true)
                                            .mount(scope);
                                    }
                                },
                            )),
                        ) as Box<dyn Send + Widget>
                    }
                    "Variant2" => {
                        let state = Arc::new(
                            state
                                .clone()
                                .filter_map(
                                    |v| {
                                        if let Self::Variant2 { .. } = v {
                                            Some(Some((value,)))
                                        } else {
                                            None
                                        }
                                    },
                                    |(value,)| Some(Self::Variant2 {}),
                                )
                                .memo((None,)),
                        );
                        let value = state
                            .clone()
                            .project_ref(|v| &v.0, |v| &mut v.0)
                            .lower_option();
                        Box::new(
                            ivy_editable::__private::violet::core::widget::col(
                                (|
                                    scope: &mut ivy_editable::__private::violet::core::Scope<
                                        '_,
                                    >|
                                {
                                    if <f32 as ivy_editable::Editable>::INLINE {
                                        ivy_editable::__private::violet::core::widget::row((
                                                ivy_editable::__private::violet::core::widget::Stack::new(
                                                        ivy_editable::__private::violet::core::widget::interactive::base::InteractiveWidget::new(
                                                                ivy_editable::__private::violet::core::widget::label(
                                                                    "value",
                                                                ),
                                                            )
                                                            .with_tooltip_text("f32"),
                                                    )
                                                    .with_maximize(
                                                        ivy_editable::__private::violet::glam::Vec2::X,
                                                    ),
                                                <f32 as ivy_editable::Editable>::create_editor(value),
                                            ))
                                            .with_cross_align(
                                                ivy_editable::__private::violet::core::layout::Align::Center,
                                            )
                                            .mount(scope);
                                    } else {
                                        ivy_editable::__private::violet::core::widget::Collapsible::new(
                                                ivy_editable::__private::violet::core::widget::interactive::base::InteractiveWidget::new(
                                                        ivy_editable::__private::violet::core::widget::label(
                                                            "value",
                                                        ),
                                                    )
                                                    .with_tooltip_text("f32"),
                                                <f32 as ivy_editable::Editable>::create_editor(value),
                                            )
                                            .indent(true)
                                            .mount(scope);
                                    }
                                }),
                            ),
                        ) as Box<dyn Send + Widget>
                    }
                    _ => {
                        ::core::panicking::panic(
                            "internal error: entered unreachable code",
                        )
                    }
                }
            });
        Box::new(col((kind_selection, StreamWidget::new(value_editor))))
    }
    fn create_editor_project<
        S: 'static
            + Send
            + Sync
            + Clone
            + ivy_editable::__private::violet::core::state::StateStreamRef<Item = Self>
            + ivy_editable::__private::violet::core::state::StateWrite<Item = Self>,
    >(
        state: S,
    ) -> Box<dyn Send + ivy_editable::__private::violet::core::widget::Widget> {
        use ivy_editable::__private::violet::core::state::StateExt;
        use ivy_editable::__private::violet::core::state::StateStream;
        use ivy_editable::__private::violet::core::style::SizeExt;
        use ivy_editable::__private::violet::core::widget::Widget;
        ::core::panicking::panic("not yet implemented")
    }
}

impl Screen for MainUI {
    fn create(self, scope: &mut violet::core::Scope<'_>, _: ivy_ui::screens::ScreenLifetimeToken) {
        let value = Mutable::new(Some(MyEnum::Variant1 {
            name: "Variant".to_string(),
            value: 100,
        }));

        maximized(
            col((
                card(MyEnum::create_editor(value.clone().lower_option())),
                card(StreamWidget::new(value.stream().map(|v| {
                    v.map(|v| label(format!("{v:#?}")))
                        .unwrap_or(bold("No Value"))
                }))),
            ))
            .with_max_size(Unit::px2(400.0, 600.0)),
        )
        .mount(scope);
    }
}
