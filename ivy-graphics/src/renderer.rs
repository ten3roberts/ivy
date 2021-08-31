use std::slice;

use crate::{Result, ShaderPass};
use ash::vk::DescriptorSet;
use hecs::World;
use ivy_resources::Resources;
use ivy_vulkan::commands::CommandBuffer;

// Generic interface for a renderer.
pub trait Renderer {
    // Draws the scene using the pass [`Pass`] and the provided camera.
    // Note: camera must have gpu side data.
    fn draw<Pass: 'static + ShaderPass + Sized + Sync + Send>(
        &mut self,
        // The ecs world
        world: &mut World,
        // The commandbuffer to record into
        cmd: &CommandBuffer,
        // The current swapchain image or backbuffer index
        current_frame: usize,
        // Descriptor sets to bind before renderer specific sets
        sets: &[DescriptorSet],
        // Dynamic offsets for supplied sets
        offsets: &[u32],
        // Graphics resources like textures and materials
        resources: &Resources,
    ) -> Result<()>;
}

// Traits for types holding one or more descriptor sets for use in rendering.
pub trait IntoSet {
    /// Get the descriptor set for the current frame
    fn set(&self, current_frame: usize) -> DescriptorSet {
        *self
            .sets()
            .get(current_frame)
            .unwrap_or_else(|| &self.sets()[0])
    }
    // Retrieve descriptor sets for all frames. May be less than frames_in_flight if the same set is
    // to be used
    fn sets(&self) -> &[DescriptorSet];
}

impl IntoSet for DescriptorSet {
    fn set(&self, _: usize) -> DescriptorSet {
        *self
    }

    fn sets(&self) -> &[DescriptorSet] {
        slice::from_ref(self)
    }
}
