#![allow(non_snake_case)]
use anyhow::Context;
use flax::{BoxedSystem, Query, Schedule, System, World};
use glam::Vec2;
use ivy_assets::AssetCache;
use ivy_base::{engine, size, Events, Layer};
use ivy_graphics::components::window;
use ivy_input::InputEvent;

use crate::{canvas, handle_events, input_field_system, update_system, UIControl};
mod event_handling;
pub use event_handling::*;

/// UI abstraction layer.
/// Handles raw input events and filters them through the UI system, and then
/// through the world in the form of [`ivy-input::InputEvent`]s.
pub struct UILayer {
    state: InteractiveState,
    update_canvas: BoxedSystem,
    schedule: Schedule,
}

impl UILayer {
    pub fn new(
        world: &mut World,
        assets: &AssetCache,
        events: &mut Events,
    ) -> anyhow::Result<Self> {
        let (tx, window_rx) = flume::unbounded();
        events
            .intercept(tx)
            .context("Failed to intercept InputEvent for UI")?;
        let control_rx = events.subscribe();
        let input_field_rx = events.subscribe();

        let schedule = Schedule::builder()
            .with_system(handle_events_system(window_rx, control_rx))
            .with_system(input_field_system(input_field_rx))
            .build();

        Ok(Self {
            state: InteractiveState::default(),
            schedule,
            update_canvas: update_system(),
        })
    }
}

impl Layer for UILayer {
    fn on_update(
        &mut self,
        world: &mut World,
        assets: &mut AssetCache,
        events: &mut Events,
        _frame_time: std::time::Duration,
    ) -> anyhow::Result<()> {
        self.update_canvas
            .run(world)
            .context("Failed to update canvas")?;

        let window = world.get(engine(), window())?;
        // Transform the cursor position to canvas size
        let cursor_pos = window.normalized_cursor_pos();
        let &size = Query::new(size())
            .with(canvas())
            .borrow(world)
            .iter()
            .next()
            .context("No canvas")?;

        drop(window);
        // let (_, (_, size)) = world
        //     .query_mut::<(&Canvas, &Size2D)>()
        //     .into_iter()
        //     .next()
        //     .context("Failed to get canvas")?;

        let mut cursor_pos = cursor_pos * size;

        self.schedule
            .execute_seq_with(world, (events, &mut self.state, &mut cursor_pos))
            .context("Failed to execute UI schedule")?;

        Ok(())
    }
}

fn handle_events_system(
    input_events: flume::Receiver<InputEvent>,
    control_events: flume::Receiver<UIControl>,
) -> BoxedSystem {
    System::builder()
        .with_world_mut()
        .with_input_mut::<Events>()
        .with_input_mut::<InteractiveState>()
        .with_input::<Vec2>()
        .build(
            move |world: &mut World,
                  events: &mut Events,
                  state: &mut InteractiveState,
                  &cursor_pos: &Vec2| {
                handle_events(
                    world,
                    events,
                    state,
                    cursor_pos,
                    input_events.try_iter(),
                    control_events.try_iter(),
                )
            },
        )
        .boxed()
}
