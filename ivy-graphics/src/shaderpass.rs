use ash::vk::PipelineLayout;
use ivy_vulkan::Pipeline;

/// Represents a single pass containing the pipeline and other data. Since
/// [Material](crate::Material) does not
/// contain a pipeline, a `ShaderPass` can be considered a master material.
pub trait ShaderPass: 'static + Send + Sync {
    /// Returns the pipeline used for this shaderpass.
    fn pipeline(&self) -> &Pipeline;

    /// Returns the pipeline layout used for this shaderpass.
    fn pipeline_layout(&self) -> PipelineLayout;
}

/// Macro to create a strongly typed shaderpass.
#[macro_export(local_inner_macros)]
macro_rules! new_shaderpass {
( $(#[$outer:meta])* $vis:vis struct $name:ident; $($rest:tt)* ) => {
$(#[$outer])*
#[repr(transparent)]
$vis struct $name( pub ivy_vulkan::Pipeline );

impl $name {
fn new(pipeline: ivy_vulkan::Pipeline) -> Self {
Self ( pipeline )
}
}

impl $crate::ShaderPass for $name {
fn pipeline(&self) -> &ivy_vulkan::Pipeline {
&self.0
}

fn pipeline_layout(&self) -> vk::PipelineLayout {
self.0.layout()
}
}

$crate::new_shaderpass!($($rest)*);
};

() => {}
}
