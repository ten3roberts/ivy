use std::sync::Arc;

use hecs::World;
use ivy_graphics::{Error, ShaderPass};
use ivy_resources::ResourceManager;
use ivy_vulkan::{
    commands::CommandBuffer,
    descriptors::{DescriptorAllocator, DescriptorLayoutCache, DescriptorSet},
    Buffer, VulkanContext,
};
use ultraviolet::{Mat4, Vec4};

use crate::ImageRenderer;

const CAPACITY: usize = 16;

// Master UI renderer. Manages the canvas.
pub struct UIRenderer {
    frames: Vec<FrameData>,

    image_renderer: ImageRenderer,
}

impl UIRenderer {
    pub fn new(
        context: Arc<VulkanContext>,
        descriptor_layout_cache: &mut DescriptorLayoutCache,
        frames_in_flight: usize,
    ) -> Result<Self, Error> {
        let mut descriptor_allocator =
            DescriptorAllocator::new(context.device().clone(), frames_in_flight as u32);

        let frames = (0..frames_in_flight)
            .map(|_| {
                FrameData::new(
                    context.clone(),
                    descriptor_layout_cache,
                    &mut descriptor_allocator,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        let image_renderer = ImageRenderer::new(
            context.clone(),
            descriptor_layout_cache,
            CAPACITY as u32,
            frames_in_flight,
        )?;

        Ok(Self {
            frames,
            image_renderer,
        })
    }

    /// Begins drawing of UI.
    pub fn draw<T: 'static + ShaderPass + Sized + Sync + Send>(
        &mut self,
        world: &mut World,
        cmd: &CommandBuffer,
        current_frame: usize,
        resources: &ResourceManager,
    ) -> Result<(), Error> {
        let frame = &mut self.frames[current_frame];

        let frame_set = frame.set;

        self.image_renderer
            .draw::<T>(world, cmd, current_frame, frame_set, resources)?;

        Ok(())
    }
}

struct FrameData {
    set: DescriptorSet,
    uniformbuffer: Buffer,
}

impl FrameData {
    pub fn new(
        context: Arc<VulkanContext>,
        descriptor_layout_cache: &mut DescriptorLayoutCache,
        descriptor_allocator: &mut DescriptorAllocator,
    ) -> Result<Self, Error> {
        let uniformbuffer = Buffer::new(
            context.clone(),
            ivy_vulkan::BufferType::Uniform,
            ivy_vulkan::BufferAccess::MappedPersistent,
            &[UIData {
                color: Vec4::new(0.0, 0.0, 0.0, 1.0),
                viewproj: Mat4::identity(),
            }],
        );

        todo!()
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
struct UIData {
    color: Vec4,
    viewproj: Mat4,
}
