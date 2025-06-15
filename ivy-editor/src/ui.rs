use flax::Entity;
use futures::StreamExt;
use glam::Vec2;
use itertools::Itertools;
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
            widget::{
                Radio, StreamWidget, bold, card, col, interactive::base::TooltipOptions, label, row,
            },
        },
        futures_signals::signal::Mutable,
    },
};

use crate::tools_controller::{current_tool, tools};

pub struct EditorUi {
    editor: Entity,
    tool_ui: flume::Receiver<Box<dyn Widget + Send>>,
}

impl EditorUi {
    pub fn new(editor: Entity, tool_ui: flume::Receiver<Box<dyn Send + Widget>>) -> Self {
        Self { editor, tool_ui }
    }
}

impl Screen for EditorUi {
    fn create(
        self,
        scope: &mut ivy_ui::violet::core::Scope<'_>,
        token: ivy_ui::screens::ScreenLifetimeToken,
    ) {
        scope.monitor_entity_lifetime(self.editor, move || token.close_screen());

        col((
            EditorMenuBar {
                editor: self.editor,
            },
            StreamWidget::new(self.tool_ui.into_stream()),
        ))
        .with_item_align(LayoutAlignment::new(Align::Start, Align::Start))
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

                    Radio::new_indexed(
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
