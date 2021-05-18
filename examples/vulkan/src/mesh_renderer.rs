use std::{mem::size_of, sync::Arc};

use hecs::World;
use ivy_graphics::Mesh;
use ivy_vulkan::{
    commands::CommandBuffer,
    descriptors::{
        DescriptorAllocator, DescriptorBuilder, DescriptorLayoutCache, DescriptorSet,
        DescriptorSetLayout,
    },
    vk, Buffer, BufferAccess, BufferType, Error, Pipeline, VulkanContext,
};
use ultraviolet::Mat4;

use crate::{components::ModelMatrix, Position, FRAMES_IN_FLIGHT};

pub const MAX_OBJECTS: usize = 256;

pub struct MeshRenderer {
    frames: Vec<FrameData>,
    context: Arc<VulkanContext>,
}

impl MeshRenderer {
    pub fn new(
        context: Arc<VulkanContext>,
        descriptor_layout_cache: &mut DescriptorLayoutCache,
        descriptor_allocator: &mut DescriptorAllocator,
    ) -> Result<Self, Error> {
        let frames = (0..FRAMES_IN_FLIGHT)
            .map(|_| {
                FrameData::new(
                    context.clone(),
                    descriptor_layout_cache,
                    descriptor_allocator,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self { context, frames })
    }

    pub fn draw(
        &mut self,
        world: &mut World,
        cmd: &CommandBuffer,
        current_frame: usize,
        global_set: DescriptorSet,
    ) -> Result<(), Error> {
        let query = world.query_mut::<(&ModelMatrix, &Arc<Mesh>, &Arc<Pipeline>)>();

        let frame = &mut self.frames[current_frame];

        let sets = &[global_set, frame.set];

        frame
            .object_buffer
            .write_slice(MAX_OBJECTS as u64, 0, |data| {
                let mut i = 0;
                for (_, (mvp, mesh, pipeline)) in query {
                    data[i] = ObjectData { mvp: **mvp };

                    cmd.bind_pipeline(pipeline);

                    cmd.bind_descriptor_sets(pipeline.layout(), 0, sets);

                    cmd.bind_vertexbuffer(0, mesh.vertex_buffer());
                    cmd.bind_indexbuffer(mesh.index_buffer(), 0);

                    cmd.draw_indexed(mesh.index_count(), 1, 0, 0, i as u32);
                    i += 1;
                }
            })?;

        Ok(())
    }
}

struct FrameData {
    set: DescriptorSet,
    set_layout: DescriptorSetLayout,
    object_buffer: Buffer,
}

impl FrameData {
    pub fn new(
        context: Arc<VulkanContext>,
        descriptor_layout_cache: &mut DescriptorLayoutCache,
        descriptor_allocator: &mut DescriptorAllocator,
    ) -> Result<Self, Error> {
        let object_buffer = Buffer::new_uninit(
            context.clone(),
            BufferType::Storage,
            BufferAccess::MappedPersistent,
            (size_of::<ObjectData>() * MAX_OBJECTS) as u64,
        )?;

        let mut set = Default::default();
        let mut set_layout = Default::default();

        DescriptorBuilder::new()
            .bind_storage_buffer(0, vk::ShaderStageFlags::VERTEX, &object_buffer)
            .build(
                context.device(),
                descriptor_layout_cache,
                descriptor_allocator,
                &mut set,
            )?
            .layout(descriptor_layout_cache, &mut set_layout)?;

        Ok(Self {
            object_buffer,
            set,
            set_layout,
        })
    }
}

#[repr(C)]
struct ObjectData {
    mvp: Mat4,
}
