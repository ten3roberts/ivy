use std::{
    fmt::Display,
    ops::{Deref, DerefMut},
    time::{Duration, Instant},
};

use anyhow::Context;
use flax::{Schedule, ScheduleBuilder, World};
use ivy_assets::AssetCache;

use crate::{
    app::{PostInitEvent, TickEvent},
    components::{delta_time, engine},
    impl_for_tuples,
    layer::events::EventRegisterContext,
    Layer,
};

pub enum TimeStepKind {
    PerTick,
    FixedTimeStep,
}

/// A plugin is added to a layer and allows logic to be added using the ECS
///
/// For full control of events and update frequency, use [crate::layer::Layer].
pub trait Plugin {
    fn install(
        &self,
        world: &mut World,
        assets: &AssetCache,
        schedules: &mut ScheduleSetBuilder,
    ) -> anyhow::Result<()>;
}

impl<U: Plugin> Plugin for Box<U> {
    fn install(
        &self,
        world: &mut World,
        assets: &AssetCache,
        schedules: &mut ScheduleSetBuilder,
    ) -> Result<(), anyhow::Error> {
        (**self).install(world, assets, schedules)
    }
}

pub trait TimeStep: 'static + Display + Copy {
    fn step(&mut self, world: &mut World, schedule: &mut Schedule) -> anyhow::Result<()>;
}

#[derive(Debug, Clone, Copy)]
pub struct PerTick {
    current_time: Instant,
}

impl TimeStep for PerTick {
    fn step(&mut self, world: &mut World, schedule: &mut Schedule) -> anyhow::Result<()> {
        let new_time = Instant::now();
        let elapsed = new_time.duration_since(self.current_time);

        self.current_time = new_time;

        world.set(engine(), delta_time(), elapsed)?;
        schedule.execute_seq(world)?;
        world.set(engine(), delta_time(), Duration::ZERO)?;

        Ok(())
    }
}

impl Display for PerTick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("PerTick").finish()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Startup;

impl TimeStep for Startup {
    fn step(&mut self, world: &mut World, schedule: &mut Schedule) -> anyhow::Result<()> {
        world.set(engine(), delta_time(), Duration::ZERO)?;
        schedule.execute_seq(world)?;

        Ok(())
    }
}

impl Display for Startup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("PerTick").finish()
    }
}

#[derive(Debug, Clone, Copy)]
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

        world.set(
            engine(),
            delta_time(),
            Duration::from_secs_f64(self.delta_time),
        )?;

        if self.acc > self.delta_time {
            // while self.acc > self.delta_time {
            schedule.execute_seq(world)?;
            self.acc -= self.delta_time;
        }

        world.set(engine(), delta_time(), Duration::ZERO)?;

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

pub struct TimeStepSchedule<T> {
    schedule: Schedule,
    time_step: T,
}

impl<T: TimeStep> TimeStepSchedule<T> {
    fn step(&mut self, world: &mut World) -> anyhow::Result<()> {
        self.time_step.step(world, &mut self.schedule)
    }
}

pub struct TimeStepScheduleBuilder<T> {
    schedule: ScheduleBuilder,
    time_step: T,
}

impl<T> Deref for TimeStepScheduleBuilder<T> {
    type Target = ScheduleBuilder;

    fn deref(&self) -> &Self::Target {
        &self.schedule
    }
}

impl<T> DerefMut for TimeStepScheduleBuilder<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.schedule
    }
}

impl<T: TimeStep> TimeStepScheduleBuilder<T> {
    pub fn new(time_step: T) -> Self {
        Self {
            schedule: Schedule::builder(),
            time_step,
        }
    }

    fn build(&mut self) -> TimeStepSchedule<T> {
        TimeStepSchedule {
            schedule: self.schedule.build(),
            time_step: self.time_step,
        }
    }

    pub fn time_step(&self) -> &T {
        &self.time_step
    }
}

