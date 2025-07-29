use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Context;
use flax::{EntityRef, Query};
use futures::StreamExt;
use glam::{BVec2, Vec2};
use itertools::Itertools;
use ivy_assets::{
    AssetCache, AssetPath, Resource,
    loadable::Loadable,
    meta::{AssetMeta, AssetPayload, AssetPayloadUntyped},
};
use ivy_core::{
    AsyncCommandBuffer, EntityBuilderExt,
    components::{TransformBundle, async_commandbuffer, engine, main_camera},
    palette::Srgba,
    template::Template,
};
use ivy_input::types::MouseButton;
use ivy_physics::{components::physics_state, rapier3d::prelude::QueryFilter};
use ivy_scene::camera::{CameraQuery, screen_to_world_ray};
use ivy_ui::{
    streamed::StreamedUiExt,
    violet::{
        core::{
            Edges, Scope, ScopeRef, Widget,
            components::{LayoutAlignment, rect},
            layout::Align,
            state::StateStream,
            stored::WeakHandle,
            style::{SizeExt, StyleExt, base_colors::*, default_corner_radius, surface_danger},
            text::{FontFamily, Wrap},
            time::sleep,
            to_owned,
            unit::Unit,
            widget::{
                Button, ButtonStyle, Collapsible, Draggable, FutureWidget, Image,
                IterWidgetCollection, LoadingSpinner, ScrollArea, Selectable, SignalWidget, Stack,
                StreamWidget, SuspenseWidget, Text, TextInput, TextInputStyle, Throbber, WidgetExt,
                card, col,
                interactive::{base::InteractiveWidget, overlay::overlay_state},
                label, pill, row,
            },
        },
        futures_signals::signal::Mutable,
        lucide::icons::{
            LUCIDE_BOX, LUCIDE_BOXES, LUCIDE_CLOUD_SUN, LUCIDE_CROSS, LUCIDE_DROPLET,
            LUCIDE_ECLIPSE, LUCIDE_FILE_ARCHIVE, LUCIDE_FILE_BOX, LUCIDE_FILE_CODE,
            LUCIDE_FILE_IMAGE, LUCIDE_FILE_JSON, LUCIDE_FILE_QUESTION, LUCIDE_FILE_TEXT,
            LUCIDE_FILE_WARNING, LUCIDE_FOLDER, LUCIDE_FOLDER_OPEN, LUCIDE_PACKAGE, LUCIDE_TRASH_2,
            LUCIDE_X,
        },
    },
};
use ivy_wgpu::material::{Material, MaterialDesc};
use notify::Watcher;
use serde::Serialize;

use crate::ui::{
    asset_inspector::AssetInspector,
    context_menu::{ContextMenu, ContextMenuItem, ContextMenuPanel},
    drop::world_drop_area,
};

pub const BROWSER_PANEL_HEIGHT: f32 = 300.0;
pub const INSPECTOR_PANEL_MAX_HEIGHT: f32 = 800.0;
pub const INSPECTOR_PANEL_WIDTH: f32 = 600.0;

pub struct DirectoryTree {
    selection: WeakHandle<Mutable<Option<PathBuf>>>,
    path: PathBuf,
    expand_depth: usize,
}

impl Widget for DirectoryTree {
    fn mount(self, scope: &mut Scope<'_>) {
        let path = self.path;
        let name = path.file_name().unwrap().to_string_lossy().to_string();

        let selection = scope.read(&self.selection).clone();

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
                        selection: self.selection,
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

        let header = Selectable::new_value(
            row((
                // label(special_folder_icon(&name).unwrap_or(icon)),
                label(icon),
                label(name),
            ))
            .with_stretch(true),
            selection,
            Some(path.clone()),
        )
        .with_style(ButtonStyle::hidden().with_align(LayoutAlignment::left_center()))
        .with_maximize(Vec2::X);

        Collapsible::deferred(header, || subdirs)
            .can_collapse(subdir_count > 0)
            .collapsed(self.expand_depth == 0)
            .with_name(path.display().to_string())
            .mount(scope);
    }
}

pub struct DirectoryListing {
    assets: AssetCache,
    path: PathBuf,
    selected_file: WeakHandle<Mutable<Option<PathBuf>>>,
    selected_dir: WeakHandle<Mutable<Option<PathBuf>>>,
}

