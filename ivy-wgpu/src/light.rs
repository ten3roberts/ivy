use ivy_assets::{loadable::ResourceDesc, AssetCache};
use ivy_core::{palette::Srgb, template::BundleDesc, Bundle};

use crate::components::{cast_shadow, light_kind, light_params};

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LightParams {
    pub color: Srgb,
    pub intensity: f32,
    pub inner_theta: f32,
    pub outer_theta: f32,
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
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LightKind {
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

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LightBundleDesc {
    pub params: LightParams,
    pub kind: LightKind,
    pub cast_shadow: bool,
}

impl ResourceDesc for LightBundleDesc {
    type Output = LightBundle;

    type Error = anyhow::Error;

    async fn load(&self, _assets: &AssetCache) -> anyhow::Result<Self::Output> {
        Ok(LightBundle {
            params: self.params.clone(),
            kind: self.kind,
            cast_shadow: self.cast_shadow,
        })
    }
}

#[typetag::serde]
impl BundleDesc for LightBundleDesc {}
