use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Context;
use flax::{EntityRef, Query};
use futures::{StreamExt, future::ready};
use glam::{BVec2, Vec2};
use itertools::Itertools;
use ivy_assets::{
    AssetCache, AssetPath, Resource,
    loadable::Loadable,
    meta::{AssetMeta, AssetPayload, AssetPayloadUntyped},
};
use ivy_core::{
    components::{async_commandbuffer, engine, main_camera, TransformBundle}, palette::Srgba, template::{Template, TemplateDesc}, AsyncCommandBuffer, EntityBuilderExt
};
use ivy_input::types::MouseButton;
use ivy_physics::{components::physics_state, rapier3d::prelude::QueryFilter};
use ivy_scene::{
    camera::{CameraQuery, screen_to_world_ray},
    drop::world_drop_area,
    scene_world,
};
use ivy_ui::{
    streamed::StreamedUiExt,
    toast::{toasts, Toast},
    violet::{
        core::{
            components::{rect, LayoutAlignment}, layout::Align, state::StateStream, stored::WeakHandle, style::{
                base_colors::*, default_corner_radius, surface_danger, surface_primary, surface_secondary, SizeExt, StyleExt
            }, text::{FontFamily, Wrap}, time::sleep, to_owned, unit::Unit, widget::{
                card, col, interactive::{base::InteractiveWidget, overlay::overlay_state, tooltip::Tooltip}, label, panel, pill, raised_card, row, subtitle, Button, ButtonStyle, Checkbox, Collapsible, Draggable, FutureWidget, Image, IterWidgetCollection, List, LoadingSpinner, Rectangle, ScrollArea, Selectable, SignalWidget, Stack, StreamWidget, SuspenseWidget, Text, TextInput, TextInputStyle, Throbber, WidgetExt
            }, Edges, Scope, ScopeRef, StateExt, StateStreamRef, Widget, WidgetCollection
        },
        futures_signals::signal::Mutable,
        lucide::icons::{
            LUCIDE_BOX, LUCIDE_CLOUD_SUN, LUCIDE_COPY_PLUS, LUCIDE_ECLIPSE, LUCIDE_FILE_ARCHIVE, LUCIDE_FILE_BOX, LUCIDE_FILE_CODE, LUCIDE_FILE_IMAGE, LUCIDE_FILE_JSON, LUCIDE_FILE_QUESTION, LUCIDE_FILE_TEXT, LUCIDE_FILE_WARNING, LUCIDE_FOLDER, LUCIDE_FOLDER_OPEN, LUCIDE_LOCK, LUCIDE_PACKAGE, LUCIDE_SATELLITE, LUCIDE_TRASH_2
        },
    },
};
use ivy_wgpu::material::{Material, MaterialDesc};
use notify::Watcher;

use crate::ui::{
    asset_inspector::AssetEditor,
    context_menu::{ContextMenu, ContextMenuItem, ContextMenuPanel},
};

pub const BROWSER_PANEL_HEIGHT: f32 = 200.0;
pub const INSPECTOR_PANEL_WIDTH: f32 = 600.0;

pub struct DirectoryTree {
    state: DirectoryBrowserState,
    path: PathBuf,
    expand_depth: usize,
}

impl Widget for DirectoryTree {
    fn mount(self, scope: &mut Scope<'_>) {
        let path = self.path;
        let name = path.file_name().unwrap().to_string_lossy().to_string();

        let selection = self.state.active_dir.clone();

        let mut item_count = 0;
        let subdirs = std::fs::read_dir(&path)
            .unwrap()
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let entry_path = entry.path();

                item_count += 1;

                if entry_path.is_dir() {
                    Some(DirectoryTree {
                        path: entry_path,
                        expand_depth: self.expand_depth.saturating_sub(1),
                        state: self.state.clone(),
                    })
                } else {
                    None
                }
            })
        .collect_vec();

        let subdir_count = subdirs.len();
        let subdirs = col(subdirs)
            .with_padding(Edges::new(8.0, 0.0, 0.0, 0.0))
            .with_stretch(true);

        let icon = match item_count {
            0 => LUCIDE_FOLDER_OPEN,
            _ => LUCIDE_FOLDER,
        };

        let header =
            Selectable::new_value(row((label(icon), label(name))), selection, path.clone())
            .with_style(ButtonStyle::hidden().with_align(LayoutAlignment::left_center()))
            .with_maximize(Vec2::X);

        Collapsible::deferred(header, || subdirs)
            .can_collapse(subdir_count > 0)
            .start_collapsed(self.expand_depth == 0)
            .with_name(path.display().to_string())
            .mount(scope);
        }
}

