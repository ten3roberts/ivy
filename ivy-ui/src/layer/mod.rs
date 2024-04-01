#![allow(non_snake_case)]
use anyhow::Context;
use flax::{BoxedSystem, Query, Schedule, System, World};
use glam::Vec2;
use ivy_base::{size, Events, Layer};
use ivy_input::InputEvent;
use ivy_resources::Resources;
use ivy_window::Window;

use crate::{canvas, handle_events, input_field_system, systems, Canvas, UIControl};
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
    pub fn new(
        _world: &mut World,
        _resources: &mut Resources,
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
            .with_system(move |w: SubWorld<_>, state: Read<_>, events: Write<_>| {
                input_field_system(w, state, input_field_rx.try_iter(), events)
            })
            .build();

        eprintln!("UI Layer: {}", schedule.batch_info());

        Ok(Self {
            state: InteractiveState::default(),
            schedule,
        })
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
        let size = Query::new(size())
            .with(canvas())
            .borrow(world)
            .iter()
            .next()
            .context("No canvas")?;
        // let (_, (_, size)) = world
        //     .query_mut::<(&Canvas, &Size2D)>()
        //     .into_iter()
        //     .next()
        //     .context("Failed to get canvas")?;

        let mut cursor_pos = cursor_pos * *size;

        self.schedule
            .execute_seq((world, events, &mut self.state, &mut cursor_pos))
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
            |world: &mut World,
             events: &mut Events,
             state: &mut InteractiveState,
             cursor_pos: &Vec2| {
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
