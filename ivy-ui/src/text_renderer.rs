use anyhow::Context;
use flax::{entity_ids, Component, Fetch, Mutable, Query, World};
use glam::{Mat4, Vec2, Vec3, Vec4};
use ivy_assets::{Asset, AssetCache};
use ivy_graphics::{batch_id, Allocator, BaseRenderer, BufferAllocation, Mesh, Renderer};
use ivy_rendergraph::Node;
use ivy_vulkan::{
    commands::CommandBuffer,
    context::SharedVulkanContext,
    descriptors::{DescriptorSet, IntoSet},
    vk::{
        self, AccessFlags, AttachmentSampleCountInfoAMD, BufferCopy, BufferMemoryBarrier, IndexType,
    },
    BufferAccess, BufferUsage, PassInfo, Shader,
};
use ivy_vulkan::{device, Buffer};
use parking_lot::Mutex;
use std::{mem::size_of, sync::Arc};

use crate::WrapStyle;
use crate::{alignment, font, margin, text, wrap, Font, UIVertex};
use crate::{Alignment, Text};
use crate::{Error, Result};
use ivy_base::{color, position, size, visible, Color, ColorExt};

flax::component! {
    block: BufferAllocation<Marker>,
}

#[derive(Fetch)]
struct TextQuery {
    text: Mutable<Text>,
    font: Component<Asset<Font>>,
    block: Mutable<BufferAllocation<Marker>>,
    bounds: Component<Vec2>,
    alignment: Component<Alignment>,
    wrap: Component<WrapStyle>,
    margin: Component<Vec2>,
}

impl TextQuery {
    fn new() -> Self {
        Self {
            text: text().as_mut(),
            font: font(),
            block: block().as_mut(),
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
        let inserted = Query::new((entity_ids(), text()))
            .without(block())
            .borrow(world)
            .iter()
            .map(|(id, text)| {
                self.glyph_count += text.len() as u32;
                self.allocator.allocate(text.len()).map(|block| (id, block))
            })
            .collect::<Option<Vec<_>>>();

        if let Some(inserted) = inserted {
            inserted.into_iter().for_each(|(id, block)| {
                world.set(id, self::block(), block).unwrap();
            });
        } else {
            self.grow()?;

            return self.register_entities(world);
        }

        Ok(())
    }

    fn update_dirty_texts(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        cmd: &CommandBuffer,
        current_frame: usize,
    ) -> Result<()> {
        self.register_entities(world)?;

        self.glyph_count = 0;

        // Reallocate as needed
        let mut query = Query::new((text(), block().as_mut()));
        let success = query
            .borrow(world)
            .iter()
            .map(|(text, block)| {
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
            return self.update_dirty_texts(world, assets, cmd, current_frame);
        }

        let mut offset = 0;

        let staging_buffer = &mut self.staging_buffers[current_frame];
        let sb = staging_buffer.buffer();
        let vb = self.mesh.vertex_buffer().into();

        let mut query = Query::new(TextQuery::new());
        let mut query = query.borrow(world);
        let dirty_texts = query
            .iter()
            .filter(|item| {
                item.text.len() > 0
                    && (item.text.dirty()
                        || item.text.old_bounds() != *item.bounds
                        || item.text.old_margin() != *item.margin
                        || &item.text.old_wrap() != item.wrap)
            })
            .flat_map(|item| {
                item.text.set_dirty(false);

                let size = (item.text.len() * 4 * size_of::<UIVertex>()) as u64;

                let region = &[BufferCopy {
                    src_offset: offset,
                    dst_offset: item.block.offset() as u64 * 4 * size_of::<UIVertex>() as u64,
                    size,
                }];

                cmd.copy_buffer(sb, vb, region);

                offset += size;

                item.text
                    .layout(
                        item.font,
                        *item.bounds,
                        *item.wrap,
                        *item.alignment,
                        *item.margin,
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
        assets: &AssetCache,
        cmd: &ivy_vulkan::CommandBuffer,
        sets: &[DescriptorSet],
        pass_info: &PassInfo,
        offsets: &[u32],
        current_frame: usize,
        shaderpass: Component<Shader>,
    ) -> anyhow::Result<()> {
        return Ok(());
        cmd.bind_vertexbuffer(0, self.mesh.vertex_buffer());
        cmd.bind_indexbuffer(self.mesh.index_buffer(), IndexType::UINT32, 0);

        let pass = self.base_renderer.pass_mut(shaderpass)?;
        {
            pass.register(world, KeyQuery::new());
            pass.build_batches(world, pass_info)?;
            pass.update(
                current_frame,
                Query::new((
                    entity_ids(),
                    batch_id(shaderpass.id()),
                    ObjectDataQuery::new(),
                    visible(),
                ))
                .borrow(world)
                .iter()
                .filter_map(|(id, &marker, object, visible)| {
                    if visible.is_visible() {
                        Some((id, marker, ObjectData::from(object)))
                    } else {
                        None
                    }
                }),
            )?;
        }

        let frame_set = pass.set(current_frame);

        let object_buffer = pass.object_buffer(current_frame);
        let object_data = object_buffer
            .mapped_slice::<ObjectData>()
            .expect("Non mappable object data buffer");

        for batch in pass.batches().iter() {
            let key = batch.key();

            cmd.bind_pipeline(batch.pipeline());

            if !sets.is_empty() {
                cmd.bind_descriptor_sets(batch.layout(), 0, sets, offsets);
            }

            cmd.bind_descriptor_sets(
                batch.layout(),
                sets.len() as u32,
                &[frame_set, key.font.set(0)],
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

#[derive(Fetch)]
struct ObjectDataQuery {
    position: Component<Vec3>,
    color: Component<Color>,
    text: Component<Text>,
    block: Component<BufferAllocation<Marker>>,
}

impl ObjectDataQuery {
    pub fn new() -> Self {
        Self {
            position: position(),
            color: color(),
            text: text(),
            block: block(),
        }
    }
}

impl<'a> From<ObjectDataQueryItem<'a>> for ObjectData {
    fn from(value: ObjectDataQueryItem) -> ObjectData {
        ObjectData {
            mvp: Mat4::from_translation(*value.position),
            color: value.color.to_vec4(),
            offset: value.block.offset() as u32,
            len: value.text.len() as u32,
        }
    }
}

#[derive(Fetch)]
struct KeyQuery {
    font: Component<Asset<Font>>,
}

impl KeyQuery {
    fn new() -> Self {
        Self { font: font() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Key {
    font: Asset<Font>,
}

impl<'a> From<KeyQueryItem<'a>> for Key {
    fn from(value: KeyQueryItem) -> Self {
        Self {
            font: value.font.clone(),
        }
    }
}
pub struct TextUpdateNode {
    text_renderer: Arc<Mutex<TextRenderer>>,
    buffer: vk::Buffer,
}

impl TextUpdateNode {
    pub fn new(assets: &AssetCache, text_renderer: Arc<Mutex<TextRenderer>>) -> Result<Self> {
        let buffer = text_renderer.lock().vertex_buffer();

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
        assets: &AssetCache,
        cmd: &CommandBuffer,
        _: &PassInfo,
        current_frame: usize,
    ) -> anyhow::Result<()> {
        self.text_renderer
            .lock()
            .update_dirty_texts(world, assets, cmd, current_frame)
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