pub struct DirectoryListing {
    assets: AssetCache,
    path: PathBuf,
    state: DirectoryBrowserState,
}

impl Widget for DirectoryListing {
    fn mount(self, scope: &mut Scope<'_>) {
        to_owned!(path = self.path, state = self.state, assets = self.assets);
        let read_dir = move || {
            let items = std::fs::read_dir(&path)
                .unwrap()
                .filter_map(Result::ok)
                .map(|entry| {
                    let entry_path = entry.path();
                    let entry_name = entry_path.file_name().unwrap();
                    let is_dir = entry_path.is_dir();

                    FileItem {
                        is_dir,
                        name: entry_name.to_string_lossy().to_string(),
                        path: entry_path,
                        state: state.clone(),
                        assets: assets.clone(),
                    }
                })
            .sorted_by_key(|item| (!item.is_dir, item.name.clone()));

            let lines = items.chunks(8);
            lines
                .into_iter()
                .map(|line| row(line.collect_vec()))
                .collect_vec()
        };

        let selected_file = scope.store(self.state.selected_file);

        let path = self.path.clone();
        let open_context_menu = {
            move |scope: &ScopeRef, pos| {
                open_context_menu(scope, pos, populate_menu(path.clone(), selected_file))
            }
        };

        let refresh_state = Mutable::new(());

        if let Err(err) = watch_directory(scope, &self.path, refresh_state.clone()) {
            tracing::error!("Failed to watch directory {}: {}", self.path.display(), err);
        }

        InteractiveWidget::new(
            col((
                    Breadcrumbs {
                        path: &self.path,
                        selection: self.state.active_dir.clone(),
                    },
                    ScrollArea::vertical(StreamWidget::new(
                            refresh_state.stream().map(move |()| col(read_dir())),
                    )),
            ))
            .with_stretch(true),
        )
            .on_generic_mouse_input(move |scope, input| {
                if input.state.is_pressed() && input.button == MouseButton::Right {
                    let pos = input.cursor.absolute_pos;
                    open_context_menu(scope, pos);
                    return None;
                }

                Some(input)
            })
        .mount(scope)
    }
}

fn open_context_menu(scope: &ScopeRef, position: Vec2, menu: ContextMenu) {
    scope
        .get_context(overlay_state())
        .open(ContextMenuPanel::new(position, menu));
}

fn watch_directory(
    scope: &mut Scope<'_>,
    path: &Path,
    refresh_state: Mutable<()>,
) -> anyhow::Result<()> {
    let mut watcher = notify::recommended_watcher({
        to_owned!(refresh_state);
        move |event: notify::Result<notify::Event>| {
            let Ok(event) = event else {
                tracing::error!("Failed to receive file system event");
                return;
            };

            match event.kind {
                notify::EventKind::Create(_) | notify::EventKind::Remove(_) => {
                    refresh_state.set(())
                }
                _ => {}
            }
        }
    })?;

    watcher.watch(path, notify::RecursiveMode::NonRecursive)?;
    scope.store(watcher); // drop on detach
    Ok(())
}

pub fn find_next_filename(name: impl AsRef<Path>, dir: &Path) -> PathBuf {
    let name = name.as_ref();
    let base_name = name.file_stem().unwrap_or_default().to_string_lossy();
    let extension = name.extension().unwrap_or_default();
    tracing::info!(
        "Finding next filename for {} {:?} in {}",
        base_name,
        extension,
        dir.display()
    );
    let mut counter = 1;
    let mut new_name = base_name.to_string();
    let mut path = dir.join(&new_name).with_extension(extension);

    while path.exists() {
        new_name = format!("{} ({})", base_name, counter);
        path = dir.join(&new_name).with_extension(extension);
        counter += 1;
    }

    path
}

