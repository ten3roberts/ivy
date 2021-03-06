use crate::*;
use glam::{Mat4, Vec2, Vec3, Vec4};
use hecs::{Query, World};
use ivy_base::{Color, Position2D, Size2D, Visible};
use ivy_graphics::{BaseRenderer, BatchMarker, Mesh, Renderer};
use ivy_resources::{Handle, Resources};
use ivy_vulkan::{
    context::SharedVulkanContext, descriptors::*, shaderpass::ShaderPass, vk::IndexType, PassInfo,
};

/// A mesh renderer using vkCmdDrawIndirectIndexed and efficient batching.
pub struct ImageRenderer {
    square: Mesh<UIVertex>,
    base_renderer: BaseRenderer<Key, ObjectData, UIVertex>,
}

impl ImageRenderer {
    pub fn new(
        context: SharedVulkanContext,
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

            Mesh::new(context, &vertices, &indices, vec![])
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
        world: &mut World,
        resources: &Resources,
        cmd: &ivy_vulkan::CommandBuffer,
        sets: &[DescriptorSet],
        pass_info: &PassInfo,
        offsets: &[u32],
        current_frame: usize,
    ) -> Result<()> {
        cmd.bind_vertexbuffer(0, self.square.vertex_buffer());
        cmd.bind_indexbuffer(self.square.index_buffer(), IndexType::UINT32, 0);

        let images = resources.fetch::<Image>()?;

        let pass = self.base_renderer.pass_mut::<Pass>()?;

        pass.register::<Pass, KeyQuery, ObjectDataQuery>(world);
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
#[derive(Default, Debug, Clone, Copy, PartialEq)]
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
            mvp: Mat4::from_translation(self.position.extend(0.0))
                * Mat4::from_scale(self.size.extend(1.0)),
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
