use anyhow::Context;
use flax::{Component, Fetch, Mutable, Query, World};
use glam::{Mat4, Vec2, Vec4};
use ivy_graphics::{Allocator, BaseRenderer, BatchMarker, BufferAllocation, Mesh, Renderer};
use ivy_rendergraph::Node;
use ivy_resources::{Handle, Resources};
use ivy_vulkan::{
    commands::CommandBuffer,
    context::SharedVulkanContext,
    descriptors::{DescriptorSet, IntoSet},
    vk::{self, AccessFlags, BufferCopy, BufferMemoryBarrier, IndexType},
    BufferAccess, BufferUsage, PassInfo, Shader,
};
use ivy_vulkan::{device, Buffer};
use std::{mem::size_of, path::Component};

use crate::WrapStyle;
use crate::{alignment, font, margin, text, wrap, Font, UIVertex};
use crate::{Alignment, Text};
use crate::{Error, Result};
use ivy_base::{size, Color, Visible};

flax::component! {
    block: BufferAllocation<Marker>,
}

#[derive(Fetch)]
struct TextQuery<'a> {
    text: Mutable<Text>,
    font: Component<Handle<Font>>,
    block: Mutable<BufferAllocation<Marker>>,
    bounds: Component<Vec2>,
    alignment: Component<Alignment>,
    wrap: Component<WrapStyle>,
    margin: Component<Vec2>,
}

impl<'a> TextQuery<'a> {
    fn new() -> Self {
        Self {
            text: text(),
            font: font(),
            block: block(),
            bounds: size(),
            alignment: alignment(),
            wrap: wrap(),
            margin: margin(),
        }
    }
}

/// Renders arbitrary text using associated font and text objects attached to
/// entity. TextUpdateNode needs to be added to rendergraph before as the text
/// vertex data needs to be updated with a transfer.
pub struct TextRenderer {
    mesh: Mesh<UIVertex>,
    staging_buffers: Vec<Buffer>,
    allocator: Allocator<Marker>,
    base_renderer: BaseRenderer<Key, ObjectData, UIVertex>,
    /// The total number of glyphs
    glyph_count: u32,
    frames_in_flight: usize,
}

impl TextRenderer {
    pub fn new(
        context: SharedVulkanContext,
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
            staging_buffers,
            base_renderer,
            allocator: Allocator::new(glyph_capacity as _),
            glyph_count: 0,
            frames_in_flight,
        })
    }

    pub fn vertex_buffer(&self) -> vk::Buffer {
        self.mesh.vertex_buffer().buffer()
    }

    // Creates a mesh able to store `capacity` characters
    pub fn create_mesh(
        context: SharedVulkanContext,
        glyph_capacity: u32,
    ) -> Result<Mesh<UIVertex>> {
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
        context: SharedVulkanContext,
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

    fn grow(&mut self) -> Result<()> {
        let context = self.base_renderer.context();
        device::wait_idle(self.base_renderer.context().device())?;

        self.allocator.grow_double();

        self.mesh = Self::create_mesh(context.clone(), self.allocator.capacity() as _)?;

        self.staging_buffers = Self::create_staging_buffers(
            context.clone(),
            self.allocator.capacity() as _,
            self.frames_in_flight,
        )?;

        Ok(())
    }

    /// Registers all unregistered entities capable of being rendered for specified pass. Does
    /// nothing if entities are already registered. Call this function after adding new entities to the world.
    /// # Failures
    /// Fails if object buffer cannot be reallocated to accomodate new entities.
    fn register_entities(&mut self, world: &mut World) -> Result<()> {
        let inserted = Query::new(text())
            .without(block)
            .borrow(world)
            .iter()
            .map(|(e, text)| {
                self.glyph_count += text.len() as u32;
                self.allocator.allocate(text.len()).map(|block| (e, block))
            })
            .collect::<Option<Vec<_>>>();

        if let Some(inserted) = inserted {
            inserted
                .into_iter()
                .for_each(|(e, block)| world.insert_one(e, block).unwrap());
        } else {
            self.grow()?;

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
            .query_mut::<(&Text, &mut BufferAllocation<Marker>)>()
            .into_iter()
            .map(|(_, (text, block))| {
                self.glyph_count += text.len() as u32;

                // Reallocate to fit longer text
                if text.len() > block.len() {
                    let len = nearest_power_2(text.len());

                    self.allocator.free(*block);

                    if let Some(new_block) = self.allocator.allocate(len) {
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
            self.grow()?;
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
                        || query.text.old_margin() != *query.margin
                        || &query.text.old_wrap() != query.wrap)
            })
            .flat_map(|(_, query)| {
                query.text.set_dirty(false);

                let size = (query.text.len() * 4 * size_of::<UIVertex>()) as u64;

                let region = &[BufferCopy {
                    src_offset: offset,
                    dst_offset: query.block.offset() as u64 * 4 * size_of::<UIVertex>() as u64,
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
                        *query.wrap,
                        *query.alignment,
                        *query.margin,
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
    fn draw(
        &mut self,
        world: &mut World,
        resources: &Resources,
        cmd: &ivy_vulkan::CommandBuffer,
        sets: &[DescriptorSet],
        pass_info: &PassInfo,
        offsets: &[u32],
        current_frame: usize,
        shaderpass: Component<Shader>,
    ) -> Result<()> {
        cmd.bind_vertexbuffer(0, self.mesh.vertex_buffer());
        cmd.bind_indexbuffer(self.mesh.index_buffer(), IndexType::UINT32, 0);

        {
            let pass = self.base_renderer.pass_mut::<Pass>()?;
            pass.register(world);
            pass.build_batches::<Pass, KeyQuery>(world, resources, pass_info)?;
            let iter = world
                .query_mut::<(&BatchMarker<ObjectData, Pass>, ObjectDataQuery, &Visible)>()
                .into_iter()
                .filter_map(|(e, (marker, obj, visible))| {
                    if visible.is_visible() {
                        Some((e, (marker, obj)))
                    } else {
                        None
                    }
                });

            pass.update(current_frame, iter)?;
        }

        let pass = self.base_renderer.pass::<Pass>();

        let frame_set = pass.set(current_frame);

        let object_buffer = pass.object_buffer(current_frame);
        let object_data = object_buffer
            .mapped_slice::<ObjectData>()
            .expect("Non mappable object data buffer");

        for batch in pass.batches().iter() {
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
#[derive(Default, Debug, Clone, Copy, PartialEq)]
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
    block: &'a BufferAllocation<Marker>,
}

impl<'a> Into<ObjectData> for ObjectDataQuery<'a> {
    fn into(self) -> ObjectData {
        ObjectData {
            mvp: Mat4::from_translation(self.position.extend(0.0)),
            color: self.color.into(),
            offset: self.block.offset() as u32,
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
        _: &PassInfo,
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

fn nearest_power_2(val: usize) -> usize {
    let mut result = 1;
    while result < val {
        result *= 2;
    }
    result
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
struct Marker;