pub fn populate_item_menu(
    selected_item: WeakHandle<Mutable<Option<PathBuf>>>,
    path: PathBuf,
) -> ContextMenu {
    ContextMenu::new(vec![
        ContextMenuItem::new(label(LUCIDE_TRASH_2), "Delete File", {
            to_owned!(path);
            move |scope| {
                if let Err(err) = std::fs::remove_file(&path) {
                    tracing::error!("Failed to delete file: {}", err);
                } else {
                    scope.read(selected_item).set(None);
                }
                Ok(())
            }
        }),
        ContextMenuItem::new(label(LUCIDE_COPY_PLUS), "Duplicate", {
            to_owned!(path, selected_item);
            move |scope| {
                let dir = path.parent().unwrap_or_else(|| Path::new("."));
                let file_name = path.file_name().unwrap_or_default();
                let new_path = find_next_filename(file_name, dir);

                if let Err(err) = std::fs::copy(&path, &new_path) {
                    tracing::error!("Failed to duplicate file: {}", err);
                } else {
                    scope.read(selected_item).set(Some(new_path));
                }
                Ok(())
            }
        }),
        ])
}

pub fn create_asset<T>(
    dir: &Path,
    new_name: impl AsRef<Path>,
    content: T::Desc,
) -> anyhow::Result<PathBuf>
where
    T: Resource,
    T::Desc: serde::Serialize,
{
    let new_name = new_name.as_ref();
    let new_path = find_next_filename(new_name, &dir);
    let content =
        AssetPayload::new(AssetMeta::new(T::tag_name().into()), content).serialize_json()?;

    std::fs::write(&new_path, content).context(format!(
            "Failed to create asset file at {}",
            new_path.display()
    ))?;
    Ok(new_path)
}

pub fn populate_menu(
    dir: PathBuf,
    selected_item: WeakHandle<Mutable<Option<PathBuf>>>,
) -> ContextMenu {
    ContextMenu::new(vec![
        ContextMenuItem::new(label(LUCIDE_FILE_TEXT), "New File", {
            to_owned!(dir);
            move |scope| {
                let new_path = find_next_filename("New File", &dir);
                std::fs::write(&new_path, "")?;

                scope.read(selected_item).set(Some(new_path));
                // Implement file creation logic here
                Ok(())
            }
        }),
        ContextMenuItem::new(
            FileType::Asset(AssetType::Template).label(),
            "New Template",
            {
                to_owned!(dir);
                move |scope| {
                    let new_path =
                        create_asset::<Template>(&dir, "Template.asset", TemplateDesc::default())?;
                    scope.read(selected_item).set(Some(new_path));
                    Ok(())
                }
            }
        ),
        ContextMenuItem::new(
            FileType::Asset(AssetType::Material).label(),
            "New Material",
            {
                to_owned!(dir);
                move |scope| {
                    let new_path =
                        create_asset::<Material>(&dir, "Material.asset", MaterialDesc::default())?;
                    scope.read(selected_item).set(Some(new_path));
                    Ok(())
                }
            },
        ),
        ContextMenuItem::new(FileType::Directory.label(), "New Asset Folder", {
            to_owned!(dir);
            move |scope| {
                tracing::info!("Creating new asset folder");
                let new_path = find_next_filename("Assets", &dir);
                std::fs::create_dir(&new_path).context(format!(
                        "Failed to create directory at {}",
                        new_path.display()
                ))?;
                scope.read(selected_item).set(Some(new_path));
                Ok(())
            }
        }),
        ])
}

pub const ITEM_SIZE: Unit<Vec2> = Unit::px2(120.0, 100.0);

pub struct FileIcon {
    path: PathBuf,
    filetype: FileType,
}

impl Widget for FileIcon {
    fn mount(self, scope: &mut Scope<'_>) {
        // Load the file type asynchronously
        if self.filetype.is_image() {
            // If the file is an image, we can display it directly
            let image = Image::new(self.path.canonicalize().unwrap().to_owned());
            image
                .with_exact_size(Unit::px2(64.0, 64.0))
                .with_corner_radius(default_corner_radius())
                .mount(scope);
            return;
        }

        label(self.filetype.icon())
            .with_color(self.filetype.color())
            .with_font_size(48.0)
            .mount(scope)
    }
}

