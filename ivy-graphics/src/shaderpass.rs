use ash::vk::PipelineLayout;
use ivy_vulkan::Pipeline;

/// Represents a single pass containing the pipeline and other data. Since
/// [Material](crate::Material) does not
/// contain a pipeline, a `ShaderPass` can be considered a master material.
pub trait ShaderPass {
    /// Returns the pipeline used for this shaderpass.
    fn pipeline(&self) -> &Pipeline;

    /// Returns the pipeline layout used for this shaderpass.
    fn pipeline_layout(&self) -> PipelineLayout;
}
