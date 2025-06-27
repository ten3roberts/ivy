use std::{
    future::ready,
    path::{Path, PathBuf},
    time::Duration,
};

use async_std::{
    io::{BufReadExt, BufReader},
    stream::StreamExt,
};
use flax::Component;
use futures::AsyncReadExt;
use glam::{BVec2, Vec2};
use itertools::Itertools;
use ivy_core::palette::Srgba;
use ivy_ui::violet::{
    core::{
        Edges, Scope, ScopeRef, Widget,
        layout::Align,
        stored::WeakHandle,
        style::{
            SizeExt, StyleExt, base_colors::*, element_pressed, element_primary, surface_danger,
        },
        text::{FontFamily, Wrap},
        time::sleep,
        unit::Unit,
        widget::{
            Button, ButtonStyle, Collapsible, CollapsibleStyle, DeferWidget, FutureWidget, Image,
            IterWidgetCollection, ScrollArea, Selectable, SignalWidget, TextInput, TextInputStyle,
            card, col,
            interactive::{base::InteractiveWidget, tooltip::Tooltip},
            label, pill, row,
        },
    },
    futures_signals::signal::{Mutable, SignalExt},
    lucide::icons::{
        LUCIDE_ELLIPSIS, LUCIDE_FILE, LUCIDE_FILE_ARCHIVE, LUCIDE_FILE_BOX, LUCIDE_FILE_CODE,
        LUCIDE_FILE_IMAGE, LUCIDE_FILE_JSON, LUCIDE_FILE_QUESTION, LUCIDE_FILE_TEXT, LUCIDE_FOLDER,
        LUCIDE_FOLDER_OPEN, LUCIDE_IMAGE,
    },
};

pub struct DirectoryTree {
    selection: WeakHandle<Mutable<Option<PathBuf>>>,
    path: PathBuf,
    max_depth: usize,
}

impl Widget for DirectoryTree {
    fn mount(self, scope: &mut Scope<'_>) {
        let path = self.path;
        let name = path.file_name().unwrap().to_owned();

        if self.max_depth == 0 {
            Tooltip::label(label(LUCIDE_ELLIPSIS), "Maximum depth reached").mount(scope);
            return;
        }

        let selection = scope.read(&self.selection).clone();
        DeferWidget::new(label("Loading"), async move {
            sleep(Duration::from_millis(100)).await;

            let mut item_count = 0;
            let subdirs = std::fs::read_dir(&path)
                .unwrap()
                .filter_map(Result::ok)
                .filter_map(|entry| {
                    let entry_path = entry.path();
                    let entry_name = entry_path.file_name().unwrap();

                    item_count += 1;

                    if entry_path.is_dir() {
                        tracing::info!("Directory: {}", entry_path.display());
                        Some(DirectoryTree {
                            path: entry_path,
                            max_depth: self.max_depth - 1,
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
                row((label(icon), label(name.to_string_lossy()))).with_stretch(true),
                selection,
                Some(path),
            )
            .with_style(ButtonStyle::hidden());

            Collapsible::new(header, subdirs).can_collapse(subdir_count > 0)
        })
        .mount(scope);
    }
}

pub struct DirectoryListing {
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
                tracing::info!("Listing entry: {}", entry.path().display());
                let entry_path = entry.path();
                let entry_name = entry_path.file_name().unwrap();

                Item {
                    name: entry_name.to_string_lossy().to_string(),
                    path: entry_path,
                    selected: self.selected_file,
                    selected_dir: self.selected_dir,
                }
                // if entry_path.is_dir() {
                //     tracing::info!("Directory: {}", entry_path.display());
                //     Some(DirectoryTree {
                //         path: entry_path,
                //         max_depth: self.max_depth - 1,
                //     })
                // } else {
                //     None
                // }
            });

        let lines = items.chunks(6);
        let cols = lines
            .into_iter()
            .map(|line| row(IterWidgetCollection::new(line)));

        col((
            Breadcrumbs {
                path: &self.path,
                selection: self.selected_dir,
            },
            ScrollArea::vertical(col(IterWidgetCollection::new(cols)).with_maximize(Vec2::X)),
        ))
        .mount(scope)
    }
}

pub const ITEM_SIZE: Unit<Vec2> = Unit::px2(120.0, 100.0);

pub struct FileIcon<'a> {
    path: &'a Path,
}