pub struct FileItem {
    name: String,
    path: PathBuf,
    assets: AssetCache,
    state: DirectoryBrowserState,
    is_dir: bool,
}

impl Widget for FileItem {
    fn mount(self, scope: &mut Scope<'_>) {
        let is_selected = self.state.selected_file.clone();
        let selected_file = scope.store(is_selected.clone());
        let widget = async move {
            let path = self.path.clone();
            let assets = self.assets.clone();
            let filetype = FileType::from_path(&self.state.root, &path, &assets).await;
            let preview = {
                to_owned!(path, filetype);
                move || FileIcon {
                    path: path.clone(),
                    filetype: filetype.clone(),
                }
            };

            let on_drop = {
                to_owned!(path, state = self.state, filetype);
                move |scope: &ScopeRef, target: Option<(EntityRef, Vec2)>| {
                    if let Some((widget, pos)) = target {
                        let Ok(scene_id) = widget.get_copy(world_drop_area()) else {
                            return;
                        };

                        let screen_pos = pos / widget.get(rect()).unwrap().size();
                        let toasts = scope.get_atom_cloned(toasts()).unwrap();

                        to_owned!(path, assets, filetype, state);
                        scope.apply(move |world| {
                            let world = &mut *world.get_mut(scene_id, scene_world())?;

                            let mut main_camera =
                                Query::new(CameraQuery::new()).with(main_camera());

                            let mut main_camera = main_camera.borrow(world);

                            let main_camera = main_camera.first().context("No main camera")?;

                            let physics = world.get(engine(), physics_state()).unwrap();

                            let ray = screen_to_world_ray(screen_pos, main_camera);
                            let hit = physics.cast_ray(ray, 1000.0, false, QueryFilter::default());

                            match filetype {
                                FileType::Asset(AssetType::Template) => {
                                    if let Some(hit) = hit {
                                        let asset_path: AssetPath<Template> =
                                            AssetPath::from_root(&state.root, &path);

                                        let async_cmd = world
                                            .get_clone(engine(), async_commandbuffer())
                                            .unwrap();

                                        async_std::task::spawn(async move {
                                            let fut = spawn_template(
                                                assets,
                                                ray,
                                                asset_path.clone(),
                                                hit,
                                                async_cmd,
                                            )
                                                .await;

                                            match fut {
                                                Ok(_) => {
                                                    toasts.send(Toast::info(
                                                            "Editor",
                                                            format!("Spawned template {asset_path:?}"),
                                                    ));
                                                }
                                                Err(e) => {
                                                    toasts.send(Toast::error(
                                                            "Editor",
                                                            format!("Failed to spawn template {asset_path:?}\n{e}"),
                                                    ));
                                                }
                                            }
                                        });
                                    } 
                                }
                                _ => {}
                            }

                            Ok(())
                        });
                    }
                }
            };
            Selectable::new_value(
                Draggable::new(
                    Stack::new(
                        col((
                                FileIcon {
                                    path: path.clone(),
                                    filetype: filetype.clone(),
                                },
                                RenamableItem {
                                    path: self.path.clone(),
                                    state: self.state.clone(),
                                },
                        ))
                        .center(),
                    )
                    .with_alignment(LayoutAlignment::center())
                    .with_exact_size(ITEM_SIZE),
                    preview,
                    on_drop,
                ),
                is_selected.dedup(),
                Some(self.path.clone()),
                )
                    .on_double_click({
                        move |_: &ScopeRef| {
                            if self.is_dir {
                                self.state.active_dir.set(path.clone());
                            }
                        }
                    })
            .on_mouse_input({
                to_owned!(path = self.path);
                move |scope, input| {
                    if input.state.is_pressed() && input.button == MouseButton::Right {
                        open_context_menu(
                            scope,
                            input.cursor.absolute_pos,
                            populate_item_menu(selected_file, path.clone()),
                        );
                    }

                    Some(input)
                }
            })
            .with_style(ButtonStyle::hidden())
        };

        FutureWidget::new(widget).mount(scope);
    }
}

