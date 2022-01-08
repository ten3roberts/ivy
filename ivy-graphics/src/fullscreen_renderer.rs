use crate::{Error, Renderer, Result};
use ash::vk::DescriptorSet;
use ivy_resources::Resources;
use ivy_vulkan::{context::SharedVulkanContext, shaderpass::ShaderPass, PassInfo, Pipeline};
use once_cell::sync::OnceCell;

// Renders a fullscreen quad using the supplied shader pass and descriptors
pub struct FullscreenRenderer {
    pipeline: OnceCell<Pipeline>,
}

impl FullscreenRenderer {
    pub fn new() -> Self {
        Self {
            pipeline: OnceCell::new(),
        }
    }
}

impl Renderer for FullscreenRenderer {
    type Error = Error;
    fn draw<Pass: ShaderPass>(
        &mut self,
        _world: &mut hecs::World,
        resources: &Resources,
        cmd: &ivy_vulkan::commands::CommandBuffer,
        sets: &[DescriptorSet],
        pass_info: &PassInfo,
        offsets: &[u32],
        _current_frame: usize,
    ) -> Result<()> {
        let pipeline = self.pipeline.get_or_try_init(|| {
            let context = resources.get_default::<SharedVulkanContext>()?;
            let pass = resources.get_default::<Pass>()?;
            Pipeline::new::<()>(context.clone(), pass.pipeline(), pass_info)
        })?;

        cmd.bind_pipeline(pipeline);

        if !sets.is_empty() {
            cmd.bind_descriptor_sets(pipeline.layout(), 0, sets, offsets);
        }

        cmd.draw(3, 1, 0, 0);

        Ok(())
    }
}
