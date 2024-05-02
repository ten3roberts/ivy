use ivy_assets::Asset;

use crate::PipelineInfo;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shader {
    pub pipeline_info: Asset<PipelineInfo>,
}

impl Shader {
    pub fn new(pipeline_info: Asset<PipelineInfo>) -> Self {
        Self { pipeline_info }
    }
}