impl Widget for DirectoryListing {
    fn mount(self, scope: &mut Scope<'_>) {
        let path = self.path.clone();
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
                        selected: self.selected_file,
                        selected_dir: self.selected_dir,
                        assets: self.assets.clone(),
                    }
                })
                .sorted_by_key(|item| (!item.is_dir, item.name.clone()));

            let lines = items.chunks(8);
            lines
                .into_iter()
                .map(|line| row(line.collect_vec()))
                .collect_vec()
        };

        let path = self.path.clone();
        let open_context_menu = {
            move |scope: &ScopeRef, pos| {
                open_context_menu(scope, pos, populate_menu(path.clone(), self.selected_file))
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
                    selection: self.selected_dir,
                },
                ScrollArea::vertical(StreamWidget::new(
                    refresh_state.stream().map(move |()| col(read_dir())),
                )),
            ))
            .with_stretch(true),
        )
        .on_mouse_input(move |scope, input| {
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
        ContextMenuItem::new(label(LUCIDE_TRASH_2), "Delete File", move |scope| {
            if let Err(err) = std::fs::remove_file(&path) {
                tracing::error!("Failed to delete file: {}", err);
            } else {
                tracing::info!("File deleted: {}", path.display());
                scope.read(selected_item).set(None);
            }
            Ok(())
        }),
        ContextMenuItem::new(label(LUCIDE_X), "Close", move |_| Ok(())),
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
    tracing::info!("Asset created: {}", new_path.display());
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
                tracing::info!("Creating new file");
                let new_path = find_next_filename("New File", &dir);
                std::fs::write(&new_path, "")?;
                tracing::info!("File created: {}", new_path.display());

                scope.read(selected_item).set(Some(new_path));
                // Implement file creation logic here
                Ok(())
            }
        }),
        ContextMenuItem::new(
            FileType::Asset(AssetType::Template).label(),
            "New Template",
            |_| Ok(()),
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
        ContextMenuItem {
            icon: FileType::Directory.label(),
            label: "New Folder".to_string(),
            action: Box::new(|_| {
                tracing::info!("Creating new folder");
                // Implement folder creation logic here
                Ok(())
            }),
        },
        ContextMenuItem::new(label(LUCIDE_X), "Close", move |_| Ok(())),
    ])
}

pub const ITEM_SIZE: Unit<Vec2> = Unit::px2(120.0, 100.0);

pub struct FileIcon {
    path: PathBuf,
    assets: AssetCache,
}

impl Widget for FileIcon {
    fn mount(self, scope: &mut Scope<'_>) {
        to_owned!(path = self.path, assets = self.assets);
        SuspenseWidget::new(Throbber::new(48.0), async move {
            let path = path;
            let ty = FileType::from_path(&path, &assets).await;

            move |scope: &mut Scope<'_>| {
                // Load the file type asynchronously
                if ty.is_image() {
                    // If the file is an image, we can display it directly
                    let image = Image::new(path.canonicalize().unwrap().to_owned());
                    image
                        .with_exact_size(Unit::px2(64.0, 64.0))
                        .with_corner_radius(default_corner_radius())
                        .mount(scope);
                    return;
                }

                label(ty.icon())
                    .with_color(ty.color())
                    .with_font_size(48.0)
                    .mount(scope);
            }
        })
        .mount(scope)
    }
}

pub struct FileItem {
    name: String,
    path: PathBuf,
    assets: AssetCache,
    selected: WeakHandle<Mutable<Option<PathBuf>>>,
    selected_dir: WeakHandle<Mutable<Option<PathBuf>>>,
    is_dir: bool,
}