impl Widget for FileIcon<'_> {
    fn mount(self, scope: &mut Scope<'_>) {
        let ty = FileType::from_path(self.path);

        if ty.is_image() {
            // If the file is an image, we can display it directly
            let image = Image::new(self.path.canonicalize().unwrap().to_owned());
            image.with_exact_size(Unit::px2(64.0, 64.0)).mount(scope);
            return;
        }

        label(ty.icon())
            .with_color(ty.color())
            .with_font_size(48.0)
            .mount(scope);
    }
}

pub struct Item {
    name: String,
    path: PathBuf,
    selected: WeakHandle<Mutable<Option<PathBuf>>>,
    selected_dir: WeakHandle<Mutable<Option<PathBuf>>>,
}

impl Widget for Item {
    fn mount(self, scope: &mut Scope<'_>) {
        let path = self.path.clone();
        let is_dir = path.is_dir();
        Selectable::new_value(
            col((
                FileIcon { path: &self.path },
                label(self.name)
                    .with_wrap(Wrap::WordOrGlyph)
                    .with_font_size(12.0),
            ))
            .with_cross_align(Align::Center)
            .with_exact_size(ITEM_SIZE),
            scope.read(&self.selected).clone(),
            Some(self.path.clone()),
        )
        .on_double_click(move |scope: &ScopeRef| {
            if is_dir {
                scope.read(self.selected_dir).set(Some(path.clone()));
            }
        })
        .with_style(ButtonStyle::hidden())
        .mount(scope);
    }
}

pub struct DirectoryBrowser {
    path: PathBuf,
}

