use std::{fmt::Display, time::Instant};

use anyhow::Context;
use flax::{Schedule, ScheduleBuilder, World};
use ivy_assets::AssetCache;

use crate::{
    app::{InitEvent, TickEvent},
    layer::events::EventRegisterContext,
    Layer,
};

pub trait Plugin<T: TimeStep> {
    fn install(
        &self,
        world: &mut World,
        assets: &AssetCache,
        schedule: &mut ScheduleBuilder,
        time_step: &T,
    ) -> anyhow::Result<()>;
}

impl<T: TimeStep, U> Plugin<T> for Box<U>
where
    U: Plugin<T>,
{
    fn install(
        &self,
        world: &mut World,
        assets: &AssetCache,
        schedule: &mut ScheduleBuilder,
        time_step: &T,
    ) -> Result<(), anyhow::Error> {
        (**self).install(world, assets, schedule, time_step)
    }
}

pub trait TimeStep: 'static + Display {
    fn step(&mut self, world: &mut World, schedule: &mut Schedule) -> anyhow::Result<()>;
}

pub struct PerTick;

impl TimeStep for PerTick {
    fn step(&mut self, world: &mut World, schedule: &mut Schedule) -> anyhow::Result<()> {
        schedule.execute_par(world)
    }
}

impl Display for PerTick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("PerTick").finish()
    }
}

pub struct FixedTimeStep {
    delta_time: f64,
    current_time: Instant,
    acc: f64,
}

impl FixedTimeStep {
    pub fn new(dt: f64) -> Self {
        Self {
            delta_time: dt,
            current_time: Instant::now(),
            acc: 0.0,
        }
    }

    pub fn delta_time(&self) -> f64 {
        self.delta_time
    }
}

impl TimeStep for FixedTimeStep {
    fn step(&mut self, world: &mut World, schedule: &mut Schedule) -> anyhow::Result<()> {
        let now = Instant::now();

        let elapsed = now.duration_since(self.current_time);
        self.current_time = now;

        self.acc += elapsed.as_secs_f64();

        if self.acc > self.delta_time {
            schedule.execute_seq(world)?;
            self.acc -= self.delta_time;
        }

        Ok(())
    }
}

impl Display for FixedTimeStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("FixedTimeStep")
            .field(&self.delta_time)
            .finish()
    }
}
/// Executes a schedule using the provided time step
pub struct ScheduledLayer<T> {
    time_step: T,
    schedule: Option<Schedule>,
    plugins: Vec<Box<dyn Plugin<T>>>,
}

impl<T: TimeStep> ScheduledLayer<T> {
    pub fn new(interval: T) -> Self {
        Self {
            schedule: None,
            time_step: interval,
            plugins: Vec::new(),
        }
    }

    pub fn with_plugin(mut self, plugin: impl 'static + Plugin<T>) -> Self {
        self.plugins.push(Box::new(plugin));
        self
    }

    pub fn register(&mut self, world: &mut World, assets: &AssetCache) -> anyhow::Result<()> {
        assert!(self.schedule.is_none());

        let mut schedule = Schedule::builder();
        for plugin in &self.plugins {
            plugin.install(world, assets, &mut schedule, &self.time_step)?;
        }

        self.schedule = Some(schedule.build());
        Ok(())
    }

    pub fn tick(&mut self, world: &mut World) -> anyhow::Result<()> {
        let Some(schedule) = &mut self.schedule else {
            return Ok(());
        };

        self.time_step
            .step(world, schedule)
            .with_context(|| format!("Failed to execute schedule {}", self.time_step))?;

        Ok(())
    }
}

impl<T: TimeStep> Layer for ScheduledLayer<T> {
    fn register(
        &mut self,
        _: &mut World,
        _: &AssetCache,
        mut events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        events.subscribe(|this, world, assets, _: &InitEvent| this.register(world, assets));
        events.subscribe(|this, world, _, _: &TickEvent| this.tick(world));

        Ok(())
    }
}
