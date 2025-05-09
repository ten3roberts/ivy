use std::{
    any,
    collections::BTreeMap,
    fmt::Display,
    mem,
    ops::{Deref, DerefMut},
    time::{Duration, Instant},
};

use anyhow::Context;
use flax::{Schedule, ScheduleBuilder, World};
use ivy_assets::AssetCache;

use crate::{
    app::{PostInitEvent, TickEvent},
    components::{delta_time, elapsed_time, engine},
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
    // Installs the plugin to the schedule set
    fn install(
        &self,
        world: &mut World,
        assets: &AssetCache,
        schedules: &mut ScheduleSetBuilder,
    ) -> anyhow::Result<()>;

    fn key(&self) -> &'static str {
        any::type_name::<Self>()
    }

    // Plugin runs before other plugin
    fn before(&self) -> Vec<&str> {
        Vec::new()
    }

    // Plugin runs after another plugin
    fn after(&self) -> Vec<&str> {
        Vec::new()
    }
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
    elapsed: Duration,
}

impl TimeStep for PerTick {
    fn step(&mut self, world: &mut World, schedule: &mut Schedule) -> anyhow::Result<()> {
        let new_time = Instant::now();
        let dt = new_time.duration_since(self.current_time);

        self.current_time = new_time;
        self.elapsed += dt;

        world.set(engine(), delta_time(), dt)?;
        world.set(engine(), elapsed_time(), self.elapsed)?;
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
        world.set(engine(), elapsed_time(), Duration::ZERO)?;
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
    elapsed: Duration,
}

impl FixedTimeStep {
    pub fn new(dt: f64) -> Self {
        Self {
            delta_time: dt,
            current_time: Instant::now(),
            acc: 0.0,
            elapsed: Duration::ZERO,
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
            world.set(engine(), elapsed_time(), self.elapsed)?;
            // while self.acc > self.delta_time {
            schedule.execute_seq(world)?;

            self.elapsed += Duration::from_secs_f64(self.delta_time);
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
                elapsed: Duration::ZERO,
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

        let plugins = mem::take(&mut self.plugins);
        for plugin in Self::sort_plugins(&plugins)? {
            plugin.install(world, assets, &mut self.builder)?;
        }

        self.schedules = Some(self.builder.build());

        Ok(())
    }

    fn collect_dependencies(
        plugins: &[Box<dyn Plugin>],
    ) -> anyhow::Result<BTreeMap<String, String>> {
        let mut dependencies = BTreeMap::new();

        for plugin in plugins {
            let name = plugin.key();
            let before = plugin.before();
            let after = plugin.after();

            tracing::info!(?name, ?before, ?after);

            for b in before {
                dependencies.insert(b.to_string(), name.to_string());
            }

            for a in after {
                dependencies.insert(name.to_string(), a.to_string());
            }
        }

        Ok(dependencies)
    }

    // Sort plugins based on dependencies using topological sort
    fn sort_plugins(plugins: &[Box<dyn Plugin>]) -> anyhow::Result<Vec<&dyn Plugin>> {
        fn visit<'a>(
            plugin: &'a dyn Plugin,
            dependencies: &BTreeMap<String, String>,
            sorted: &mut Vec<&'a dyn Plugin>,
            visited: &mut BTreeMap<String, bool>,
        ) -> anyhow::Result<()> {
            let name = plugin.key();
            let before = plugin.before();
            let after = plugin.after();

            if let Some(visited) = visited.get(name) {
                if *visited {
                    return Ok(());
                }
            }

            visited.insert(name.to_string(), false);

            for b in before {
                if let Some(dependency) = dependencies.get(b) {
                    if !visited.get(dependency).unwrap_or(&false) {
                        return Err(anyhow::anyhow!(
                            "Plugin {} has a circular dependency with {}",
                            name,
                            dependency
                        ));
                    }

                    visit(plugin, dependencies, sorted, visited)?;
                } else {
                    return Err(anyhow::anyhow!(
                        "Plugin {} has a dependency on {} which does not exist",
                        name,
                        b
                    ));
                }
            }

            sorted.push(plugin);
            visited.insert(name.to_string(), true);

            for a in after {
                if let Some(dependency) = dependencies.get(a) {
                    if !visited.get(dependency).unwrap_or(&false) {
                        return Err(anyhow::anyhow!(
                            "Plugin {} has a circular dependency with {}",
                            name,
                            dependency
                        ));
                    }

                    visit(plugin, dependencies, sorted, visited)?;
                }
            }

            Ok(())
        }

        let dependencies = Self::collect_dependencies(plugins)?;

        let mut sorted = Vec::new();
        let mut visited = BTreeMap::new();

        for plugin in plugins {
            visit(&**plugin, &dependencies, &mut sorted, &mut visited)?;
        }

        Ok(sorted)
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
        events.subscribe(|this, ctx, _: &PostInitEvent| this.register(ctx.world, ctx.assets));
        events.subscribe(|this, ctx, _: &TickEvent| this.tick(ctx.world));

        Ok(())
    }
}
