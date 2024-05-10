use std::time::Duration;

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

pub struct FixedTimeStepDesc<T>(pub Duration, pub T);
