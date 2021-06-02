use flume::Receiver;
use rand::prelude::*;
use std::{thread::sleep, time::Duration};

use hecs::World;
use ivy_core::*;
use log::*;
use rand::{prelude::StdRng, SeedableRng};

fn main() -> anyhow::Result<()> {
    // Setup logging
    Logger {
        show_location: false,
        max_level: LevelFilter::Debug,
    }
    .install();

    let mut app = App::builder().push_layer(SandboxLayer::new).build();

    app.run()
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Position {
    x: i32,
    y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Velocity {
    x: i32,
    y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SandboxEvent {
    DummyEvent(usize),
}

struct SandboxLayer {
    frame: usize,
    elapsed: Clock,
    last_status: Clock,

    rx: Receiver<SandboxEvent>,
}

impl SandboxLayer {
    fn new(world: &mut World, events: &mut Events) -> Self {
        info!("Attached sandbox layer");

        let mut rng = StdRng::seed_from_u64(0);
        // Spawn some with velocities
        world.spawn_batch((0..10).map(|_| {
            (
                Position {
                    x: rng.gen_range(-5..5),
                    y: rng.gen_range(-5..5),
                },
                Velocity {
                    x: rng.gen_range(-3..3),
                    y: rng.gen_range(-3..3),
                },
            )
        }));

        // And some without
        world.spawn_batch((0..10).map(|_| {
            (Position {
                x: rng.gen_range(-5..5),
                y: rng.gen_range(-5..5),
            },)
        }));

        // And many unrelated
        world.spawn_batch((0..1_000).map(|_| {
            let name = (0..5).map(|_| rng.gen_range('a'..'z')).collect::<String>();
            (name,)
        }));

        let (tx, rx) = flume::unbounded();
        events.subscribe(tx);

        Self {
            frame: 0,
            elapsed: Clock::new(),
            last_status: Clock::new(),
            rx,
        }
    }
}

impl Layer for SandboxLayer {
    fn on_update(
        &mut self,
        world: &mut World,
        events: &mut Events,
        frame_time: Duration,
    ) -> anyhow::Result<()> {
        // Send dummy events
        events.send(SandboxEvent::DummyEvent(self.frame));

        if self.last_status.elapsed() > 1.secs() {
            self.last_status.reset();
            info!(
                "Updating SandboxLayer. frame: {}, \telapsed: {:?}, \tdt: {:?}",
                self.frame,
                self.elapsed.elapsed(),
                frame_time
            );
        }

        integrate(world);

        let status = world
            .query::<(&Position, Option<&Velocity>)>()
            .iter()
            .map(|(id, val)| format!("  {:?}:\t {:?}\n", id, val))
            .collect::<String>();

        info!("Entities:\n{}", status);

        // Receive events
        for event in self.rx.try_iter() {
            info!("Event: {:?}", event);
        }

        self.frame += 1;

        if self.elapsed.elapsed() > 2.0.secs() {
            events.send(AppEvent::Exit)
        }

        sleep(Duration::from_millis(100));
        Ok(())
    }
}

fn integrate(world: &mut World) {
    world
        .query_mut::<(&mut Position, &Velocity)>()
        .into_iter()
        .for_each(|(_id, (pos, vel))| {
            pos.x += vel.x;
            pos.y += vel.y
        });
}
