use std::{any::type_name, cell::RefCell, sync::Arc, time::Duration};

use async_std::task::sleep;
use flax::{Entity, EntityIds, World};
use futures::channel::oneshot;
use glam::Vec2;
use ivy_assets::AssetCache;
use ivy_core::{components::engine, update_layer::Plugin};
use ivy_scene::{
    OpenSceneCommand, SceneBuilder, SceneCommand, scene_commands,
    viewport_provider::scene_viewport_state,
};
use ivy_ui::{
    screens::{Screen, screen_state},
    streamed::StreamedUiPlugin,
    violet::{
        core::{
            Edges, Widget,
            layout::Align,
            style::{SizeExt, element_accent, surface_primary, surface_secondary},
            to_owned,
            widget::{
                Button, FutureWidget, StreamWidget, bold, card, col, maximized, panel, raised_card,
                row, subtitle,
            },
        },
        futures_signals::signal::{Mutable, SignalExt},
        lucide::icons::{LUCIDE_FOLDER, LUCIDE_LEAF, LUCIDE_PACKAGE, LUCIDE_SATELLITE_DISH},
    },
};

use crate::ui::browser::{DetailsPanel, DirectoryBrowser, DirectoryBrowserState};

pub struct EditorHost {
    scene_commands: flume::Sender<ivy_scene::SceneCommand>,
    state: Mutable<EditorState>,
}

pub struct EditorState {
    current_scene: Option<Entity>,
}

impl EditorHost {
    pub fn new(
        scene_commands: flume::Sender<ivy_scene::SceneCommand>,
        state: Mutable<EditorState>,
    ) -> Self {
        Self {
            scene_commands,
            state,
        }
    }

    pub fn open_scene(
        &mut self,
        scene: impl 'static + Send + FnOnce() -> SceneBuilder,
    ) -> anyhow::Result<()> {
        let (on_ready_tx, on_ready_rx) = oneshot::channel();

        let scene_builder = || scene();

        let _ =
            self.scene_commands
                .send(ivy_scene::SceneCommand::OpenScene(OpenSceneCommand::new(
                    scene_builder,
                    Some(on_ready_tx),
                )));

        to_owned!(state = self.state);
        async_std::task::spawn(async move {
            sleep(Duration::from_millis(500)).await;
            let scene_id = on_ready_rx.await.expect("Scene created");
            tracing::info!(?scene_id, "editor: opened scene");
            state.lock_mut().current_scene = Some(scene_id);
        });

        Ok(())
    }
}

/// Plugin for the outer engine to manage scene state and cross-scene editor functionality
pub struct EditorHostPlugin {
    scene: Option<Arc<dyn Send + Sync + 'static + Fn() -> SceneBuilder>>,
}

impl EditorHostPlugin {
    pub fn new() -> Self {
        Self { scene: None }
    }

    pub fn with_scene(mut self, scene: impl 'static + Send + Sync + Fn() -> SceneBuilder) -> Self {
        self.scene = Some(Arc::new(scene));
        self
    }
}

impl Plugin for EditorHostPlugin {
    fn install(
        // TODO: mut or maybe config?
        &self,
        world: &mut World,
        // TODO: collect into struct
        assets: &ivy_assets::AssetCache,
        _: &mut ivy_assets::stored::DynamicStore,
        _schedules: &mut ivy_core::update_layer::ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        let scene_commands = world.get(engine(), scene_commands())?;
        let screens = world.get_mut(engine(), screen_state())?;

        let editor_state = Mutable::new(EditorState {
            current_scene: None,
        });

        let mut editor_host = EditorHost::new(scene_commands.clone(), editor_state.clone());

        // Open initial scene
        if let Some(scene) = self.scene.clone() {
            // TODO: maybe just "with world" and provided base scene builder?
            editor_host.open_scene(move || scene())?;
        }

        screens.open(MainEditorUI {
            editor_state,
            assets: assets.clone(),
        });

        Ok(())
    }

    fn after(&self) -> Vec<&str> {
        vec![type_name::<StreamedUiPlugin>()]
    }
}

struct MainEditorUI {
    editor_state: Mutable<EditorState>,
    assets: AssetCache,
}

impl Screen for MainEditorUI {
    fn create(
        self,
        scope: &mut ivy_ui::violet::core::Scope<'_>,
        token: ivy_ui::screens::ScreenLifetimeToken,
    ) {
        let main_viewport = maximized(StreamWidget(
            self.editor_state
                .signal_ref(|v| {
                    v.current_scene
                        .map(|id| Box::new(SceneView { scene_id: id }) as Box<dyn Widget>)
                        .unwrap_or(Box::new(panel(bold("No Scene Open"))))
                })
                .to_stream(),
        ));

        let browser_state = DirectoryBrowserState::new();
        let directory_browser = window(
            LUCIDE_PACKAGE,
            "Assets",
            DirectoryBrowser::new(self.assets.clone(), "./assets", browser_state.clone()),
        );

        let details_panel = window(
            LUCIDE_SATELLITE_DISH,
            "Inspector",
            DetailsPanel::new(self.assets, browser_state),
        );

        panel(
            col((
                header(),
                row((col((main_viewport, directory_browser)), details_panel)),
            ))
            .with_contain_margins(true),
        )
        .with_background(surface_primary())
        .with_maximize(Vec2::ONE)
        .mount(scope);
    }
}

// TODO: to main Ui component
pub fn window_header(icon: impl Into<String>, title: impl Into<String>) -> impl Widget {
    raised_card(
        row((subtitle(icon.into()), subtitle(title.into())))
            .with_cross_align(Align::Center)
            .with_maximize(Vec2::X),
    )
}

fn window(icon: impl Into<String>, title: impl Into<String>, content: impl Widget) -> impl Widget {
    col((window_header(icon, title), card(content))).with_background(surface_secondary())
}

fn header() -> impl Widget {
    raised_card(
        row((
            subtitle(LUCIDE_LEAF).with_color(element_accent()),
            subtitle("Editor"),
            Button::label("Save").disabled(),
            Button::label("Load").disabled(),
            Button::label("Menu").disabled(),
        ))
        .with_cross_align(Align::Center)
        .with_maximize(Vec2::X),
    )
}

struct SceneView {
    scene_id: Entity,
}

impl Widget for SceneView {
    fn mount(self, scope: &mut ivy_ui::violet::core::Scope<'_>) {
        let viewports = scope.get_context_cloned(scene_viewport_state());

        let tx = viewports.open_viewport(self.scene_id);

        FutureWidget::new(async move {
            let scene = tx.await;

            match scene {
                Ok(scene) => Box::new(ivy_scene::ui::SceneViewport::new(scene)) as Box<dyn Widget>,
                Err(e) => {
                    tracing::error!("Failed to open scene viewport: {e:?}");
                    Box::new(panel(bold(format!("Failed to open scene: {e:?}"))))
                }
            }
        })
        .mount(scope);
    }
}