pub struct ScheduleSetBuilder {
    per_tick: TimeStepScheduleBuilder<PerTick>,
    fixed: TimeStepScheduleBuilder<FixedTimeStep>,
    startup: TimeStepScheduleBuilder<Startup>,
}

impl ScheduleSetBuilder {
    pub fn new(fixed_timestep: FixedTimeStep) -> Self {
        Self {
            per_tick: TimeStepScheduleBuilder::new(PerTick {
                current_time: Instant::now(),
            }),
            fixed: TimeStepScheduleBuilder::new(fixed_timestep),
            startup: TimeStepScheduleBuilder::new(Startup),
        }
    }

    pub fn build(&mut self) -> ScheduleSet {
        ScheduleSet {
            per_tick: self.per_tick.build(),
            fixed_timestep: self.fixed.build(),
            startup: Some(self.startup.build()),
        }
    }

    pub fn fixed_mut(&mut self) -> &mut TimeStepScheduleBuilder<FixedTimeStep> {
        &mut self.fixed
    }

    pub fn per_tick_mut(&mut self) -> &mut TimeStepScheduleBuilder<PerTick> {
        &mut self.per_tick
    }

    pub fn startup_mut(&mut self) -> &mut TimeStepScheduleBuilder<Startup> {
        &mut self.startup
    }
}

pub struct ScheduleSet {
    per_tick: TimeStepSchedule<PerTick>,
    fixed_timestep: TimeStepSchedule<FixedTimeStep>,
    startup: Option<TimeStepSchedule<Startup>>,
}

impl ScheduleSet {
    pub fn per_tick_mut(&mut self) -> &mut TimeStepSchedule<PerTick> {
        &mut self.per_tick
    }

    pub fn fixed_timestep_mut(&mut self) -> &mut TimeStepSchedule<FixedTimeStep> {
        &mut self.fixed_timestep
    }

    pub fn startup_mut(&mut self) -> &mut Option<TimeStepSchedule<Startup>> {
        &mut self.startup
    }
}

/// Executes a schedule using the provided time step
pub struct ScheduledLayer {
    builder: ScheduleSetBuilder,
    schedules: Option<ScheduleSet>,
    plugins: Vec<Box<dyn Plugin>>,
}

impl ScheduledLayer {
    pub fn new(fixed_timestep: FixedTimeStep) -> Self {
        Self {
            builder: ScheduleSetBuilder::new(fixed_timestep),
            schedules: None,
            plugins: Vec::new(),
        }
    }

    pub fn with_plugin(mut self, plugin: impl 'static + Plugin) -> Self {
        self.plugins.push(Box::new(plugin));
        self
    }

    pub fn register(&mut self, world: &mut World, assets: &AssetCache) -> anyhow::Result<()> {
        assert!(self.schedules.is_none());

        for plugin in &self.plugins {
            plugin.install(world, assets, &mut self.builder)?;
        }

        self.schedules = Some(self.builder.build());

        Ok(())
    }

    pub fn tick(&mut self, world: &mut World) -> anyhow::Result<()> {
        let Some(schedules) = &mut self.schedules else {
            return Ok(());
        };

        if let Some(mut startup) = schedules.startup.take() {
            startup
                .step(world)
                .context("Failed to execute startup schedule")?;
        }

        schedules.fixed_timestep.step(world).with_context(|| {
            format!(
                "Failed to execute schedule {}",
                schedules.fixed_timestep.time_step
            )
        })?;

        schedules.per_tick.step(world).with_context(|| {
            format!(
                "Failed to execute schedule {}",
                schedules.per_tick.time_step
            )
        })?;

        Ok(())
    }
}

impl Layer for ScheduledLayer {
    fn register(
        &mut self,
        _: &mut World,
        _: &AssetCache,
        mut events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        events.subscribe(|this, world, assets, _: &PostInitEvent| this.register(world, assets));
        events.subscribe(|this, world, _, _: &TickEvent| this.tick(world));

        Ok(())
    }
}
