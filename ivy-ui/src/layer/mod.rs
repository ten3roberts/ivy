#![allow(non_snake_case)]
use anyhow::Context;
use flume::Receiver;
use glfw::{Key, WindowEvent};
use hecs::World;
use ivy_base::{Events, Layer, Size2D};
use ivy_graphics::Window;
use ivy_resources::Resources;

use crate::{events::WidgetEvent, handle_events, input_field_system, systems, Canvas};
mod event_handling;
pub use event_handling::*;

pub struct UILayerInfo {
    /// Universal key to unfocus a focused widget
    pub unfocus_key: Option<Key>,
}

/// UI abstraction layer.
/// Handles raw input events and filters them through the UI system, and then
/// through the world in the form of [`InputEvent`]s.
pub struct UILayer {
    rx: Receiver<WindowEvent>,
    input_field_events: Receiver<WidgetEvent>,
    state: InteractiveState,
    unfocus_key: Key,
}

impl UILayer {
    pub fn new(
        _world: &mut World,
        _resources: &mut Resources,
        events: &mut Events,
        info: UILayerInfo,
    ) -> Self {
        let (tx, rx) = flume::unbounded();

        events.subscribe(tx);
        let (tx, input_field_events) = flume::unbounded();
        events.subscribe(tx);

        Self {
            rx,
            input_field_events,
            state: InteractiveState::default(),
            unfocus_key: info.unfocus_key.unwrap_or(Key::Unknown),
        }
    }
}

impl Layer for UILayer {
    fn on_update(
        &mut self,
        world: &mut World,
        resources: &mut Resources,
        events: &mut Events,
        _frame_time: std::time::Duration,
    ) -> anyhow::Result<()> {
        systems::update(world)?;

        let window = resources.get_default::<Window>()?;
        // Transform the cursor position to canvas size
        let cursor_pos = window.normalized_cursor_pos();
        let (_, (_, size)) = world
            .query_mut::<(&Canvas, &Size2D)>()
            .into_iter()
            .next()
            .context("Failed to get canvas")?;

        let cursor_pos = cursor_pos * **size;

        handle_events(
            world,
            events,
            self.rx.try_iter(),
            cursor_pos.into(),
            &mut self.state,
            self.unfocus_key,
        );

        if let Some(active) = self.state.focused() {
            input_field_system(world, self.input_field_events.try_iter(), active.id())?;
        }

        Ok(())
    }
}