async fn spawn_template(
    assets: AssetCache,
    ray: ivy_physics::shapes::Ray,
    asset_path: AssetPath<Template>,
    hit: ivy_physics::state::RaycastHit,
    cmd: AsyncCommandBuffer,
) -> anyhow::Result<()> {
    let template = asset_path.load(&assets).await?;

    tracing::info!("Loded template: {:?}", asset_path);
    template
        .build()
        .mount(TransformBundle::default().with_position(ray.at(hit.intersection.time_of_impact)))
        .spawn_into(&mut *cmd.lock());

    Ok(())
}

struct RenamableItem {
    path: PathBuf,
    state: DirectoryBrowserState,
}

impl Widget for RenamableItem {
    fn mount(mut self, scope: &mut Scope<'_>) {
        let renaming = Mutable::new(false);

        let mut edit_state = None as Option<Mutable<String>>;

        let selected = self.state.selected_file.clone();
        let selected_dir = self.state.active_dir.clone();

        let renaming = scope.store(renaming);
        let controls = scope.read(&renaming).stream().map(move |v| {
            if !v {
                if let Some(edit_state) = edit_state.take() {
                    let new_path = self.path.with_file_name(edit_state.get_cloned().trim());
                    std::fs::rename(&self.path, new_path.clone()).unwrap_or_else(|err| {
                        tracing::error!("Failed to rename file: {}", err);
                    });
                    self.path = new_path;

                    selected.set(Some(self.path.clone()));
                    selected_dir.set(
                        self.path
                        .parent()
                        .unwrap_or_else(|| Path::new("."))
                        .to_owned(),
                    );
                }
            }

            let name = self.path.file_name().unwrap_or_default().to_string_lossy();

            if v {
                Box::new(
                    TextInput::new(
                        edit_state
                        .get_or_insert_with(|| Mutable::new(name.to_string()))
                        .clone(),
                    )
                    .on_focus_lost(move |scope: &ScopeRef| {
                        scope.read(renaming).set(false);
                    })
                    .with_style(TextInputStyle::default().with_font_family(FontFamily::Monospace)),
                ) as Box<dyn Widget>
            } else {
                // Otherwise, show a label
                Box::new(
                    label(name)
                    .with_wrap(Wrap::WordOrGlyph)
                    .with_font_size(12.0),
                )
            }
        });

        InteractiveWidget::new(StreamWidget::new(controls))
            .on_double_click(move |scope| scope.read(renaming).set(true))
            .mount(scope)
    }
}

#[derive(Clone, Debug)]
pub struct DirectoryBrowserState {
    selected_file: Mutable<Option<PathBuf>>,
    active_dir: Mutable<PathBuf>,
    root: PathBuf,
}

impl DirectoryBrowserState {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            selected_file: Mutable::new(None),
            active_dir: Mutable::new(root.clone()),
            root: root,
        }
    }
}

pub struct AspectInspectorPanel {
    assets: AssetCache,
    state: DirectoryBrowserState,
}

impl AspectInspectorPanel {
    pub fn new(assets: AssetCache, state: DirectoryBrowserState) -> Self {
        Self { assets, state }
    }
}

// TODO: to main Ui component
pub fn window_header(
    icon: impl Into<String>,
    title: impl Into<String>,
    controls: impl Widget,
) -> impl Widget {
    raised_card(
        row((
                subtitle(icon.into()),
                subtitle(title.into()),
                Rectangle::new(Srgba::new(0.0, 0.0, 0.0, 0.0)),
                controls,
        )).with_contain_margins(true)
        .with_cross_align(Align::Center)
    )
}

pub fn window(
    icon: impl Into<String>,
    title: impl Into<String>,
    controls: impl Widget,
    content: impl Widget,
) -> List<impl WidgetCollection> {
    col((window_header(icon, title, controls), card(content))).with_stretch(true).with_background(surface_secondary())
}

