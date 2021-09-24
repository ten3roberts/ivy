use crate::{image::Image, Error, Result, UIVertex};
use hecs::{Query, World};
use ivy_core::ModelMatrix;
use ivy_graphics::{BaseRenderer, Mesh, Renderer, ShaderPass};
use ivy_resources::{Handle, Resources};
use ivy_vulkan::vk::IndexType;
use ivy_vulkan::{commands::CommandBuffer, descriptors::*, VulkanContext};
use ultraviolet::{Vec2, Vec3};

use std::sync::Arc;

/// Same as RenderObject except without ObjectBufferMarker
type ObjectId = u32;

/// A mesh renderer using vkCmdDrawIndirectIndexed and efficient batching.
pub struct ImageRenderer {
    square: Mesh<UIVertex>,
    base_renderer: BaseRenderer<Key, ObjectData>,
}

impl ImageRenderer {
    pub fn new(
        context: Arc<VulkanContext>,
        capacity: ObjectId,
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

    /// Updates all registered entities gpu side data
    pub fn update(&mut self, world: &mut World, current_frame: usize) -> Result<()> {
        self.base_renderer.register_entities::<KeyQuery, _>(world)?;
        self.base_renderer
            .update::<KeyQuery, ObjectDataQuery, _, _>(world, current_frame)?;

        Ok(())
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

        let frame_set = self.base_renderer.set(current_frame);

        let pass = self.base_renderer.pass_mut::<Pass>();

        pass.get_unbatched::<Pass, KeyQuery, _>(world);
        pass.build_batches::<Pass, KeyQuery, _, _>(world, &passes)?;

        for batch in pass.batches() {
            let key = batch.key();

            let image = resources.get(key.image)?;

            if !sets.is_empty() {
                cmd.bind_descriptor_sets(batch.layout(), 0, sets, offsets);
            }

            cmd.bind_descriptor_sets(
                batch.layout(),
                sets.len() as u32,
                &[frame_set, image.set(0)],
                &[],
            );

            cmd.bind_pipeline(batch.pipeline());

            println!("Image rendering");
            for id in batch.ids() {
                println!("Drawing: {}", id);
                cmd.draw_indexed(6, 1, 0, 0, *id);
            }
        }

        Ok(())
        // let frame = &mut self.frames[current_frame];

        // let frame_set = frame.set;

        // let pass = match self.passes.get_mut(&TypeId::of::<Pass>()) {
        //     Some(pass) => pass,
        //     None => {
        //         self.passes.insert(
        //             TypeId::of::<Pass>(),
        //             PassData::new(self.context.clone(), 8, self.frames_in_flight)?,
        //         );
        //         self.passes.get_mut(&TypeId::of::<Pass>()).unwrap()
        //     }
        // };

        // let passes = resources.fetch()?;
        // let images = resources.fetch()?;

        // pass.build_batches::<Pass>(world, &passes)?;

        // pass.draw(
        //     cmd,
        //     current_frame,
        //     sets,
        //     offsets,
        //     frame_set,
        //     images.deref(),
        //     &self.square,
        // )?;

        // Ok(())
    }
}

#[repr(C, align(16))]
struct ObjectData {
    mvp: ModelMatrix,
}

#[derive(Query)]
struct ObjectDataQuery<'a> {
    mvp: &'a ModelMatrix,
}

impl<'a> Into<ObjectData> for ObjectDataQuery<'a> {
    fn into(self) -> ObjectData {
        ObjectData { mvp: *self.mvp }
    }
}

#[derive(Query, PartialEq, Eq)]
pub struct KeyQuery<'a> {
    image: &'a Handle<Image>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
    image: Handle<Image>,
}

impl<'a> ivy_graphics::KeyQuery for KeyQuery<'a> {
    type K = Key;

    fn into_key(&self) -> Self::K {
        Self::K { image: *self.image }
    }
}
