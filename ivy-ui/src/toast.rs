use std::{char::ToUppercase, time::Duration};

use flax::{component, Component};
use glam::{vec2, BVec2};
use ivy_core::{
    components::engine,
    plugin::{Plugin, PluginContext},
};
use violet::{
    core::{
        components::{rotation, translation, LayoutAlignment},
        layout::Align,
        style::{
            base_colors::ZINC_700, element_accent, element_danger, element_warning, spacing_large,
            spacing_small, text_medium, SizeExt, StyleExt,
        },
        time::sleep,
        tweens::{tween::Tweener, ComponentTween},
        unit::Unit,
        widget::{
            self, bold, card, col, label, row, AnimateLifecycle, ScrollArea, Text, TextStyle,
        },
        Scope, Widget,
    },
    lucide::icons::{LUCIDE_CIRCLE_X, LUCIDE_INFO, LUCIDE_TRIANGLE_ALERT},
    palette::Srgba,
};

use crate::screens::{screen_state, Screen};

component! {
    pub toast_state: ToastState,
}

violet::core::declare_atom! {
    pub toasts: ToastState,
}

#[derive(Clone)]
pub struct ToastState {
    toasts_tx: flume::Sender<Toast>,
}

impl ToastState {
    pub fn send(&self, toast: Toast) {
        let _ = self.toasts_tx.send(toast);
    }
}

pub enum ToastLevel {
    Info,
    Warning,
    Error,
}

impl ToastLevel {
    pub fn color(&self) -> Component<Srgba> {
        match self {
            ToastLevel::Info => element_accent(),
            ToastLevel::Warning => element_warning(),
            ToastLevel::Error => element_danger(),
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            ToastLevel::Info => LUCIDE_INFO,
            ToastLevel::Warning => LUCIDE_TRIANGLE_ALERT,
            ToastLevel::Error => LUCIDE_CIRCLE_X,
        }
    }
}

pub const MAX_TOAST_WIDTH: f32 = 600.0;
pub const MAX_TOAST_HEIGHT: f32 = 300.0;

pub struct Toast {
    title: Text,
    body: Text,
    level: ToastLevel,
}

impl Toast {
    pub fn info(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self::new(title, body, ToastLevel::Info)
    }

    pub fn warning(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self::new(title, body, ToastLevel::Warning)
    }

    pub fn error(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self::new(title, body, ToastLevel::Error)
    }

    pub fn new(title: impl Into<String>, body: impl Into<String>, level: ToastLevel) -> Self {
        Self {
            title: widget::bold(title.into()).with_style(TextStyle::default()),
            body: label(body),
            level,
        }
    }
}

impl Widget for Toast {
    fn mount(self, scope: &mut Scope<'_>) {
        scope.set_default(translation()).set_default(rotation());

        AnimateLifecycle::new(
            card(
                col((
                    row((
                        Text::medium(self.level.icon())
                            .with_margin(spacing_small())
                            .with_font_size(text_medium())
                            .with_color(self.level.color()),
                        self.title,
                    ))
                    .center(),
                    ScrollArea::new(BVec2::TRUE, self.body)
                        .with_max_size(Unit::px2(MAX_TOAST_WIDTH, MAX_TOAST_HEIGHT)),
                ))
                .with_stretch(true),
            )
            .with_background(ZINC_700)
            .with_margin(spacing_large())
            .with_padding(spacing_large()),
            ComponentTween::new(
                translation(),
                Tweener::elastic_out(vec2(MAX_TOAST_WIDTH + 10.0, 0.0), vec2(0.0, 0.0), 0.8),
            ),
            ComponentTween::new(
                translation(),
                Tweener::elastic_in(vec2(0.0, 0.0), vec2(MAX_TOAST_WIDTH + 10.0, 0.0), 0.8),
            ),
        )
        .mount(scope);
    }
}

pub struct ToastScreen {
    toasts_rx: flume::Receiver<Toast>,
    toasts_tx: flume::Sender<Toast>,
}

impl ToastScreen {
    pub fn new(toasts_rx: flume::Receiver<Toast>, toasts_tx: flume::Sender<Toast>) -> Self {
        Self {
            toasts_rx,
            toasts_tx,
        }
    }
}

impl Screen for ToastScreen {
    fn create(self, scope: &mut Scope<'_>, _: crate::screens::ScreenLifetimeToken) {
        scope.set_atom(
            toasts(),
            ToastState {
                toasts_tx: self.toasts_tx.clone(),
            },
        );

        scope.spawn_stream(self.toasts_rx.into_stream(), |scope, toast| {
            let toast_id = scope.attach(toast);

            scope.spawn_future(sleep(Duration::from_secs(5)), move |scope, _| {
                scope.detach(toast_id);
            });
        });

        let toasts_tx = self.toasts_tx.clone();
        col(())
            .with_cross_align(Align::End)
            .with_contain_margins(true)
            .with_item_align(LayoutAlignment::top_right())
            .mount(scope);
    }
}

/// Provides toast notification functionality.
pub struct ToastPlugin;

impl Plugin for ToastPlugin {
    fn install(&self, ctx: &mut PluginContext) -> anyhow::Result<()> {
        let (toasts_tx, toasts_rx) = flume::unbounded();

        ctx.world
            .get(engine(), screen_state())?
            .open(ToastScreen::new(toasts_rx, toasts_tx.clone()));

        toasts_tx
            .send(Toast::info(
                "Welcome to Ivy!",
                "This is a toast notification. It will disappear after a few seconds.",
            ))
            .ok();

        ctx.world
            .set(engine(), toast_state(), ToastState { toasts_tx });

        Ok(())
    }
}