impl Widget for AspectInspectorPanel {
    fn mount(self, scope: &mut Scope<'_>) {
        let locked = Mutable::new(false);

        let details = 
            self.state
            .selected_file
            .dedup()
            .stream_ref({
                to_owned!(root = self.state.root, assets = self.assets);
                move |selected| {
                    to_owned!(root, assets);
                    selected.as_ref().map(move |v| FileDetailsWidget {
                        assets: assets.clone(),
                        path: v.to_owned(),
                        asset_root: root,
                    })
                }
            })
        .filter_map({to_owned!(locked);
        move |v| {
            if locked.get() {
                ready(None)
            } else {
                ready(Some(v))
            }
        }
        });

        StreamWidget::new(details.filter_map(|v| ready(v)).map(move |details| {
            let lock_widget = Checkbox::with_label(label(LUCIDE_LOCK), locked.clone())
                .with_style(ButtonStyle::default());

            window(
                LUCIDE_SATELLITE,
                "Inspector",
                lock_widget,
                panel(details),
            )
            .with_max_size(Unit::px2(INSPECTOR_PANEL_WIDTH, f32::MAX))
        }))
        .mount(scope);
        }
}

pub struct AssetBrowser {
    assets: AssetCache,
    state: DirectoryBrowserState,
}

impl AssetBrowser {
    pub fn new(assets: AssetCache, state: DirectoryBrowserState) -> Self {
        Self { assets, state }
    }
}

impl Widget for AssetBrowser {
    fn mount(self, scope: &mut Scope<'_>) {
        row((
                card(ScrollArea::vertical(DirectoryTree {
                    path: self.state.root.clone(),
                    state: self.state.clone(),
                    expand_depth: 1,
                }))
                .with_background(surface_primary()),
                SignalWidget::new(self.state.active_dir.signal_ref({
                    to_owned!(state = self.state, assets = self.assets);
                    move |path| DirectoryListing {
                        path: path.clone(),
                        state: state.clone(),
                        assets: assets.clone(),
                    }
                })),
        ))
            .with_min_size(Unit::px2(100.0, BROWSER_PANEL_HEIGHT))
            .with_max_size(Unit::px2(f32::MAX, BROWSER_PANEL_HEIGHT))
            .with_maximize(Vec2::X)
            .mount(scope)
    }
}

pub struct Breadcrumbs<'a> {
    path: &'a Path,
    selection: Mutable<PathBuf>,
}

impl Widget for Breadcrumbs<'_> {
    fn mount(self, scope: &mut Scope<'_>) {
        let mut tail = Vec::new();
        let selection = scope.store(self.selection);
        let items = self.path.components().map(|segment| {
            let segment_str = segment.as_os_str().to_string_lossy().to_string();
            let full_path = tail.iter().chain([&segment_str]).collect::<PathBuf>();

            let widget = InteractiveWidget::new(
                pill(label(&segment_str))
                .with_margin(Edges::even(2.0))
            )
                .on_click(move |scope: &ScopeRef| {
                    scope.read(selection).set(full_path.clone());
                });

            tail.push(segment_str);
            widget
        });

        row(IterWidgetCollection::new(items))
            .with_cross_align(Align::Center)
            .mount(scope);
    }
}