impl DirectoryBrowser {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl Widget for DirectoryBrowser {
    fn mount(self, scope: &mut Scope<'_>) {
        let selected_dir = Mutable::new(None as Option<PathBuf>);
        let selected_file = Mutable::new(None as Option<PathBuf>);

        let selected_dir = scope.store(selected_dir);
        let selected_file = scope.store(selected_file);

        let details_panel =
            SignalWidget::new(scope.read(&selected_file).signal_ref(move |selected| {
                selected
                    .as_ref()
                    .map(|v| FileDetailsPanel { path: v.to_owned() })
            }));

        row((
            card(row((
                ScrollArea::vertical(DirectoryTree {
                    selection: selected_dir,
                    path: self.path,
                    max_depth: 4,
                }),
                SignalWidget::new(scope.read(&selected_dir).signal_ref(move |selected| {
                    selected.as_ref().map(|v| DirectoryListing {
                        path: v.clone(),
                        selected_file,
                        selected_dir,
                    })
                })),
            )))
            .with_max_size(Unit::px2(f32::MAX, 300.0))
            .with_maximize(Vec2::X),
            details_panel,
        ))
        .with_cross_align(Align::End)
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
                tracing::info!("Breadcrumb clicked: {}", full_path.display());
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
    path: PathBuf,
}

fn bytes_to_human_readable(size: u64) -> String {
    if size < 1024 {
        format!("{} bytes", size)
    } else if size < 1024 * 1024 {
        format!("{:.2} KB", size as f64 / 1024.0)
    } else if size < 1024 * 1024 * 1024 {
        format!("{:.2} MB", size as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", size as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

enum FileType {
    Directory,
    Text,
    Image,
    Archive,
    Json,
    Rust,
    C,
    Cpp,
    Python,
    Js,
    Wgsl,
    Wasm,
    Blend,
    Other,
}

impl FileType {
    fn from_path(path: &Path) -> Self {
        if path.is_dir() {
            FileType::Directory
        } else {
            match path.extension().and_then(|s| s.to_str()) {
                Some(ext) => match ext {
                    "txt" | "md" | "markdown" => FileType::Text,
                    // "rs" | "py" | "js" | "ts" | "c" | "cpp" | "h" | "hpp" => FileType::Code,
                    "png" | "jpg" | "jpeg" | "gif" | "webp" => FileType::Image,
                    "zip" | "tar" | "gz" | "rar" => FileType::Archive,
                    "json" | "yaml" | "yml" => FileType::Json,
                    "rs" => FileType::Rust,
                    "c" => FileType::C,
                    "cpp" => FileType::Cpp,
                    "py" => FileType::Python,
                    "js" => FileType::Js,
                    "wgsl" => FileType::Wgsl,
                    "wasm" => FileType::Wasm,
                    "blend" => FileType::Blend,
                    "blend1" => FileType::Blend, // Blender backup files
                    "blend2" => FileType::Blend, // Blender backup files
                    _ => FileType::Other,
                },
                _ => FileType::Other,
            }
        }
    }

    fn is_code(&self) -> bool {
        matches!(
            self,
            FileType::Rust
                | FileType::C
                | FileType::Cpp
                | FileType::Python
                | FileType::Js
                | FileType::Wgsl
                | FileType::Wasm
        )
    }

    fn is_text(&self) -> bool {
        matches!(self, FileType::Text | FileType::Json) || self.is_code()
    }

    fn icon(&self) -> &'static str {
        match self {
            FileType::Directory => LUCIDE_FOLDER,
            FileType::Text => LUCIDE_FILE_TEXT,
            FileType::Image => LUCIDE_FILE_IMAGE,
            FileType::Rust => LUCIDE_FILE_CODE,
            FileType::C => LUCIDE_FILE_CODE,
            FileType::Cpp => LUCIDE_FILE_CODE,
            FileType::Python => LUCIDE_FILE_CODE,
            FileType::Js => LUCIDE_FILE_CODE,
            FileType::Wgsl => LUCIDE_FILE_CODE,
            FileType::Wasm => LUCIDE_FILE_CODE,
            FileType::Blend => LUCIDE_FILE_BOX,
            FileType::Archive => LUCIDE_FILE_ARCHIVE,
            FileType::Json => LUCIDE_FILE_JSON,
            FileType::Other => LUCIDE_FILE_QUESTION,
        }
    }

    fn color(&self) -> Srgba {
        match self {
            FileType::Directory => OCEAN_200,
            FileType::Text => PLATINUM_50,
            FileType::Image => CHERRY_400,
            FileType::Rust => AMBER_400,
            FileType::C => OCEAN_400,
            FileType::Cpp => OCEAN_400,
            FileType::Python => CITRUS_400,
            FileType::Js => CITRUS_400,
            FileType::Wgsl => AMETHYST_400,
            FileType::Wasm => AMETHYST_400,
            FileType::Archive => PLATINUM_500,
            FileType::Json => FOREST_400,
            FileType::Other => PLATINUM_50,
            FileType::Blend => AMBER_400,
        }
    }

    /// Returns `true` if the file type is [`Image`].
    ///
    /// [`Image`]: FileType::Image
    #[must_use]
    fn is_image(&self) -> bool {
        matches!(self, Self::Image)
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

        card(
            col((
                FilePreview { path: &self.path },
                label("File Details")
                    .with_font_size(16.0)
                    .with_color(OCEAN_200),
                label(format!("Name: {}", file_name)),
                label(format!("Size: {}", file_size_str)),
                // label(format!("Path: {}", path.display())),
            ))
            .with_max_size(Unit::px2(500.0, 500.0)),
        )
        .mount(scope);
    }
}

struct FilePreview<'a> {
    path: &'a Path,
}

impl Widget for FilePreview<'_> {
    fn mount(self, scope: &mut Scope<'_>) {
        let ty = FileType::from_path(&self.path);
        let icon = label(ty.icon())
            .with_color(ty.color())
            .with_font_size(256.0);

        if ty.is_image() {
            Image::new(self.path.canonicalize().unwrap())
                .with_exact_size(Unit::px2(256.0, 256.0))
                .mount(scope);
        } else if ty.is_text() {
            let path = self.path.to_owned();
            let async_load = async {
                let path = path;
                let content = async_std::fs::read_to_string(&path).await;
                |scope: &mut Scope<'_>| match content {
                    Ok(v) => FileEditor::new(Mutable::new(v), path).mount(scope),
                    Err(err) => label("Could not read file")
                        .with_color(surface_danger())
                        .mount(scope),
                }
            };

            DeferWidget::new(
                label("Loading file preview...").with_wrap(Wrap::WordOrGlyph),
                async_load,
            )
            .mount(scope);
        } else {
            icon.mount(scope);
        }
    }
}

async fn read_file_chunked(path: &Path, max_size: usize) -> Result<String, std::io::Error> {
    let file = async_std::fs::File::open(path).await?;

    let mut reader = BufReader::new(file);
    let mut content = String::new();
    let mut buf = [0; 1024];

    loop {
        if content.len() >= max_size {
            break;
        }

        let read = reader.read(&mut buf).await?;
        if read == 0 {
            break;
        }

        content.push_str(&String::from_utf8_lossy(&buf[..read]));
    }

    Ok(content)
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
        let path = self.path.clone();

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
