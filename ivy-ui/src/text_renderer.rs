use anyhow::Context;
use hecs::{Query, World};
use ivy_core::ModelMatrix;
use ivy_graphics::{BaseRenderer, Mesh, Renderer};
use ivy_rendergraph::Node;
use ivy_resources::{Handle, Resources};
use ivy_vulkan::{
    commands::CommandBuffer,
    descriptors::IntoSet,
    vk::{self, AccessFlags, BufferCopy, BufferMemoryBarrier, IndexType},
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

/// Renders arbitrary text using associated font and text objects attached to
/// entity. TextUpdateNode needs to be added to rendergraph before as the text
/// vertex data needs to be updated with a transfer.
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
    pub fn create_mesh(context: Arc<VulkanContext>, glyph_capacity: u32) -> Result<Mesh<UIVertex>> {
        let mut mesh = Mesh::new_uninit(context, glyph_capacity * 4, glyph_capacity * 6)?;

        // Pre fill indices
        let indices = (0..glyph_capacity * 6)
            .flat_map(|i| [i * 4, i * 4 + 1, i * 4 + 2, i * 4 + 2, i * 4 + 3, i * 4]);

        mesh.index_buffer_mut().write_iter(0, indices)?;

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
                Buffer::new_uninit::<UIVertex>(
                    context.clone(),
                    BufferUsage::TRANSFER_SRC,
                    BufferAccess::Mapped,
                    glyph_capacity as u64 * 4,
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

    /// Registers all unregistered entities capable of being rendered for specified pass. Does
    /// nothing if entities are already registered. Call this function after adding new entities to the world.
    /// # Failures
    /// Fails if object buffer cannot be reallocated to accomodate new entities.
    fn register_entities(&mut self, world: &mut World) -> Result<()> {
        let inserted = world
            .query::<(&Text, &Handle<Font>)>()
            .without::<BufferAllocation>()
            .iter()
            .map(|(e, (text, _))| (e, self.allocate(text.len() as u32).unwrap()))
            .collect::<Vec<_>>();

        inserted
            .into_iter()
            .try_for_each(|(e, block)| world.insert_one(e, block))?;

        self.base_renderer.register_entities::<KeyQuery, _>(world)?;

        Ok(())
    }

    /// Updates the text rendering
    pub fn update(&mut self, world: &mut World, current_frame: usize) -> Result<()> {
        self.register_entities(world)?;

        self.base_renderer
            .update::<KeyQuery, ObjectDataQuery, _, _>(world, current_frame)?;

        Ok(())
    }

    fn update_dirty_texts(
        &mut self,
        world: &mut World,
        resources: &Resources,
        cmd: &CommandBuffer,
        current_frame: usize,
    ) -> Result<()> {
        let mut offset = 0;

        let fonts = resources.fetch::<Font>()?;
        let staging_buffer = &mut self.staging_buffers[current_frame];
        let sb = staging_buffer.buffer();
        let vb = self.mesh.vertex_buffer().into();

        let dirty_texts = world
            .query_mut::<(&mut Text, &Handle<Font>, &BufferAllocation)>()
            .into_iter()
            .filter(|(_, (t, _, _))| t.dirty())
            .flat_map(|(_, (text, font, _block))| {
                text.set_dirty(false);

                let size = (text.len() * 4 * size_of::<UIVertex>()) as u64;

                let region = &[BufferCopy {
                    src_offset: 0,
                    dst_offset: 0 as _,
                    size,
                }];

                cmd.copy_buffer(sb, vb, region);

                offset += text.len() as u64;

                let font = fonts.get(*font).unwrap();
                text.layout(font).unwrap()
            });

        let barrier = BufferMemoryBarrier {
            src_access_mask: AccessFlags::SHADER_READ,
            dst_access_mask: AccessFlags::TRANSFER_WRITE,
            buffer: vb,
            size: vk::WHOLE_SIZE,
            ..Default::default()
        };

        cmd.pipeline_barrier(
            vk::PipelineStageFlags::VERTEX_SHADER,
            vk::PipelineStageFlags::TRANSFER,
            &[barrier],
            &[],
        );

        staging_buffer.write_iter(0, dirty_texts)?;

        let barrier = BufferMemoryBarrier {
            src_access_mask: AccessFlags::TRANSFER_WRITE,
            dst_access_mask: AccessFlags::VERTEX_ATTRIBUTE_READ,
            buffer: vb,
            size: vk::WHOLE_SIZE,
            ..Default::default()
        };

        cmd.pipeline_barrier(
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::VERTEX_INPUT,
            &[barrier],
            &[],
        );

        Ok(())
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
        sets: &[ivy_vulkan::descriptors::DescriptorSet],
        // Dynamic offsets for supplied sets
        offsets: &[u32],
        // Graphics resources like textures and materials
        resources: &ivy_resources::Resources,
    ) -> Result<()> {
        cmd.bind_vertexbuffer(0, self.mesh.vertex_buffer());
        cmd.bind_indexbuffer(self.mesh.index_buffer(), IndexType::UINT32, 0);

        let frame_set = self.base_renderer.set(current_frame);

        let passes = resources.fetch::<Pass>()?;

        {
            let pass = self.base_renderer.pass_mut::<Pass>();
            pass.get_unbatched::<Pass, KeyQuery, _>(world);
            pass.build_batches::<Pass, KeyQuery, _, _>(world, &passes)?;
        }

        let pass = self.base_renderer.pass::<Pass>();
        let object_buffer = self.base_renderer.object_buffer(current_frame);
        let object_data = object_buffer
            .mapped_slice::<ObjectData>()
            .expect("Non mappable object data buffer");

        for batch in pass.batches() {
            let key = batch.key();

            let font = resources.get(key.font)?;

            if !sets.is_empty() {
                cmd.bind_descriptor_sets(batch.layout(), 0, sets, offsets);
            }

            cmd.bind_descriptor_sets(
                batch.layout(),
                sets.len() as u32,
                &[frame_set, font.set(0)],
                &[],
            );

            cmd.bind_pipeline(batch.pipeline());

            for id in batch.ids() {
                let data = &object_data[*id as usize];

                cmd.draw_indexed(data.len * 6, 1, 0, 0, *id);
            }
        }

        Ok(())
    }
}

#[repr(C, align(16))]
#[derive(Debug, Clone, Copy)]
struct ObjectData {
    mvp: ModelMatrix,
    offset: u32,
    len: u32,
}

#[derive(Query)]
struct ObjectDataQuery<'a> {
    mvp: &'a ModelMatrix,
    block: &'a BufferAllocation,
}

impl<'a> Into<ObjectData> for ObjectDataQuery<'a> {
    fn into(self) -> ObjectData {
        ObjectData {
            mvp: *self.mvp,
            offset: self.block.offset,
            len: self.block.len,
        }
    }
}

#[derive(Query, PartialEq, Eq)]
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

pub struct TextUpdateNode {
    text_renderer: Handle<TextRenderer>,
}

impl TextUpdateNode {
    pub fn new(text_renderer: Handle<TextRenderer>) -> Self {
        Self { text_renderer }
    }
}

impl Node for TextUpdateNode {
    fn node_kind(&self) -> ivy_rendergraph::NodeKind {
        ivy_rendergraph::NodeKind::Transfer
    }

    fn execute(
        &mut self,
        world: &mut World,
        cmd: &CommandBuffer,
        current_frame: usize,
        resources: &Resources,
    ) -> anyhow::Result<()> {
        resources
            .get_mut(self.text_renderer)?
            .update_dirty_texts(world, resources, cmd, current_frame)
            .context("Failed to update text")
    }
}
