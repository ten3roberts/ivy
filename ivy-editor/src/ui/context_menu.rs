use glam::Vec2;
use ivy_core::palette::Srgba;
use ivy_ui::violet::core::{
    Scope, ScopeRef, Widget,
    components::{offset, opacity},
    layout::Align,
    style::{StyleExt, surface_tertiary},
    unit::Unit,
    widget::{
        Button, ButtonStyle, IterWidgetCollection, Text, card, col,
        interactive::{
            base::InteractiveWidget,
            overlay::{Overlay, OverlayHandle},
        },
        label, maximized, row,
    },
};

pub struct ContextMenu {
    pub items: Vec<ContextMenuItem>,
}

impl ContextMenu {
    pub fn new(items: Vec<ContextMenuItem>) -> Self {
        Self { items }
    }
}

pub struct ContextMenuItem {
    pub icon: Text,
    pub label: String,
    pub action: Box<dyn Fn(&ScopeRef) -> anyhow::Result<()> + Send + Sync>,
}

impl ContextMenuItem {
    pub fn new(
        icon: Text,
        label: impl Into<String>,
        action: impl 'static + Send + Sync + Fn(&ScopeRef) -> anyhow::Result<()>,
    ) -> Self {
        Self {
            icon: icon.into(),
            label: label.into(),
            action: Box::new(action),
        }
    }
}

impl std::fmt::Debug for ContextMenuItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContextMenuItem")
            .field("icon", &self.icon)
            .field("label", &self.label)
            .finish_non_exhaustive()
    }
}

pub struct ContextMenuPanel {
    position: Vec2,
    menu: ContextMenu,
}

impl ContextMenuPanel {
    pub fn new(position: Vec2, menu: ContextMenu) -> Self {
        Self { position, menu }
    }
}

impl Overlay for ContextMenuPanel {
    fn create(self, scope: &mut Scope<'_>, token: OverlayHandle) {
        let token = scope.store(token);

        let menu = |scope: &mut Scope| {
            scope
                .set(offset(), Unit::px(self.position))
                .set(opacity(), 0.95);
            card(
                col(IterWidgetCollection::new(self.menu.items.into_iter().map(
                    |item| {
                        Button::new(
                            row((item.icon, label(item.label))).with_cross_align(Align::Center),
                        )
                        .with_style(ButtonStyle::selectable_entry())
                        .on_click(move |scope| {
                            if let Err(err) = (item.action)(scope) {
                                tracing::error!("Error executing context menu action: {:?}", err);
                            }

                            scope.read(token).close()
                        })
                    },
                )))
                .with_stretch(true),
            )
            .with_background(surface_tertiary())
            .mount(scope);
        };

        InteractiveWidget::new(maximized(menu).with_background(Srgba::new(0.0, 0.0, 0.0, 0.0)))
            .on_mouse_input(move |scope, input| {
                scope.read(token).close();
                Some(input)
            })
            .mount(scope)
    }
}
