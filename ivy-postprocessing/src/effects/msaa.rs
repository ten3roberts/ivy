#[derive(Debug, Clone)]
pub struct MsaaConfig {
    pub sample_count: u32,
}

impl Default for MsaaConfig {
    fn default() -> Self {
        Self { sample_count: 4 }
    }
}