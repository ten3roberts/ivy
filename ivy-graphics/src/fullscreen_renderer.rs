use crate::{Renderer, Result, ShaderPass};
use ash::vk::DescriptorSet;
use ivy_resources::Resources;

// Renders a fullscreen quad using the supplied shader pass and descriptors
pub struct FullscreenRenderer;

impl Renderer for FullscreenRenderer {
    fn draw<Pass: 'static + ShaderPass + Sized + Sync + Send>(
        &mut self,
        // The ecs world
        _world: &mut hecs::World,
        // The commandbuffer to record into
        cmd: &ivy_vulkan::commands::CommandBuffer,
        // The current swapchain image or backbuffer index
        _current_frame: usize,
        // Descriptor sets to bind before renderer specific sets
        sets: &[DescriptorSet],
        // Dynamic offsets for supplied sets
        offsets: &[u32],
        // Graphics resources like textures and materials
        resources: &Resources,
    ) -> Result<()> {
        let pass = resources.get_default::<Pass>()?;

        cmd.bind_pipeline(pass.pipeline());

        if !sets.is_empty() {
            cmd.bind_descriptor_sets(pass.pipeline_layout(), 0, sets, offsets);
        }

        cmd.draw(3, 1, 0, 0);

        Ok(())
    }
}
