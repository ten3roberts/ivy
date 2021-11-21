use anyhow::Context;
use hecs::{Query, World};
use ivy_graphics::{BaseRenderer, Mesh, Renderer};
use ivy_rendergraph::Node;
use ivy_resources::{Handle, Resources};
use ivy_vulkan::{
    commands::CommandBuffer,
    descriptors::IntoSet,
    vk::{self, AccessFlags, BufferCopy, BufferMemoryBarrier, IndexType},
    BufferAccess, BufferUsage, VulkanContext,
};
use ivy_vulkan::{device, Buffer};
use std::{mem::size_of, sync::Arc};
use ultraviolet::{Mat4, Vec4};

use crate::Text;
use crate::UIVertex;
use crate::WrapStyle;
use crate::{Error, Result};
use crate::{Font, TextAlignment};
use ivy_base::{Color, Position2D, Size2D};

#[derive(Query)]
struct TextQuery<'a> {
    text: &'a mut Text,
    font: &'a Handle<Font>,
    block: &'a mut BufferAllocation,
    bounds: &'a Size2D,
    alignment: Option<&'a TextAlignment>,
    wrap: Option<&'a WrapStyle>,
}

/// Attached to each text that has a part of the buffer reserved for its text
/// mesh data. `len` and `block` refers to the number of quads allocated, not
/// vertices nor indices.
#[derive(Debug, Clone, Copy, PartialEq)]
struct BufferAllocation {
    // Length in number of characters
    len: u32,
    // Offset in number of characters
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
    context: Arc<VulkanContext>,
    mesh: Mesh<UIVertex>,
    // Free contiguos blocks in mesh
    free: Vec<BufferAllocation>,
    /// The number of registered text objects
    staging_buffers: Vec<Buffer>,
    base_renderer: BaseRenderer<Key, ObjectData>,
    glyph_capacity: u32,
    /// The total number of glyphs
    glyph_count: u32,
    frames_in_flight: usize,
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
            context,
            mesh,
            free: vec![BufferAllocation::new(glyph_capacity, 0)],
            staging_buffers,
            base_renderer,
            glyph_capacity,
            glyph_count: 0,
            frames_in_flight,
        })
    }

    pub fn vertex_buffer(&self) -> vk::Buffer {
        self.mesh.vertex_buffer().buffer()
    }

    // Creates a mesh able to store `capacity` characters
    pub fn create_mesh(context: Arc<VulkanContext>, glyph_capacity: u32) -> Result<Mesh<UIVertex>> {
        let mut mesh = Mesh::new_uninit(context, glyph_capacity * 4, glyph_capacity * 6, vec![])?;

        // Pre fill indices
        let indices = (0..glyph_capacity * 6)
            .step_by(4)
            .flat_map(|i| [i, i + 1, i + 2, i + 2, i + 3, i]);
        // 0 1 2 2 3 0

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
    fn allocate(&mut self, len: u32) -> Option<BufferAllocation> {
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

    fn free(&mut self, block: BufferAllocation) {
        let index = self
            .free
            .iter()
            .enumerate()
            .find(|val| val.1.offset > block.offset)
            .map(|val| val.0)
            .unwrap_or(self.free.len());

        if index != 0 && index != self.free.len() {
            match &mut self.free[index - 1..=index] {
                [a, b] => {
                    if a.offset + a.len + block.len == b.offset {
                        a.len += block.len + b.len;
                        self.free.remove(index);
                    } else if a.offset + a.len == block.offset {
                        a.len += block.len;
                    } else if block.offset + block.len == b.offset {
                        b.len += block.len;
                        b.offset = block.len;
                    } else {
                        self.free.insert(index, block);
                    }
                }
                _ => unreachable!(),
            }
        }
    }

    fn resize(&mut self, world: &mut World, glyph_capacity: u32) -> Result<()> {
        device::wait_idle(self.context.device())?;

        // eprintln!(
        //     "Resizing glyph_capacity from {} to {}",
        //     self.glyph_capacity, glyph_capacity
        // );

        self.glyph_capacity = glyph_capacity;

        self.mesh = Self::create_mesh(self.context.clone(), glyph_capacity)?;

        self.staging_buffers = Self::create_staging_buffers(
            self.context.clone(),
            glyph_capacity,
            self.frames_in_flight,
        )?;

        self.free = vec![BufferAllocation::new(glyph_capacity, 0)];

        // Refit all blocks
        world
            .query_mut::<(&Text, &mut BufferAllocation)>()
            .into_iter()
            .for_each(|(_, (text, block))| {
                *block = self
                    .allocate(text.len() as u32)
                    .expect("Cannot allocate after resize");
            });

        Ok(())
    }

    /// Registers all unregistered entities capable of being rendered for specified pass. Does
    /// nothing if entities are already registered. Call this function after adding new entities to the world.
    /// # Failures
    /// Fails if object buffer cannot be reallocated to accomodate new entities.
    fn register_entities(&mut self, world: &mut World) -> Result<()> {
        let inserted = world
            .query::<&Text>()
            .without::<BufferAllocation>()
            .iter()
            .map(|(e, text)| {
                self.glyph_count += text.len() as u32;
                self.allocate(text.len() as u32).map(|block| (e, block))
            })
            .collect::<Option<Vec<_>>>();

        if let Some(inserted) = inserted {
            inserted
                .into_iter()
                .try_for_each(|(e, block)| world.insert_one(e, block))?;
        } else {
            self.resize(world, nearest_power_2(self.glyph_count))?;

            return self.register_entities(world);
        }

        Ok(())
    }

    fn update_dirty_texts(
        &mut self,
        world: &mut World,
        resources: &Resources,
        cmd: &CommandBuffer,
        current_frame: usize,
    ) -> Result<()> {
        self.register_entities(world)?;

        self.glyph_count = 0;

        // Reallocate as needed
        let success = world
            .query_mut::<(&Text, &mut BufferAllocation)>()
            .into_iter()
            .map(|(_, (text, block))| {
                self.glyph_count += text.len() as u32;

                // Reallocate to fit longer text
                if text.len() as u32 > block.len {
                    let len = nearest_power_2(text.len() as u32);

                    self.free(*block);

                    if let Some(new_block) = self.allocate(len) {
                        *block = new_block;
                        true
                    } else {
                        false
                    }
                } else {
                    true
                }
            })
            .fold(true, |acc, val| acc && val);

        // Resize to fit
        if !success {
            self.resize(world, nearest_power_2(self.glyph_count))?;
            return self.update_dirty_texts(world, resources, cmd, current_frame);
        }

        let mut offset = 0;

        let fonts = resources.fetch::<Font>()?;
        let staging_buffer = &mut self.staging_buffers[current_frame];
        let sb = staging_buffer.buffer();
        let vb = self.mesh.vertex_buffer().into();

        let dirty_texts = world
            .query_mut::<TextQuery>()
            .into_iter()
            .filter(|(_, query)| {
                query.text.len() > 0
                    && (query.text.dirty()
                        || query.text.old_bounds() != *query.bounds
                        || Some(&query.text.old_wrap()) != query.wrap)
            })
            .flat_map(|(_, query)| {
                query.text.set_dirty(false);

                let size = (query.text.len() * 4 * size_of::<UIVertex>()) as u64;

                let region = &[BufferCopy {
                    src_offset: offset,
                    dst_offset: query.block.offset as u64 * 4 * size_of::<UIVertex>() as u64,
                    size,
                }];

                cmd.copy_buffer(sb, vb, region);

                offset += size;

                let font = fonts.get(*query.font).unwrap();
                query
                    .text
                    .layout(
                        font,
                        *query.bounds,
                        query.wrap.cloned().unwrap_or_default(),
                        query.alignment.cloned().unwrap_or_default(),
                    )
                    .unwrap()
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

        let passes = resources.fetch::<Pass>()?;

        {
            let pass = self.base_renderer.pass_mut::<Pass>()?;
            pass.get_unbatched::<Pass, KeyQuery, ObjectDataQuery, _, _>(world);
            pass.build_batches::<Pass, KeyQuery, _, _>(world, &passes)?;
            pass.update::<Pass, ObjectDataQuery, _>(world, current_frame)?;
        }

        let pass = self.base_renderer.pass::<Pass>();

        let frame_set = pass.set(current_frame);

        let object_buffer = pass.object_buffer(current_frame);
        let object_data = object_buffer
            .mapped_slice::<ObjectData>()
            .expect("Non mappable object data buffer");

        for batch in pass.batches() {
            let key = batch.key();

            let font = resources.get(key.font)?;

            cmd.bind_pipeline(batch.pipeline());

            if !sets.is_empty() {
                cmd.bind_descriptor_sets(batch.layout(), 0, sets, offsets);
            }

            cmd.bind_descriptor_sets(
                batch.layout(),
                sets.len() as u32,
                &[frame_set, font.set(0)],
                &[],
            );

            for id in batch.ids() {
                let data = &object_data[id as usize];

                cmd.draw_indexed(data.len * 6, 1, data.offset * 6, 0, id);
            }
        }

        Ok(())
    }
}

#[repr(C, align(16))]
#[derive(Debug, Clone, Copy)]
struct ObjectData {
    mvp: Mat4,
    color: Vec4,
    offset: u32,
    len: u32,
}

#[derive(Query)]
struct ObjectDataQuery<'a> {
    position: &'a Position2D,
    color: &'a Color,
    text: &'a Text,
    block: &'a BufferAllocation,
}

impl<'a> Into<ObjectData> for ObjectDataQuery<'a> {
    fn into(self) -> ObjectData {
        ObjectData {
            mvp: Mat4::from_translation(self.position.xyz()),
            color: self.color.into(),
            offset: self.block.offset,
            len: self.text.len() as u32,
        }
    }
}

#[derive(Query, PartialEq, Eq)]
struct KeyQuery<'a> {
    font: &'a Handle<Font>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
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
    buffer: vk::Buffer,
}

impl TextUpdateNode {
    pub fn new(resources: &Resources, text_renderer: Handle<TextRenderer>) -> Result<Self> {
        let buffer = resources
            .get::<TextRenderer>(text_renderer)?
            .vertex_buffer();

        Ok(Self {
            text_renderer,
            buffer,
        })
    }
}

impl Node for TextUpdateNode {
    fn node_kind(&self) -> ivy_rendergraph::NodeKind {
        ivy_rendergraph::NodeKind::Transfer
    }

    fn buffer_writes(&self) -> &[vk::Buffer] {
        std::slice::from_ref(&self.buffer)
    }

    fn execute(
        &mut self,
        world: &mut World,
        resources: &Resources,
        cmd: &CommandBuffer,
        current_frame: usize,
    ) -> anyhow::Result<()> {
        resources
            .get_mut(self.text_renderer)?
            .update_dirty_texts(world, resources, cmd, current_frame)
            .context("Failed to update text")
    }

    fn debug_name(&self) -> &'static str {
        "Text Update"
    }
}

fn nearest_power_2(val: u32) -> u32 {
    let mut result = 1;
    while result < val {
        result *= 2;
    }
    result
}