fn bytes_to_human_readable(size: u64) -> String {
    if size < 1024 {
        format!("{size} bytes")
    } else if size < 1024 * 1024 {
        format!("{:.2} KB", size as f64 / 1024.0)
    } else if size < 1024 * 1024 * 1024 {
        format!("{:.2} MB", size as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", size as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Code {
    Json,
    Rust,
    C,
    Cpp,
    Python,
    Js,
    Wgsl,
    Wasm,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AssetType {
    Asset,
    Template,
    Material,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FileType {
    Directory,
    Code(Code),
    Asset(AssetType),
    Text,
    Image,
    Hdri,
    Archive,
    Blend,
    Gltf,
    Other,
    Error,
}

impl FileType {
    async fn from_path(asset_root: &Path, path: &Path, assets: &AssetCache) -> Self {
        if path.is_dir() {
            FileType::Directory
        } else {
            match path.extension().and_then(|s| s.to_str()) {
                Some(ext) => match ext {
                    "asset" => Self::determine_asset_type(asset_root, path, assets)
                        .await
                        .map(FileType::Asset)
                        .unwrap_or(FileType::Error),
                        "txt" | "md" | "markdown" => FileType::Text,
                        // "rs" | "py" | "js" | "ts" | "c" | "cpp" | "h" | "hpp" => FileType::Code,
                        "png" | "jpg" | "jpeg" | "gif" | "webp" => FileType::Image,
                        "hdr" | "exr" => FileType::Hdri,
                        "zip" | "tar" | "gz" | "rar" => FileType::Archive,
                        "blend" => FileType::Blend,
                        "blend1" => FileType::Blend, // Blender backup files
                        "blend2" => FileType::Blend, // Blender backup files
                        "json" | "yaml" | "yml" => FileType::Code(Code::Json),
                        "glb" | "gltf" => FileType::Gltf,
                        "rs" => FileType::Code(Code::Rust),
                        "c" => FileType::Code(Code::C),
                        "cpp" => FileType::Code(Code::Cpp),
                        "py" => FileType::Code(Code::Python),
                        "js" => FileType::Code(Code::Js),
                        "wgsl" => FileType::Code(Code::Wgsl),
                        "wasm" => FileType::Code(Code::Wasm),
                        _ => FileType::Other,
                },
                _ => FileType::Other,
            }
        }
    }

    async fn determine_asset_type(
        asset_root: &Path,
        path: &Path,
        assets: &AssetCache,
    ) -> anyhow::Result<AssetType> {
        let path = AssetPath::from_root(asset_root, path);
        let meta = AssetPayloadUntyped::load_meta_from_file(&path, assets).await?;

        match meta.type_name.as_str() {
            "Template" => Ok(AssetType::Template),
            "Material" => Ok(AssetType::Material),
            _ => Ok(AssetType::Asset),
        }
    }

    fn is_text(&self) -> bool {
        matches!(self, FileType::Text | FileType::Code(_))
    }

    fn icon(&self) -> &'static str {
        match self {
            FileType::Directory => LUCIDE_FOLDER,
            FileType::Text => LUCIDE_FILE_TEXT,
            FileType::Image => LUCIDE_FILE_IMAGE,
            FileType::Code(Code::Rust) => LUCIDE_FILE_CODE,
            FileType::Code(Code::C) => LUCIDE_FILE_CODE,
            FileType::Code(Code::Cpp) => LUCIDE_FILE_CODE,
            FileType::Code(Code::Python) => LUCIDE_FILE_CODE,
            FileType::Code(Code::Js) => LUCIDE_FILE_CODE,
            FileType::Code(Code::Wgsl) => LUCIDE_FILE_CODE,
            FileType::Code(Code::Wasm) => LUCIDE_FILE_CODE,
            FileType::Code(Code::Json) => LUCIDE_FILE_JSON,
            FileType::Blend => LUCIDE_FILE_BOX,
            FileType::Gltf => LUCIDE_BOX,
            FileType::Archive => LUCIDE_FILE_ARCHIVE,
            FileType::Other => LUCIDE_FILE_QUESTION,
            FileType::Error => LUCIDE_FILE_WARNING,
            FileType::Hdri => LUCIDE_CLOUD_SUN,
            FileType::Asset(AssetType::Asset) => LUCIDE_PACKAGE,
            FileType::Asset(AssetType::Template) => LUCIDE_BOX,
            FileType::Asset(AssetType::Material) => LUCIDE_ECLIPSE,
        }
    }

    fn color(&self) -> Srgba {
        match self {
            FileType::Directory => SAPPHIRE_200,
            FileType::Text => PLATINUM_50,
            FileType::Image => RUBY_400,
            FileType::Code(Code::Rust) => AMBER_400,
            FileType::Code(Code::C) => SAPPHIRE_400,
            FileType::Code(Code::Cpp) => SAPPHIRE_400,
            FileType::Code(Code::Python) => AMBER_400,
            FileType::Code(Code::Js) => AMBER_400,
            FileType::Code(Code::Wgsl) => AMETHYST_400,
            FileType::Code(Code::Wasm) => AMETHYST_400,
            FileType::Code(Code::Json) => FOREST_400,
            FileType::Archive => PLATINUM_500,
            FileType::Other => PLATINUM_50,
            FileType::Error => RUBY_400,
            FileType::Blend => AMBER_400,
            FileType::Gltf => TEAL_400,
            FileType::Hdri => AMBER_400,
            FileType::Asset(AssetType::Asset) => AMBER_400,
            FileType::Asset(AssetType::Template) => SAPPHIRE_400,
            FileType::Asset(AssetType::Material) => RUBY_400,
        }
    }

    fn label(&self) -> Text {
        label(self.icon()).with_color(self.color())
    }

    /// Returns `true` if the file type is [`Image`].
    ///
    /// [`Image`]: FileType::Image
    #[must_use]
    fn is_image(&self) -> bool {
        matches!(self, Self::Image)
    }

    /// Returns `true` if the file type is [`Asset`].
    ///
    /// [`Asset`]: FileType::Asset
    #[must_use]
    fn is_asset(&self) -> bool {
        matches!(self, Self::Asset(_))
    }
}

struct FileDetailsWidget {
    assets: AssetCache,
    path: PathBuf,
    asset_root: PathBuf,
}

impl Widget for FileDetailsWidget {
    fn mount(self, scope: &mut Scope<'_>) {
        let path = self.path.clone();
        let file_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let file_size = path.metadata().map(|m| m.len()).unwrap_or(0);
        let file_size_str = bytes_to_human_readable(file_size);

        col((
                FilePreview {
                    assets: &self.assets,
                    path: &self.path,
                    asset_root: &self.asset_root,
                },
                Tooltip::label(
                    label(format!("Name: {file_name}")),
                    path.display().to_string(),
                ),
                label(format!("Size: {file_size_str}")),
        ))
            .mount(scope);
    }
}

struct FilePreview<'a> {
    assets: &'a AssetCache,
    path: &'a Path,
    asset_root: &'a Path,
}

impl Widget for FilePreview<'_> {
    fn mount(self, scope: &mut Scope<'_>) {
        to_owned!(
            assets = self.assets,
            root = self.asset_root,
            path = self.path
        );
        SuspenseWidget::new(Throbber::new(45.0), async move {
            let path = path;
            let ty = FileType::from_path(&root, &path, &assets).await;
            let icon = label(ty.icon())
                .with_color(ty.color())
                .with_font_size(256.0);

            move |scope: &mut Scope| {
                if ty.is_image() {
                    Image::new(path.canonicalize().unwrap())
                        .with_corner_radius(default_corner_radius())
                        .with_exact_size(Unit::px2(200.0, 200.0))
                        .mount(scope);
                    } else if ty.is_asset() {
                        AssetEditor::new(assets.clone(), root.into(), path.into()).mount(scope)
                    } else if ty.is_text() {
                        let async_load = async {
                            sleep(Duration::from_millis(500)).await;
                            let path = path;
                            let content = async_std::fs::read_to_string(&path).await;
                            |scope: &mut Scope<'_>| match content {
                                Ok(v) => FileEditor::new(Mutable::new(v), path).mount(scope),
                                Err(_) => label("Could not read file")
                                    .with_color(surface_danger())
                                    .mount(scope),
                            }
                        };

                        SuspenseWidget::new(LoadingSpinner::new("Loading Text"), async_load)
                            .mount(scope);
                        } else {
                            icon.mount(scope);
                    }
            }
        })
        .mount(scope)
    }
}

struct FileEditor {
    content: Mutable<String>,
    path: PathBuf,
}

impl FileEditor {
    fn new(content: Mutable<String>, path: PathBuf) -> Self {
        Self { content, path }
    }
}

impl Widget for FileEditor {
    fn mount(self, scope: &mut Scope<'_>) {
        let content = self.content.clone();

        col((
                ScrollArea::new(
                    BVec2::TRUE,
                    TextInput::new(content)
                    .with_style(TextInputStyle::default().with_font_family(FontFamily::Monospace)),
                ),
                Button::label("Save").on_click(move |_| {
                    let content = self.content.get_cloned();
                    let path = self.path.clone();
                    async_std::task::spawn(async move {
                        if let Err(err) = async_std::fs::write(&path, content).await {
                            tracing::error!("Failed to save file: {}", err);
                        } else {
                            tracing::info!("File saved successfully: {}", path.display());
                        }
                    });
                }),
        ))
            .mount(scope);
        }
}
