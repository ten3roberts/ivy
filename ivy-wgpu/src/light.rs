use futures::stream::PollNext;
use ivy_core::palette::{Srgb, Srgba};

use crate::renderer::LightData;

pub struct PointLight {
    pub color: Srgb,
    pub intensity: f32,
}

impl PointLight {
    pub fn new(color: Srgb, intensity: f32) -> Self {
        Self { color, intensity }
    }
}
