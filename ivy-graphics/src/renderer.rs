use crate::{Result, ShaderPass};
use hecs::{Entity, World};
use ivy_resources::ResourceManager;
use ivy_vulkan::commands::CommandBuffer;

// Generic interface for a renderer.
pub trait Renderer {
    // Draws the scene using the pass [`Pass`] and the provided camera.
    // Note: camera must have gpu side data.
    fn draw<Pass: 'static + ShaderPass + Sized + Sync + Send>(
        &mut self,
        // The ecs world
        world: &mut World,
        // The camera to use for rendering
        camera: Entity,
        // The commandbuffer to record into
        cmd: &CommandBuffer,
        // The current swapchain image or backbuffer index
        current_frame: usize,
        // The first set the renderer will use
        first_set: u32,
        // Graphics resources like textures and materials
        resources: &ResourceManager,
    ) -> Result<()>;
}
