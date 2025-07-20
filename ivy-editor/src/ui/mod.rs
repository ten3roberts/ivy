pub mod asset_inspector;
pub mod browser;
mod drop;
pub mod entity_editor;

use flax::Entity;
use futures::StreamExt;
use glam::Vec2;
use itertools::Itertools;
use ivy_assets::AssetCache;
use ivy_ui::{
    screens::Screen,
    streamed::StreamedUiExt,
    violet::{
        core::{
            Widget,
            components::LayoutAlignment,
            layout::Align,
            state::StateExt,
            style::SizeExt,
            to_owned,
            unit::Unit,
            widget::{
                Selectable, Stack, StreamWidget, card, col, interactive::base::TooltipOptions,
                label, maximized, row,
            },
        },
        futures_signals::signal::Mutable,
    },
};

use crate::{
    plugin::selection,
    tools_controller::{current_tool, tools},
    ui::{browser::DirectoryBrowser, drop::WorldDropArea, entity_editor::EntityComponentEditor},
};

pub struct EditorUi {
    assets: AssetCache,
    editor: Entity,
    tool_ui: flume::Receiver<Box<dyn Widget + Send>>,
}

impl EditorUi {
    pub fn new(
        assets: AssetCache,
        editor: Entity,
        tool_ui: flume::Receiver<Box<dyn Send + Widget>>,
    ) -> Self {
        Self {
            assets,
            editor,
            tool_ui,
        }
    }
}

impl Screen for EditorUi {
    fn create(
        self,
        scope: &mut ivy_ui::violet::core::Scope<'_>,
        token: ivy_ui::screens::ScreenLifetimeToken,
    ) {
        scope.monitor_entity_lifetime(self.editor, move || token.close_screen());

        Stack::new((
            WorldDropArea {},
            col((
                EditorMenuBar {
                    editor: self.editor,
                },
                maximized((
                    StreamWidget::new(self.tool_ui.into_stream()),
                    InspectorUI {
                        editor: self.editor,
                    },
                )),
            )),
            DirectoryBrowser::new(self.assets, "./assets"),
        ))
        .mount(scope);
    }
}

pub struct EditorMenuBar {
    editor: Entity,
}

impl Widget for EditorMenuBar {
    fn mount(self, scope: &mut ivy_ui::violet::core::Scope<'_>) {
        card(
            row((ToolSelectionWidget {
                editor: self.editor,
            },))
            .with_cross_align(Align::Center)
            .with_maximize(Vec2::X),
        )
        .with_item_align(LayoutAlignment::new(Align::Start, Align::Start))
        .mount(scope);
    }
}

pub struct ToolSelectionWidget {
    editor: Entity,
}

impl Widget for ToolSelectionWidget {
    fn mount(self, scope: &mut ivy_ui::violet::core::Scope<'_>) {
        let selection = Mutable::new(None);
        scope.stream_component_duplex(current_tool(), self.editor, selection.clone());

        let tools = scope.stream_component(tools(), self.editor).into_stream();

        let item_column = {
            to_owned!(selection);
            tools.map(move |tools| {
                to_owned!(selection);
                let items = tools.iter().enumerate().map(move |(i, v)| {
                    let name = v.name.to_string();

                    Selectable::new_value(
                        label(&v.icon),
                        selection.clone().lower_option().lower_option(),
                        i,
                    )
                    .with_tooltip(TooltipOptions::new(move || label(&name)))
                });

                row(items.collect_vec())
            })
        };

        (StreamWidget::new(item_column)).mount(scope);
    }
}

pub struct InspectorUI {
    editor: Entity,
}

impl Widget for InspectorUI {
    fn mount(self, scope: &mut ivy_ui::violet::core::Scope<'_>) {
        let selection = scope.stream_component(selection(), self.editor);

        let editor = selection.into_stream().map(move |selection| {
            if let Some(&entity) = selection.entities().first() {
                let editor = EntityComponentEditor::new(entity);
                Some(card(editor).with_min_size(Unit::px2(300.0, 200.0)))
            } else {
                None
            }
        });

        Stack::new(StreamWidget::new(editor))
            .with_item_align(LayoutAlignment::new(Align::End, Align::Start))
            .mount(scope);
    }
}
