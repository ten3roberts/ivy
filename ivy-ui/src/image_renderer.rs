use crate::*;
use hecs::{Query, World};
use ivy_base::{Color, Position2D, Size2D};
use ivy_graphics::{BaseRenderer, Mesh, Renderer, ShaderPass};
use ivy_resources::{Handle, Resources};
use ivy_vulkan::{commands::CommandBuffer, descriptors::*, vk::IndexType, VulkanContext};
use std::sync::Arc;
use ultraviolet::{Mat4, Vec2, Vec3, Vec4};

/// A mesh renderer using vkCmdDrawIndirectIndexed and efficient batching.
pub struct ImageRenderer {
    square: Mesh<UIVertex>,
    base_renderer: BaseRenderer<Key, ObjectData>,
}

impl ImageRenderer {
    pub fn new(
        context: Arc<VulkanContext>,
        capacity: u32,
        frames_in_flight: usize,
    ) -> Result<Self> {
        let square = {
            let context = context.clone();

            // Simple quad
            let vertices = [
                UIVertex::new(Vec3::new(-1.0, -1.0, 0.0), Vec2::new(0.0, 1.0)),
                UIVertex::new(Vec3::new(1.0, -1.0, 0.0), Vec2::new(1.0, 1.0)),
                UIVertex::new(Vec3::new(1.0, 1.0, 0.0), Vec2::new(1.0, 0.0)),
                UIVertex::new(Vec3::new(-1.0, 1.0, 0.0), Vec2::new(0.0, 0.0)),
            ];

            let indices: [u32; 6] = [0, 1, 2, 2, 3, 0];

            Mesh::new(context, &vertices, &indices)
        }?;

        let base_renderer = BaseRenderer::new(context, capacity, frames_in_flight)?;

        Ok(Self {
            square,
            base_renderer,
        })
    }
}

impl Renderer for ImageRenderer {
    type Error = Error;
    /// Will draw all entities with a Handle<Material>, Handle<Mesh>, Modelmatrix and Shaderpass `Handle<T>`
    fn draw<Pass: ShaderPass>(
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
    ) -> Result<()> {
        cmd.bind_vertexbuffer(0, self.square.vertex_buffer());
        cmd.bind_indexbuffer(self.square.index_buffer(), IndexType::UINT32, 0);

        let passes = resources.fetch::<Pass>()?;
        let images = resources.fetch::<Image>()?;

        let pass = self.base_renderer.pass_mut::<Pass>()?;

        pass.get_unbatched::<Pass, KeyQuery, _>(world);
        pass.build_batches::<Pass, KeyQuery, _, _>(world, &passes)?;
        pass.update::<Pass, ObjectDataQuery, _>(world, current_frame)?;

        pass.sort_batches_if_dirty();

        let frame_set = pass.set(current_frame);

        for batch in pass.ordered_batches() {
            let key = batch.key();

            let image = images.get(key.image)?;

            cmd.bind_pipeline(batch.pipeline());

            if !sets.is_empty() {
                cmd.bind_descriptor_sets(batch.layout(), 0, sets, offsets);
            }

            cmd.bind_descriptor_sets(
                batch.layout(),
                sets.len() as u32,
                &[frame_set, image.set(0)],
                &[],
            );

            cmd.draw_indexed(6, batch.instance_count(), 0, 0, batch.first_instance());
        }

        Ok(())
    }
}

#[repr(C, align(16))]
struct ObjectData {
    mvp: Mat4,
    color: Vec4,
}

#[derive(Query)]
struct ObjectDataQuery<'a> {
    position: &'a Position2D,
    size: &'a Size2D,
    color: &'a Color,
}

impl<'a> Into<ObjectData> for ObjectDataQuery<'a> {
    fn into(self) -> ObjectData {
        ObjectData {
            mvp: Mat4::from_translation(self.position.xyz())
                * Mat4::from_nonuniform_scale(self.size.into_homogeneous_point()),
            color: self.color.into(),
        }
    }
}

#[derive(Query, PartialEq)]
struct KeyQuery<'a> {
    depth: &'a WidgetDepth,
    image: &'a Handle<Image>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    depth: WidgetDepth,
    image: Handle<Image>,
}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.depth.partial_cmp(&other.depth)
    }
}

impl Ord for Key {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.depth.cmp(&other.depth)
    }
}

impl<'a> ivy_graphics::KeyQuery for KeyQuery<'a> {
    type K = Key;

    fn into_key(&self) -> Self::K {
        Self::K {
            depth: *self.depth,
            image: *self.image,
        }
    }
}
