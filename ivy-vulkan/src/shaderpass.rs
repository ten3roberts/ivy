use ivy_resources::Handle;

use crate::PipelineInfo;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Shader {
    pub pipeline_info: Handle<PipelineInfo>,
}
