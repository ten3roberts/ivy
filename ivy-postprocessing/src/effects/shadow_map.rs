#[derive(Debug, Clone)]
pub struct ShadowMapConfig {
    pub resolution: u32,
    pub max_cascades: u32,
    pub max_shadows: u32,
}

impl Default for ShadowMapConfig {
    fn default() -> Self {
        Self {
            resolution: 2048,
            max_cascades: 1,
            max_shadows: 8,
        }
    }
}