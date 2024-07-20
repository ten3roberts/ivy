use ivy_core::palette::Srgb;

pub struct LightData {
    pub color: Srgb,
    pub intensity: f32,
}

impl LightData {
    pub fn new(color: Srgb, intensity: f32) -> Self {
        Self { color, intensity }
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy)]
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
