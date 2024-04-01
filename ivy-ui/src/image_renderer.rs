use crate::*;
use flax::{entity_ids, Component, Fetch, Query, World};
use glam::{vec2, vec3, Mat4, Vec2, Vec3, Vec4};
use ivy_base::{color, position, size, Color};
use ivy_graphics::{batch_id, BaseRenderer, Mesh, Renderer};
use ivy_resources::{Handle, Resources};
use ivy_vulkan::{context::SharedVulkanContext, descriptors::*, vk::IndexType, PassInfo, Shader};

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
                UIVertex::new(vec3(-1.0, -1.0, 0.0), vec2(0.0, 1.0)),
                UIVertex::new(vec3(1.0, -1.0, 0.0), vec2(1.0, 1.0)),
                UIVertex::new(vec3(1.0, 1.0, 0.0), vec2(1.0, 0.0)),
                UIVertex::new(vec3(-1.0, 1.0, 0.0), vec2(0.0, 0.0)),
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
    /// Draw all entities with a material, mesh, and model matrix for the specified shaderpass.
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
        cmd.bind_vertexbuffer(0, self.square.vertex_buffer());
        cmd.bind_indexbuffer(self.square.index_buffer(), IndexType::UINT32, 0);

        let images = resources.fetch::<Image>()?;

        let pass = self.base_renderer.pass_mut(shaderpass)?;

        pass.register(world, KeyQuery::new());
        pass.build_batches(world, resources, pass_info)?;

        pass.update(
            current_frame,
            Query::new((
                entity_ids(),
                batch_id(shaderpass.id()),
                ObjectDataQuery::new(),
            ))
            .borrow(world)
            .iter()
            .filter_map(|(e, &batch_id, obj /* , bound */)| {
                // if visible.is_visible()
                //     && camera.visible(**obj.position, **bound * obj.scale.max_element())
                // {
                Some((e, batch_id, ObjectData::from(obj)))
                // } else {
                //     None
                // }
            }),
        )?;

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

#[derive(Fetch)]
struct ObjectDataQuery<'a> {
    position: Component<Vec3>,
    size: Component<Vec2>,
    color: Component<Color>,
}

impl<'a> ObjectDataQuery<'a> {
    fn new() -> Self {
        Self {
            position: position(),
            size: size(),
            color: color(),
        }
    }
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

#[derive(Fetch, PartialEq)]
struct KeyQuery {
    depth: Component<u32>,
    image: Component<Handle<Image>>,
}

impl KeyQuery {
    pub fn new() -> Self {
        Self {
            depth: widget_depth(),
            image: image(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    depth: u32,
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

impl From<KeyQueryItem<'_>> for Key {
    fn from(item: KeyQueryItem) -> Self {
        Self {
            depth: *item.depth,
            image: *item.image,
        }
    }
}