impl Widget for FileItem {
    fn mount(self, scope: &mut Scope<'_>) {
        let is_selected = scope.read(&self.selected).clone();
        let widget = async move {
            let path = self.path.clone();
            let assets = self.assets.clone();
            let filetype = FileType::from_path(&path, &assets).await;
            let preview = {
                to_owned!(path);
                move || label(path.file_name().unwrap_or_default().to_string_lossy())
            };

            let on_drop = {
                to_owned!(path);
                move |_scope: &ScopeRef, target: Option<(EntityRef, Vec2)>| {
                    tracing::info!(?path, "Dropping file to {:?}", target);

                    if let Some((widget, pos)) = target {
                        if widget.has(world_drop_area()) {
                            let screen_pos = pos / widget.get(rect()).unwrap().size();
                            to_owned!(path, assets, filetype);
                            _scope.apply(move |world| {
                                let mut main_camera =
                                    Query::new(CameraQuery::new()).with(main_camera());

                                let mut main_camera = main_camera.borrow(world);

                                let main_camera = main_camera.first().context("No main camera")?;

                                let physics = world.get(engine(), physics_state()).unwrap();

                                let ray = screen_to_world_ray(screen_pos, main_camera);
                                let hit =
                                    physics.cast_ray(ray, 1000.0, false, QueryFilter::default());

                                match filetype {
                                    FileType::Asset(AssetType::Template) => {
                                        if let Some(hit) = hit {
                                            tracing::info!("Dropping template at {:?}", hit);
                                            let asset_path =
                                                AssetPath::<Template>::new(path.canonicalize()?);

                                            let async_cmd = world
                                                .get_clone(engine(), async_commandbuffer())
                                                .unwrap();

                                            async_std::task::spawn(async move {
                                                let fut = spawn_template(
                                                    assets, ray, asset_path, hit, async_cmd,
                                                )
                                                .await;

                                                if let Err(err) = fut {
                                                    tracing::error!("{err:?}");
                                                }
                                            });
                                        } else {
                                            tracing::warn!("No hit detected for template drop");
                                        }
                                    }
                                    _ => {
                                        tracing::info!("Dropping file at {:?}", pos);
                                    }
                                }

                                Ok(())
                            });
                        }
                    }
                }
            };
            Selectable::new_value(
                Draggable::new(
                    col((
                        FileIcon {
                            path: path.clone(),
                            assets: self.assets,
                        },
                        RenamableItem {
                            path: self.path.clone(),
                            selected: self.selected,
                            selected_dir: self.selected_dir,
                        },
                    ))
                    .with_cross_align(Align::Center)
                    .with_exact_size(ITEM_SIZE),
                    preview,
                    on_drop,
                ),
                is_selected,
                Some(self.path.clone()),
            )
            .on_double_click({
                move |scope: &ScopeRef| {
                    if self.is_dir {
                        scope.read(self.selected_dir).set(Some(path.clone()));
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
                            populate_item_menu(self.selected.clone(), path.clone()),
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
    selected: WeakHandle<Mutable<Option<PathBuf>>>,
    selected_dir: WeakHandle<Mutable<Option<PathBuf>>>,
}

impl Widget for RenamableItem {
    fn mount(mut self, scope: &mut Scope<'_>) {
        let renaming = Mutable::new(false);

        let mut edit_state = None as Option<Mutable<String>>;

        let selected = scope.read(&self.selected).clone();
        let selected_dir = scope.read(&self.selected_dir).clone();

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
                    selected_dir.set(Some(
                        self.path
                            .parent()
                            .unwrap_or_else(|| Path::new("."))
                            .to_owned(),
                    ));
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

pub struct DirectoryBrowser {
    assets: AssetCache,
    path: PathBuf,
}

impl DirectoryBrowser {
    pub fn new(assets: AssetCache, path: impl Into<PathBuf>) -> Self {
        Self {
            assets,
            path: path.into(),
        }
    }
}

impl Widget for DirectoryBrowser {
    fn mount(self, scope: &mut Scope<'_>) {
        let selected_dir = Mutable::new(Some(self.path.clone()));
        let selected_file = Mutable::new(Some(self.path.clone()));

        let selected_dir = scope.store(selected_dir);
        let selected_file = scope.store(selected_file);

        let details_panel = SignalWidget::new(scope.read(&selected_file).signal_ref({
            to_owned!(assets = self.assets);
            move |selected| {
                to_owned!(assets);
                selected.as_ref().map(move |v| FileDetailsPanel {
                    assets: assets.clone(),
                    path: v.to_owned(),
                })
            }
        }));

        row((
            card(row((
                ScrollArea::vertical(DirectoryTree {
                    selection: selected_dir,
                    path: self.path,
                    expand_depth: 1,
                }),
                SignalWidget::new(scope.read(&selected_dir).signal_ref({
                    to_owned!(assets = self.assets);
                    move |selected| {
                        selected.as_ref().map(|v| DirectoryListing {
                            path: v.clone(),
                            selected_file,
                            selected_dir,
                            assets: assets.clone(),
                        })
                    }
                })),
            )))
            .with_min_size(Unit::px2(100.0, BROWSER_PANEL_HEIGHT))
            .with_max_size(Unit::px2(f32::MAX, BROWSER_PANEL_HEIGHT))
            .with_maximize(Vec2::X),
            details_panel,
        ))
        .with_cross_align(Align::End)
        .with_item_align(LayoutAlignment::bottom_left())
        .mount(scope)
    }
}

pub struct Breadcrumbs<'a> {
    path: &'a Path,
    selection: WeakHandle<Mutable<Option<PathBuf>>>,
}

impl Widget for Breadcrumbs<'_> {
    fn mount(self, scope: &mut Scope<'_>) {
        let mut tail = Vec::new();
        let items = self.path.components().map(|segment| {
            let segment_str = segment.as_os_str().to_string_lossy().to_string();
            let full_path = tail.iter().chain([&segment_str]).collect::<PathBuf>();

            let widget = InteractiveWidget::new(pill(
                label(&segment_str)
                    .with_color(OCEAN_200)
                    .with_wrap(Wrap::None),
            ))
            .on_click(move |scope: &ScopeRef| {
                scope.read(self.selection).set(Some(full_path.clone()));
            });

            tail.push(segment_str);
            widget
        });

        row(IterWidgetCollection::new(items))
            .with_cross_align(Align::Center)
            .mount(scope);
    }
}

pub struct FileDetailsPanel {
    assets: AssetCache,
    path: PathBuf,
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
    async fn from_path(path: &Path, assets: &AssetCache) -> Self {
        if path.is_dir() {
            FileType::Directory
        } else {
            match path.extension().and_then(|s| s.to_str()) {
                Some(ext) => match ext {
                    "asset" => Self::determine_asset_type(path, assets)
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

    async fn determine_asset_type(path: &Path, assets: &AssetCache) -> anyhow::Result<AssetType> {
        let path = AssetPath::new(path.canonicalize()?);
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
            FileType::Asset(AssetType::Template) => LUCIDE_BOXES,
            FileType::Asset(AssetType::Material) => LUCIDE_DROPLET,
        }
    }

    fn color(&self) -> Srgba {
        match self {
            FileType::Directory => OCEAN_200,
            FileType::Text => PLATINUM_50,
            FileType::Image => CHERRY_400,
            FileType::Code(Code::Rust) => AMBER_400,
            FileType::Code(Code::C) => OCEAN_400,
            FileType::Code(Code::Cpp) => OCEAN_400,
            FileType::Code(Code::Python) => CITRUS_400,
            FileType::Code(Code::Js) => CITRUS_400,
            FileType::Code(Code::Wgsl) => AMETHYST_400,
            FileType::Code(Code::Wasm) => AMETHYST_400,
            FileType::Code(Code::Json) => FOREST_400,
            FileType::Archive => PLATINUM_500,
            FileType::Other => PLATINUM_50,
            FileType::Error => RUBY_400,
            FileType::Blend => AMBER_400,
            FileType::Gltf => TEAL_400,
            FileType::Hdri => CITRUS_400,
            FileType::Asset(AssetType::Asset) => CITRUS_400,
            FileType::Asset(AssetType::Template) => OCEAN_400,
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

impl Widget for FileDetailsPanel {
    fn mount(self, scope: &mut Scope<'_>) {
        let path = self.path.clone();
        let file_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let file_size = path.metadata().map(|m| m.len()).unwrap_or(0);
        let file_size_str = bytes_to_human_readable(file_size);

        card(col((
            FilePreview {
                assets: &self.assets,
                path: &self.path,
            },
            label("File Details")
                .with_font_size(16.0)
                .with_color(OCEAN_200),
            label(format!("Name: {file_name}")),
            label(format!("Size: {file_size_str}")),
            // label(format!("Path: {}", path.display())),
        )))
        .with_min_size(Unit::px2(INSPECTOR_PANEL_WIDTH, 200.0))
        .with_max_size(Unit::px2(INSPECTOR_PANEL_WIDTH, INSPECTOR_PANEL_MAX_HEIGHT))
        .mount(scope);
    }
}

struct FilePreview<'a> {
    assets: &'a AssetCache,
    path: &'a Path,
}

impl Widget for FilePreview<'_> {
    fn mount(self, scope: &mut Scope<'_>) {
        to_owned!(assets = self.assets, path = self.path);
        SuspenseWidget::new(Throbber::new(45.0), async move {
            let path = path;
            let ty = FileType::from_path(&path, &assets).await;
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
                    tracing::info!("Inspecting asset: {}", path.display());
                    AssetInspector::new(assets.clone(), path.to_owned()).mount(scope)
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
