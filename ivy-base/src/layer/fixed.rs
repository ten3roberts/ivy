use std::time::Duration;

use flax::World;

use crate::Layer;

const MAX_ITERATIONS: f64 = 10.0;

/// Abstracts a layer executing other layers at a fixed timestep.
pub struct FixedTimeStep<T: Layer> {
    timestep: Duration,
    layers: T,
    acc: f64,
}

impl<T: Layer> FixedTimeStep<T> {
    pub fn new(timestep: Duration, layers: T) -> Self {
        Self {
            timestep,
            layers,
            acc: 0.0,
        }
    }
}

impl<T: Layer> Layer for FixedTimeStep<T> {
    fn on_update(
        &mut self,
        world: &mut World,
        resources: &mut ivy_resources::Resources,
        events: &mut crate::Events,
        frame_time: Duration,
    ) -> anyhow::Result<()> {
        let ft_s = frame_time.as_secs_f64();

        let dt = self.timestep.as_secs_f64();

        self.acc = (self.acc + ft_s).min(dt * MAX_ITERATIONS);

        while self.acc > 0.0 {
            self.layers
                .on_update(world, resources, events, self.timestep)?;

            self.acc -= dt;
        }

        Ok(())
    }
}
