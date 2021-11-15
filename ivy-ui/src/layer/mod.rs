#![allow(non_snake_case)]
use anyhow::Context;
use flume::Receiver;
use glfw::WindowEvent;
use hecs::World;
use ivy_base::{Events, Layer, Size2D};
use ivy_graphics::Window;
use ivy_resources::Resources;

use crate::Canvas;
mod event_handling;
use event_handling::*;

/// UI abstraction layer.
pub struct UILayer {
    rx: Receiver<WindowEvent>,
    state: InteractiveState,
}

impl UILayer {
    pub fn new(_world: &mut World, _resources: &mut Resources, events: &mut Events) -> Self {
        let (tx, rx) = flume::unbounded();

        events.subscribe(tx);

        Self {
            rx,
            state: InteractiveState::default(),
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
        );

        Ok(())
    }
}
