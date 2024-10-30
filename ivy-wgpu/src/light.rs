use ivy_core::{palette::Srgb, Bundle};

use crate::components::{light_kind, light_params};

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LightParams {
    pub color: Srgb,
    pub intensity: f32,
}

impl LightParams {
    pub fn new(color: Srgb, intensity: f32) -> Self {
        Self { color, intensity }
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LightKind {
    Point,
    Directional,
}

impl LightKind {
    /// Returns `true` if the light kind is [`Directional`].
    ///
    /// [`Directional`]: LightKind::Directional
    #[must_use]
    pub fn is_directional(&self) -> bool {
        matches!(self, Self::Directional)
    }

    /// Returns `true` if the light kind is [`Point`].
    ///
    /// [`Point`]: LightKind::Point
    #[must_use]
    pub fn is_point(&self) -> bool {
        matches!(self, Self::Point)
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LightBundle {
    pub params: LightParams,
    pub kind: LightKind,
}

impl Bundle for LightBundle {
    fn mount(self, entity: &mut flax::EntityBuilder) {
        entity
            .set(light_params(), self.params)
            .set(light_kind(), self.kind);
    }
}
