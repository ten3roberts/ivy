use ivy_assets::Resource;
use ivy_core::{palette::Srgb, Bundle};
use ivy_editable::Editable;
use violet::core::{state::StateExt, Widget};

use crate::components::{cast_shadow, light_kind, light_params};

#[derive(Debug, Clone, Editable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LightParams {
    pub color: Srgb,
    /// Light intensity
    #[editable(range(0.0, 100.0))]
    pub intensity: f32,
    /// Spotlight inner cone radius
    #[editable(range(0.0, 1.5708))]
    pub inner_theta: f32,
    /// Spotlight outer cone radius
    #[editable(range(0.0, 1.5708))]
    pub outer_theta: f32,
}

impl Default for LightParams {
    fn default() -> Self {
        Self {
            color: Srgb::new(1.0, 1.0, 1.0),
            intensity: 1.0,
            inner_theta: Default::default(),
            outer_theta: Default::default(),
        }
    }
}

impl LightParams {
    pub fn new(color: Srgb, intensity: f32) -> Self {
        Self {
            color,
            intensity,
            inner_theta: 1.0,
            outer_theta: 1.0,
        }
    }

    pub fn with_angular_cutoffs(mut self, inner_theta: f32, outer_theta: f32) -> Self {
        self.inner_theta = inner_theta;
        self.outer_theta = outer_theta;
        self
    }
}

#[repr(u32)]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Editable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LightKind {
    #[default]
    Point,
    Directional,
    Spotlight,
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

    /// Returns `true` if the light kind is [`Spotlight`].
    ///
    /// [`Spotlight`]: LightKind::Spotlight
    #[must_use]
    pub fn is_spotlight(&self) -> bool {
        matches!(self, Self::Spotlight)
    }
}

#[derive(Debug, Clone, Resource, Editable, Bundle)]
#[resource(derive = [Editable])]
pub struct LightBundle {
    pub params: LightParams,
    pub kind: LightKind,
    pub cast_shadow: bool,
}

impl Bundle for LightBundle {
    fn mount(&self, entity: &mut flax::EntityBuilder) {
        entity
            .set(light_params(), self.params.clone())
            .set(light_kind(), self.kind);

        if self.cast_shadow {
            entity.set(cast_shadow(), ());
        }
    }
}
