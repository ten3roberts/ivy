use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use async_std::stream::StreamExt;
use glam::{BVec2, Vec2};
use itertools::Itertools;
use ivy_assets::{AssetCache, AssetPath, meta::AssetPayloadUntyped};
use ivy_core::palette::Srgba;
use ivy_ui::violet::{
    core::{
        Edges, Scope, ScopeRef, Widget,
        components::LayoutAlignment,
        layout::Align,
        state::StateStream,
        stored::WeakHandle,
        style::{SizeExt, StyleExt, base_colors::*, default_corner_radius, surface_danger},
        text::{FontFamily, Wrap},
        time::sleep,
        to_owned,
        unit::Unit,
        widget::{
            Button, ButtonStyle, Collapsible, Image, IterWidgetCollection, LoadingSpinner,
            ScrollArea, Selectable, SignalWidget, StreamWidget, SuspenseWidget, TextInput,
            TextInputStyle, Throbber, WidgetExt, card, col, interactive::base::InteractiveWidget,
            label, pill, row,
        },
    },
    futures_signals::signal::Mutable,
    lucide::icons::{
        LUCIDE_BOX, LUCIDE_BOXES, LUCIDE_CLOUD_SUN, LUCIDE_ECLIPSE, LUCIDE_FILE_ARCHIVE,
        LUCIDE_FILE_BOX, LUCIDE_FILE_CODE, LUCIDE_FILE_IMAGE, LUCIDE_FILE_JSON,
        LUCIDE_FILE_QUESTION, LUCIDE_FILE_TEXT, LUCIDE_FILE_WARNING, LUCIDE_FOLDER,
        LUCIDE_FOLDER_OPEN, LUCIDE_PACKAGE,
    },
};

use crate::ui::asset_inspector::AssetInspector;

pub const BROWSER_PANEL_HEIGHT: f32 = 300.0;
pub const INSPECTOR_PANEL_HEIGHT: f32 = 500.0;
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
        let items = std::fs::read_dir(&self.path)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| {
                let entry_path = entry.path();
                let entry_name = entry_path.file_name().unwrap();
                let is_dir = entry_path.is_dir();

                Item {
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
        let cols = lines
            .into_iter()
            .map(|line| row(IterWidgetCollection::new(line)));

        col((
            Breadcrumbs {
                path: &self.path,
                selection: self.selected_dir,
            },
            ScrollArea::vertical(col(IterWidgetCollection::new(cols))),
        ))
        .with_stretch(true)
        .mount(scope)
    }
}

pub const ITEM_SIZE: Unit<Vec2> = Unit::px2(120.0, 100.0);

pub struct FileIcon<'a> {
    path: &'a Path,
    assets: &'a AssetCache,
}

impl Widget for FileIcon<'_> {
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

pub struct Item {
    name: String,
    path: PathBuf,
    assets: AssetCache,
    selected: WeakHandle<Mutable<Option<PathBuf>>>,
    selected_dir: WeakHandle<Mutable<Option<PathBuf>>>,
    is_dir: bool,
}

impl Widget for Item {
    fn mount(self, scope: &mut Scope<'_>) {
        let path = self.path.clone();
        Selectable::new_value(
            col((
                FileIcon {
                    path: &self.path,
                    assets: &self.assets,
                },
                RenamableItem {
                    path: self.path.clone(),
                    selected: self.selected,
                    selected_dir: self.selected_dir,
                },
            ))
            .with_cross_align(Align::Center)
            .with_exact_size(ITEM_SIZE),
            scope.read(&self.selected).clone(),
            Some(self.path.clone()),
        )
        .on_double_click(move |scope: &ScopeRef| {
            if self.is_dir {
                scope.read(self.selected_dir).set(Some(path.clone()));
            }
        })
        .with_style(ButtonStyle::hidden())
        .mount(scope);
    }
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

enum AssetType {
    Asset,
    Template,
    Material,
}

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
            "MaterialData" => Ok(AssetType::Material),
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
            FileType::Asset(AssetType::Material) => LUCIDE_ECLIPSE,
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
        .with_max_size(Unit::px2(INSPECTOR_PANEL_WIDTH, INSPECTOR_PANEL_HEIGHT))
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
