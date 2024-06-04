
use crate::{Renderer};
use ash::vk::DescriptorSet;
use flax::{Component, World};
use ivy_assets::AssetCache;
use ivy_vulkan::{
    context::{VulkanContextService},
    PassInfo, Pipeline, PipelineInfo, Shader,
};

// Renders a fullscreen quad using the supplied shader pass and descriptors
pub struct FullscreenRenderer {
    pipeline: Option<Pipeline>,
    pipeline_info: PipelineInfo,
}

impl FullscreenRenderer {
    pub fn new(pipeline_info: PipelineInfo) -> Self {
        Self {
            pipeline: None,
            pipeline_info,
        }
    }
}

impl Renderer for FullscreenRenderer {
    fn draw(
        &mut self,
        _world: &mut World,
        assets: &AssetCache,
        cmd: &ivy_vulkan::commands::CommandBuffer,
        sets: &[DescriptorSet],
        pass_info: &PassInfo,
        offsets: &[u32],
        _current_frame: usize,
        _: Component<Shader>,
    ) -> anyhow::Result<()> {
        let pipeline = match &mut self.pipeline {
            Some(v) => v,
            None => {
                let context = assets.service::<VulkanContextService>().context();
                let val = Pipeline::new::<()>(context.clone(), &self.pipeline_info, pass_info)?;

                self.pipeline.insert(val)
            }
        };

        cmd.bind_pipeline(pipeline.pipeline());

        if !sets.is_empty() {
            cmd.bind_descriptor_sets(pipeline.layout(), 0, sets, offsets);
        }

        cmd.draw(3, 1, 0, 0);

        Ok(())
    }
}
