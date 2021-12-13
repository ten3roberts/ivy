#![allow(non_snake_case)]
use anyhow::Context;
use hecs::World;
use hecs_schedule::{Read, Schedule, SubWorld, Write};
use ivy_base::{Events, Layer, Position2D, Size2D};
use ivy_graphics::Window;
use ivy_resources::Resources;

use crate::{handle_events, input_field_system, systems, Canvas};
mod event_handling;
pub use event_handling::*;

/// UI abstraction layer.
/// Handles raw input events and filters them through the UI system, and then
/// through the world in the form of [`ivy-input::InputEvent`]s.
pub struct UILayer {
    state: InteractiveState,
    schedule: Schedule,
}

impl UILayer {
    pub fn new(_world: &mut World, _resources: &mut Resources, events: &mut Events) -> Self {
        let window_rx = events.subscribe_flume();
        let control_rx = events.subscribe_flume();
        let input_field_rx = events.subscribe_flume();

        let schedule = Schedule::builder()
            .add_system(
                move |w: Write<_>, state: Write<_>, events: Write<_>, cursor_pos: Read<_>| {
                    handle_events(
                        w,
                        events,
                        state,
                        cursor_pos,
                        window_rx.try_iter(),
                        control_rx.try_iter(),
                    )
                },
            )
            .add_system(move |w: SubWorld<_>, state: Read<_>| {
                input_field_system(w, state, input_field_rx.try_iter())
            })
            .build();

        eprintln!("UI Layer: {}", schedule.batch_info());

        Self {
            state: InteractiveState::default(),
            schedule,
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

        let mut cursor_pos = Position2D(cursor_pos * **size);

        self.schedule
            .execute((world, events, &mut self.state, &mut cursor_pos))
            .context("Failed to execute UI schedule")?;

        Ok(())
    }
}
