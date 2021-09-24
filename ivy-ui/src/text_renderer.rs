use hecs::{Query, World};
use ivy_core::ModelMatrix;
use ivy_graphics::{BaseRenderer, Mesh, Renderer};
use ivy_resources::{Handle, Resources};
use ivy_vulkan::{
    commands::CommandBuffer,
    vk::{BufferCopy, IndexType},
    Buffer, BufferAccess, BufferUsage, VulkanContext,
};
use std::{mem::size_of, sync::Arc};

use crate::Error;
use crate::Font;
use crate::Result;
use crate::Text;
use crate::UIVertex;

/// Attached to each text that has a part of the buffer reserved for its text
/// mesh data. `len` and `block` refers to the number of quads allocated, not
/// vertices nor indices.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BufferAllocation {
    len: u32,
    offset: u32,
}

impl BufferAllocation {
    pub fn new(len: u32, offset: u32) -> Self {
        Self { len, offset }
    }
}

pub struct TextRenderer {
    mesh: Mesh<UIVertex>,
    // Free contiguos blocks in mesh
    free: Vec<BufferAllocation>,
    /// The number of registered text objects
    staging_buffers: Vec<Buffer>,
    base_renderer: BaseRenderer<Key, ObjectData>,
}

impl TextRenderer {
    pub fn new(
        context: Arc<VulkanContext>,
        capacity: u32,
        glyph_capacity: u32,
        frames_in_flight: usize,
    ) -> Result<Self> {
        let mesh = Self::create_mesh(context.clone(), glyph_capacity)?;
        let staging_buffers =
            Self::create_staging_buffers(context.clone(), glyph_capacity, frames_in_flight)?;

        let base_renderer = BaseRenderer::new(context.clone(), capacity, frames_in_flight)?;

        Ok(Self {
            mesh,
            free: vec![BufferAllocation::new(capacity, 0)],
            staging_buffers,
            base_renderer,
        })
    }

    // Creates a mesh able to store `capacity` characters
    pub fn create_mesh(context: Arc<VulkanContext>, capacity: u32) -> Result<Mesh<UIVertex>> {
        let mut mesh = Mesh::new_uninit(context, capacity as u64 * 4, capacity as u64 * 6)?;

        // Pre fill indices
        let indices = [0, 1, 2, 2, 3, 0]
            .iter()
            .cloned()
            .cycle()
            .take(capacity as usize)
            .collect::<Vec<_>>();

        mesh.index_buffer_mut().fill(0, &indices)?;

        Ok(mesh)
    }

    // Creates a mesh able to store `capacity` characters
    pub fn create_staging_buffers(
        context: Arc<VulkanContext>,
        glyph_capacity: u32,
        frames_in_flight: usize,
    ) -> Result<Vec<Buffer>> {
        (0..frames_in_flight)
            .map(|_| {
                Buffer::new_uninit(
                    context.clone(),
                    BufferUsage::TRANSFER_SRC,
                    BufferAccess::Mapped,
                    glyph_capacity as u64 * 4 * size_of::<UIVertex>() as u64,
                )
                .map_err(|e| e.into())
            })
            .collect::<Result<Vec<_>>>()
    }

    /// Allocates a contiguous block in the mesh
    pub fn allocate(&mut self, len: u32) -> Option<BufferAllocation> {
        let (index, block) = self
            .free
            .iter_mut()
            .enumerate()
            .find(|(_, block)| block.len >= len)?;

        if block.len == len {
            Some(self.free.remove(index))
        } else {
            let mut block = std::mem::replace(
                block,
                BufferAllocation {
                    len: block.len - len,
                    offset: block.offset + len,
                },
            );

            block.len = len;
            Some(block)
        }
    }

    /// Registers all unregisters entities capable of being rendered for specified pass. Does
    /// nothing if entities are already registered. Call this function after adding new entities to the world.
    /// # Failures
    /// Fails if object buffer cannot be reallocated to accomodate new entities.
    pub fn register_entities(&mut self, world: &mut World) -> Result<()> {
        self.base_renderer
            .register_entities::<KeyQuery, _>(world)
            .map_err(|e| e.into())
    }

    /// Updates the text rendering
    fn update(
        &mut self,
        world: &mut World,
        resources: &Resources,
        cmd: &CommandBuffer,
        current_frame: usize,
    ) -> Result<()> {
        self.register_entities(world)?;

        let fonts = resources.fetch::<Font>()?;
        let staging_buffer = &mut self.staging_buffers[current_frame];
        let sb = staging_buffer.buffer();
        let vb = self.mesh.vertex_buffer().into();

        let mut offset = 0;

        let dirty_texts = world
            .query_mut::<(&mut Text, &Handle<Font>, &BufferAllocation)>()
            .into_iter()
            .filter(|(_, (t, _, _))| t.dirty())
            .map(|(_, (text, font, block))| -> Result<_> {
                text.set_dirty(false);
                cmd.copy_buffer(
                    sb,
                    vb,
                    &[BufferCopy {
                        src_offset: offset,
                        dst_offset: block.offset as _,
                        size: (text.len() * 4 * size_of::<UIVertex>()) as u64,
                    }],
                );

                offset += text.len() as u64;

                let font = fonts.get(*font)?;
                Ok((block, text.layout(font)?))
            });

        // let mut staging_buffer = Buffer::new_uninit(
        //     self.context.clone(),
        //     BufferUsage::TRANSFER_SRC,
        //     BufferAccess::Mapped,
        //     dirty_len as u64 * 4,
        // )?;

        staging_buffer.write_iter(0, dirty_texts)??;

        todo!()
    }
}

impl Renderer for TextRenderer {
    type Error = Error;
    fn draw<Pass: ivy_graphics::ShaderPass>(
        &mut self,
        // The ecs world
        world: &mut hecs::World,
        // The commandbuffer to record into
        cmd: &ivy_vulkan::commands::CommandBuffer,
        // The current swapchain image or backbuffer index
        current_frame: usize,
        // Descriptor sets to bind before renderer specific sets
        _sets: &[ivy_vulkan::descriptors::DescriptorSet],
        // Dynamic offsets for supplied sets
        _offsets: &[u32],
        // Graphics resources like textures and materials
        resources: &ivy_resources::Resources,
    ) -> Result<()> {
        cmd.bind_vertexbuffer(0, self.mesh.vertex_buffer());
        cmd.bind_indexbuffer(self.mesh.index_buffer(), IndexType::UINT32, 0);

        self.update(world, resources, cmd, current_frame)?;

        world
            .query_mut::<&BufferAllocation>()
            .into_iter()
            .for_each(|(_, block)| {
                let v_offset = block.offset * 4;
                let i_offset = block.offset * 6;
                let index_count = block.len * 6;

                cmd.draw_indexed(index_count, 1, i_offset, v_offset as i32, 0);
            });

        todo!()
    }
}

#[repr(C, align(16))]
struct ObjectData {
    mvp: ModelMatrix,
}

#[derive(Query, Hash, PartialEq, Eq)]
pub struct KeyQuery<'a> {
    font: &'a Handle<Font>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
    font: Handle<Font>,
}

impl<'a> ivy_graphics::KeyQuery for KeyQuery<'a> {
    type K = Key;

    fn into_key(&self) -> Self::K {
        Self::K { font: *self.font }
    }
}
