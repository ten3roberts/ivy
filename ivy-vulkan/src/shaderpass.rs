use ivy_resources::Handle;

use crate::PipelineInfo;

#[derive(Debug, Copy, Clone)]
pub struct Shader {
    pub pipeline_info: Handle<PipelineInfo>,
}
