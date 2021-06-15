use hecs::World;
use ivy_graphics::{Error, Material, Mesh, ShaderPass};
use ivy_resources::{Handle, ResourceCache};
use ivy_vulkan::{
    commands::CommandBuffer, descriptors::*, vk, Buffer, BufferAccess, BufferType, VulkanContext,
};
use std::{mem::size_of, sync::Arc};
use ultraviolet::Mat4;

use crate::{components::ModelMatrix, FRAMES_IN_FLIGHT};

pub const MAX_OBJECTS: usize = 8096;

/// Any entity with these components will be renderered.
pub type RenderObject<'a, T> = (
    &'a Handle<T>,
    &'a Handle<Mesh>,
    &'a Handle<Material>,
    &'a ModelMatrix,
);

pub struct MeshRenderer {
    frames: Vec<FrameData>,
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

        Ok(Self { frames })
    }

    /// Will draw all entities with a `Handle<Material>`, `Handle<Mesh>`, Modelmatrix and Shaderpass `Handle<T>`
    pub fn draw<T: 'static + ShaderPass + Sized + Sync + Send>(
        &mut self,
        world: &mut World,
        cmd: &CommandBuffer,
        current_frame: usize,
        global_set: DescriptorSet,
        materials: &ResourceCache<Material>,
        meshes: &ResourceCache<Mesh>,
        passes: &ResourceCache<T>,
    ) -> Result<(), Error> {
        let query = world.query_mut::<RenderObject<T>>();

        let frame = &mut self.frames[current_frame];

        let frame_set = frame.set;

        frame.object_buffer.write_slice(
            MAX_OBJECTS as u64,
            0,
            |data: &mut [ObjectData]| -> Result<(), Error> {
                for (i, (_, (shaderpass, mesh, material, modelmatrix))) in
                    query.into_iter().enumerate()
                {
                    data[i] = ObjectData { mvp: **modelmatrix };

                    let shaderpass = passes.get(*shaderpass)?;
                    let material = materials.get(*material)?;
                    let mesh = meshes.get(*mesh)?;

                    cmd.bind_pipeline(shaderpass.pipeline());

                    cmd.bind_descriptor_sets(
                        shaderpass.pipeline_layout(),
                        0,
                        &[global_set, frame_set, material.set()],
                    );

                    cmd.bind_vertexbuffer(0, mesh.vertex_buffer());
                    cmd.bind_indexbuffer(mesh.index_buffer(), 0);

                    cmd.draw_indexed(mesh.index_count(), 1, 0, 0, i as u32);
                }

                Ok(())
            },
        )?;

        Ok(())
    }
}

struct FrameData {
    set: DescriptorSet,
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

        Ok(Self { set, object_buffer })
    }
}

#[repr(C)]
struct ObjectData {
    mvp: Mat4,
}
