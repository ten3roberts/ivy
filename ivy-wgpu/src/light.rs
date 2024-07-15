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
