use std::{any::type_name, cell::RefCell, io::Cursor, path::PathBuf, sync::Arc};

use async_std::stream::StreamExt;
use flax::{Entity, World};
use glam::Vec2;
use ivy_assets::{AssetCache, stored::DynamicStore};
use ivy_core::{
    components::engine,
    update_layer::{Plugin, ScheduleSetBuilder},
};
use ivy_scene::{
    OpenSceneCommand, SceneBuilder, SceneCommand, scene_commands, scene_world,
    ser::{SceneData, SceneSerializer},
    viewport_provider::scene_viewport_state,
};
use ivy_ui::{
    screens::{Screen, ScreenLifetimeToken, screen_state},
    streamed::{StreamedUiExt, StreamedUiPlugin},
    toast::{Toast, toasts},
    violet::{
        core::{
            Scope, ScopeRef, StateExt, StateStream, Widget,
            layout::Align,
            style::{SizeExt, element_accent, element_tertiary, surface_primary},
            to_owned,
            widget::{
                Button, EmptyWidget, FutureWidget, Stack, StreamWidget, bold, col,
                interactive::Tooltip, label, maximized, panel, raised_card, row, subtitle,
            },
        },
        futures_signals::signal::{Mutable, SignalExt},
        lucide::icons::{LUCIDE_FOLDER, LUCIDE_LEAF, LUCIDE_PACKAGE, LUCIDE_SAVE},
    },
};
use rfd::{AsyncFileDialog, FileHandle};

use crate::ui::browser::{AspectInspectorPanel, AssetBrowser, DirectoryBrowserState, window};

pub struct EditorHost {
    scene_commands: flume::Sender<ivy_scene::SceneCommand>,
    state: EditorState,
}

pub struct SceneState {
    scene_id: Entity,
}

pub type SceneConstructor = Arc<dyn Fn() -> SceneBuilder + Send + Sync + 'static>;

#[derive(Clone)]
pub struct EditorState {
    current_scene: Mutable<Option<SceneState>>,
    scene_path: Mutable<Option<PathBuf>>,
    scene_commands: flume::Sender<ivy_scene::SceneCommand>,
    scene_constructor: SceneConstructor,
}

impl EditorHost {
    pub fn new(scene_commands: flume::Sender<ivy_scene::SceneCommand>, state: EditorState) -> Self {
        Self {
            scene_commands,
            state,
        }
    }

    pub fn open_scene(
        &mut self,
        scene: impl 'static + Send + FnOnce() -> SceneBuilder,
    ) -> anyhow::Result<()> {
        let scene_builder = || scene();

        to_owned!(state = self.state);
        let _ =
            self.scene_commands
                .send(ivy_scene::SceneCommand::OpenScene(OpenSceneCommand::new(
                    scene_builder,
                    Some(move |scene_id| {
                        state.current_scene.set(Some(SceneState { scene_id }));
                        state.scene_path.set(None);
                    }),
                )));

        Ok(())
    }
}

/// Plugin for the outer engine to manage scene state and cross-scene editor functionality
pub struct EditorHostPlugin {
    scene: RefCell<Option<SceneData>>,
    scene_constructor: SceneConstructor,
}

impl EditorHostPlugin {
    pub fn new(scene_constructor: impl 'static + Send + Sync + Fn() -> SceneBuilder) -> Self {
        Self {
            scene: RefCell::new(None),
            scene_constructor: Arc::new(scene_constructor),
        }
    }

    pub fn with_scene(self, scene: SceneData) -> Self {
        self.scene.replace(Some(scene));
        self
    }
}

impl Plugin for EditorHostPlugin {
    fn install(
        // TODO: mut or maybe config?
        &self,
        world: &mut World,
        // TODO: collect into struct
        assets: &AssetCache,
        _: &mut DynamicStore,
        _schedules: &mut ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        let scene_commands = world.get(engine(), scene_commands())?;
        let screens = world.get_mut(engine(), screen_state())?;

        let editor_state = EditorState {
            current_scene: Mutable::new(None),
            scene_path: Mutable::new(None),
            scene_commands: scene_commands.clone(),
            scene_constructor: self.scene_constructor.clone(),
        };

        // TODO: maybe extract to command pattern
        let mut editor_host = EditorHost::new(scene_commands.clone(), editor_state.clone());

        // Open initial scene
        if let Some(scene) = self.scene.take() {
            // TODO: maybe just "with world" and provided base scene builder?
            editor_host.open_scene({
                to_owned!(scene_ctor = editor_host.state.scene_constructor);
                move || (scene_ctor()).with_world(scene.world)
            })?;
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
    editor_state: EditorState,
    assets: AssetCache,
}

impl Screen for MainEditorUI {
    fn create(self, scope: &mut Scope<'_>, _: ScreenLifetimeToken) {
        let main_viewport = maximized(StreamWidget(
            self.editor_state
                .current_scene
                .signal_ref(|v| {
                    v.as_ref()
                        .map(|v| {
                            Box::new(SceneView {
                                scene_id: v.scene_id,
                            }) as Box<dyn Widget>
                        })
                        .unwrap_or(Box::new(panel(bold("No Scene Open"))))
                })
                .to_stream(),
        ));

        let browser_state = DirectoryBrowserState::new("./assets");
        let directory_browser = window(
            LUCIDE_PACKAGE,
            "Assets",
            EmptyWidget,
            AssetBrowser::new(self.assets.clone(), browser_state.clone()),
        );

        let details_panel = AspectInspectorPanel::new(self.assets.clone(), browser_state);

        Stack::new(
            col((
                header(self.assets.clone(), self.editor_state),
                row((col((main_viewport, directory_browser)), details_panel)),
            ))
            .with_contain_margins(true),
        )
        .with_background(surface_primary())
        .with_maximize(Vec2::ONE)
        .mount(scope);
    }
}

fn local_dir() -> PathBuf {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::current_dir().unwrap()
    }
    #[cfg(target_arch = "wasm32")]
    {
        PathBuf::from(".")
    }
}

fn save_scene(
    world: &World,
    save_file: Option<PathBuf>,
) -> impl 'static + Future<Output = anyhow::Result<PathBuf>> {
    let mut data = Vec::new();
    let result = SceneSerializer::new().serialize_json(world, &mut data);
    let scene_dir = local_dir();

    async move {
        result?;

        let file = match save_file {
            Some(path) => FileHandle::from(path),
            None => {
                let file = AsyncFileDialog::new()
                    .set_title("Ivy Scene")
                    .set_directory(&scene_dir)
                    .set_file_name("scene.ivsc")
                    .save_file()
                    .await;

                let Some(file) = file else {
                    anyhow::bail!("No file selected");
                };

                file
            }
        };

        file.write(&data).await?;

        Ok(file
            .path()
            .strip_prefix(scene_dir)
            .unwrap_or_else(|_| file.path())
            .to_path_buf())
    }
}

fn load_scene(
    assets: AssetCache,
) -> impl Future<Output = anyhow::Result<Option<(PathBuf, SceneData)>>> {
    let local_dir = local_dir();
    async move {
        let file = AsyncFileDialog::new()
            .add_filter("Ivy Scene", &["ivsc"])
            .set_directory(&local_dir)
            .set_title("Open Ivy Scene")
            .pick_file()
            .await;

        if let Some(file) = file {
            let data = file.read().await;
            let scene = SceneSerializer::new()
                .load_scene(&assets, &mut Cursor::new(data))
                .await?;

            Ok(Some((
                file.path()
                    .strip_prefix(local_dir)
                    .unwrap_or_else(|_| file.path())
                    .to_path_buf(),
                scene,
            )))
        } else {
            Ok(None)
        }
    }
}

fn header(assets: AssetCache, state: EditorState) -> impl Widget {
    let save_scene = {
        to_owned!(state);
        move |scope: &ScopeRef| {
            let scene_id = state.current_scene.lock_ref().as_ref().map(|v| v.scene_id);
            let Some(scene_id) = scene_id else {
                return;
            };

            let toasts = scope.get_atom_cloned(toasts()).unwrap();

            scope.apply({
                to_owned!(state);
                move |engine_world| {
                    let scene = engine_world.get(scene_id, scene_world()).unwrap();
                    let result =
                        save_scene(&*scene, state.scene_path.get_cloned().map(|v| v.into()));

                    async_std::task::spawn(async move {
                        match result.await {
                            Ok(path) => {
                                toasts.send(Toast::info(
                                    "Scene",
                                    format!("Scene saved to {}", path.display()),
                                ));

                                state.scene_path.set(Some(path));
                            }
                            Err(e) => {
                                toasts.send(Toast::error(
                                    "Scene",
                                    format!("Failed to save scene: {e:?}"),
                                ));
                                tracing::error!("Failed to save scene: {e:?}");
                            }
                        }
                    });

                    Ok(())
                }
            });
        }
    };

    let load_scene = {
        to_owned!(state);
        move |scope: &ScopeRef| {
            let toasts = scope.get_atom_cloned(toasts()).unwrap();
            let assets = assets.clone();

            to_owned!(state);

            let result = load_scene(assets);

            async_std::task::spawn({
                to_owned!(state);
                async move {
                    match result.await {
                        Ok(Some((path, scene_data))) => {
                            to_owned!(scene_ctor = state.scene_constructor);
                            let scene_builder = move || (scene_ctor()).with_world(scene_data.world);

                            let _ = state.scene_commands.send(SceneCommand::OpenScene(
                                OpenSceneCommand::new(
                                    scene_builder,
                                    Some(move |scene_id: Entity| {
                                        state.current_scene.set(Some(SceneState { scene_id }));
                                        state.scene_path.set(Some(path));
                                    }),
                                ),
                            ));
                        }
                        Ok(None) => {
                            // User cancelled
                        }
                        Err(e) => {
                            toasts.send(Toast::error(
                                "Scene",
                                format!("Failed to load scene: {e:?}"),
                            ));
                            tracing::error!("Failed to load scene: {e:?}");
                        }
                    }
                }
            });
        }
    };

    let save_controls = row((
        Button::label(LUCIDE_SAVE)
            .with_tooltip_text("Save Scene")
            .on_click(save_scene),
        Button::label(LUCIDE_FOLDER)
            .with_tooltip_text("Open Scene")
            .on_click(load_scene),
        StreamWidget::new(state.scene_path.clone().lower_option().stream().map(|v| {
            Tooltip::label(
                label(v.display().to_string()).with_color(element_tertiary()),
                v.canonicalize().unwrap_or(v).display().to_string(),
            )
        })),
    ))
    .center();

    raised_card(
        row((
            subtitle(LUCIDE_LEAF).with_color(element_accent()),
            subtitle("Editor"),
            save_controls,
        ))
        .with_cross_align(Align::Center)
        .with_maximize(Vec2::X),
    )
}

struct SceneView {
    scene_id: Entity,
}

impl Widget for SceneView {
    fn mount(self, scope: &mut Scope<'_>) {
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
