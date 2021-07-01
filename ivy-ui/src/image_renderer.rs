use hecs::{Entity, World};
use ivy_graphics::{components::ModelMatrix, Error, Mesh, ShaderPass};
use ivy_resources::{Handle, ResourceCache, ResourceManager};
use ivy_vulkan::{
    commands::CommandBuffer, descriptors::*, vk, Buffer, BufferAccess, BufferType, VulkanContext,
};

use std::{any::TypeId, collections::HashMap, marker::PhantomData, mem::size_of, sync::Arc};

use crate::image::Image;

/// Any entity with these components will be renderered.
type RenderObject<'a, T> = (
    &'a Handle<T>,
    &'a Handle<Image>,
    &'a ModelMatrix,
    &'a ObjectBufferMarker,
);

/// Same as RenderObject except without ObjectBufferMarker
type RenderObjectUnregistered<'a, T> = (&'a Handle<T>, &'a Handle<Image>, &'a ModelMatrix);

type ObjectId = u32;

/// A mesh renderer using vkCmdDrawIndirectIndexed and efficient batching.
pub struct ImageRenderer {
    context: Arc<VulkanContext>,
    descriptor_allocator: DescriptorAllocator,
    square: Mesh,
    frames: Vec<FrameData>,
    passes: HashMap<TypeId, PassData>,
    max_object_id: ObjectId,
    free_indices: Vec<ObjectId>,
    /// Maximum number of objects that fit inside objectbuffer
    capacity: ObjectId,
    /// Number of registered entities
    object_count: ObjectId,
    frames_in_flight: usize,
}

impl ImageRenderer {
    pub fn new(
        context: Arc<VulkanContext>,
        descriptor_layout_cache: &mut DescriptorLayoutCache,
        capacity: ObjectId,
        frames_in_flight: usize,
    ) -> Result<Self, Error> {
        let mut descriptor_allocator =
            DescriptorAllocator::new(context.device().clone(), frames_in_flight as u32);

        let frames = (0..frames_in_flight)
            .map(|_| {
                FrameData::new(
                    context.clone(),
                    descriptor_layout_cache,
                    &mut descriptor_allocator,
                    capacity,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        let passes = HashMap::new();

        let square = Mesh::new_square(context.clone(), 1.0, 1.0)?;

        Ok(Self {
            context,
            descriptor_allocator,
            square,
            frames,
            passes,
            max_object_id: 0,
            free_indices: Vec::new(),
            capacity,
            object_count: 0,
            frames_in_flight,
        })
    }

    /// Gets a free object buffer id
    fn get_object_id(&mut self) -> ObjectId {
        if let Some(id) = self.free_indices.pop() {
            id
        } else {
            let id = self.max_object_id;
            self.max_object_id += 1;

            id
        }
    }

    fn resize_object_buffer(
        &mut self,
        world: &mut World,
        capacity: ObjectId,
        descriptor_layout_cache: &mut DescriptorLayoutCache,
    ) -> Result<(), Error> {
        self.free_indices.clear();
        self.max_object_id = 0;

        self.frames.clear();
        self.descriptor_allocator.reset()?;

        for _ in 0..self.frames_in_flight {
            self.frames.push(FrameData::new(
                self.context.clone(),
                descriptor_layout_cache,
                &mut self.descriptor_allocator,
                capacity,
            )?);
        }

        self.capacity = capacity;

        world
            .query_mut::<&mut ObjectBufferMarker>()
            .into_iter()
            .for_each(|(_, marker)| marker.id = self.get_object_id());

        Ok(())
    }

    /// Registers all unregisters entities capable of being rendered for specified pass. Does
    /// nothing if entities are already registered. Call this function after adding new entities to the world.
    /// # Failures
    /// Fails if object buffer cannot be reallocated to accomodate new entities.
    pub fn register_entities<T: 'static + ShaderPass + Send + Sync>(
        &mut self,
        world: &mut World,
        descriptor_layout_cache: &mut DescriptorLayoutCache,
    ) -> Result<(), Error> {
        let query = world
            .query_mut::<RenderObjectUnregistered<T>>()
            .without::<ObjectBufferMarker>();

        let inserted = query
            .into_iter()
            .map(|(e, _)| {
                self.object_count += 1;
                e
            })
            .collect::<Vec<_>>();

        inserted.into_iter().for_each(|e| {
            world
                .insert_one(
                    e,
                    ObjectBufferMarker {
                        id: self.get_object_id(),
                    },
                )
                .unwrap();
        });

        if self.max_object_id > self.capacity {
            self.resize_object_buffer(
                world,
                nearest_power_2(self.object_count as _) as _,
                descriptor_layout_cache,
            )?;
        }

        Ok(())
    }

    /// Updates all registered entities gpu side data
    pub fn update(&mut self, world: &mut World, current_frame: usize) -> Result<(), Error> {
        let query = world.query_mut::<(&ModelMatrix, &ObjectBufferMarker)>();

        let frame = &mut self.frames[current_frame];

        frame
            .object_buffer
            .write_slice(self.max_object_id as u64, 0, |data| {
                query.into_iter().for_each(|(_, (modelmatrix, marker))| {
                    data[marker.id as usize] = ObjectData { mvp: *modelmatrix }
                });
            })?;

        Ok(())
    }

    /// Will draw all entities with a Handle<Material>, Handle<Mesh>, Modelmatrix and Shaderpass `Handle<T>`
    pub fn draw<T: 'static + ShaderPass + Sized + Sync + Send>(
        &mut self,
        world: &mut World,
        cmd: &CommandBuffer,
        current_frame: usize,
        global_set: DescriptorSet,
        dynamic_offsets: &[u32],
        resources: &ResourceManager,
    ) -> Result<(), Error> {
        let frame = &mut self.frames[current_frame];

        let frame_set = frame.set;

        let pass = match self.passes.get_mut(&TypeId::of::<T>()) {
            Some(pass) => pass,
            None => {
                self.passes.insert(
                    TypeId::of::<T>(),
                    PassData::new(self.context.clone(), 8, self.frames_in_flight)?,
                );
                self.passes.get_mut(&TypeId::of::<T>()).unwrap()
            }
        };

        let passes = resources.cache()?;
        let images = resources.cache()?;

        pass.build_batches::<T>(world, &passes)?;

        pass.draw(
            cmd,
            current_frame,
            global_set,
            frame_set,
            dynamic_offsets,
            &self.square,
            &images,
        )?;

        Ok(())
    }
}

struct PassData {
    context: Arc<VulkanContext>,
    batches: Vec<BatchData>,
    batch_map: HashMap<BatchKey, usize>,
    indirect_buffers: Vec<Buffer>,
    object_count: ObjectId,
    /// Dirty indirect buffers
    dirty: Vec<bool>,
    capacity: ObjectId,
}

impl PassData {
    pub fn new(
        context: Arc<VulkanContext>,
        capacity: ObjectId,
        frames_in_flight: usize,
    ) -> Result<Self, Error> {
        let indirect_buffers = (0..frames_in_flight)
            .map(|_| create_indirect_buffer(context.clone(), capacity))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            context,
            batches: Vec::new(),
            batch_map: HashMap::new(),
            indirect_buffers,
            object_count: 0,
            dirty: vec![false; frames_in_flight],
            capacity,
        })
    }

    /// Resizes the indirect buffers
    pub fn resize_indirect_buffer(&mut self, capacity: ObjectId) -> Result<(), Error> {
        // TODO use fence epoch garbage collection
        ivy_vulkan::device::wait_idle(self.context.device())?;

        for buffer in &mut self.indirect_buffers {
            *buffer = create_indirect_buffer(self.context.clone(), capacity)?;
        }

        self.dirty = vec![true; self.dirty.len()];

        self.capacity = capacity;

        Ok(())
    }

    /// Builds rendering batches for shaderpass `T` for all objects not yet batched.
    pub fn build_batches<T: 'static + ShaderPass + Send + Sync>(
        &mut self,
        world: &mut World,
        passes: &ResourceCache<T>,
    ) -> Result<(), Error> {
        let query = world
            .query_mut::<RenderObject<T>>()
            .without::<BatchMarker<T>>();

        let unbatched = query
            .into_iter()
            .map(|(e, renderobject)| self.insert_entity::<T>(e, renderobject, passes))
            .collect::<Result<Vec<_>, _>>()?;

        if !unbatched.is_empty() {
            unbatched.into_iter().for_each(|(e, marker)| {
                world.insert_one(e, marker).unwrap();
            });
        }

        Ok(())
    }

    /// Builds the indirect buffers for frame
    pub fn build_indirect(&mut self, current_frame: usize) -> Result<(), Error> {
        let mut index = 0;
        let indirect_buffer = &mut self.indirect_buffers[current_frame];
        let batches = &mut self.batches;

        indirect_buffer.write_slice(self.object_count as u64, 0, |data| -> Result<(), Error> {
            for batch in batches {
                // Update batch offset
                batch.offset = index;

                let index_count = 6;

                for id in &batch.ids {
                    data[index as usize] = IndirectObject {
                        cmd: vk::DrawIndexedIndirectCommand {
                            first_instance: *id,
                            vertex_offset: 0,
                            first_index: 0,
                            instance_count: 1,
                            index_count,
                        },
                    };
                    index += 1;
                }
            }
            Ok(())
        })?;

        self.dirty[current_frame] = false;

        Ok(())
    }

    /// Inserts a new entity into the correct batch. Note: The entity should not already exist in pass,
    /// behaviour is undefined.
    pub fn insert_entity<'a, T: 'static + ShaderPass>(
        &mut self,
        entity: Entity,
        renderobject: RenderObject<'a, T>,
        passes: &ResourceCache<T>,
    ) -> Result<(Entity, BatchMarker<T>), Error> {
        let (shaderpass, image, _modelmatrix, object_marker) = renderobject;

        let shaderpass = passes.get(*shaderpass)?;
        let (_, batch) = self.get_batch(shaderpass, *image);

        batch.ids.push(object_marker.id);
        self.dirty = vec![true; self.dirty.len()];
        self.object_count += 1;

        Ok((
            entity,
            BatchMarker {
                _shaderpass: PhantomData,
            },
        ))
    }

    pub fn get_batch<T: ShaderPass>(
        &mut self,
        shaderpass: &T,
        image: Handle<Image>,
    ) -> (usize, &mut BatchData) {
        let idx = match self.batch_map.get(&(shaderpass.pipeline().into(), image)) {
            Some(val) => *val,
            None => {
                self.batches.push(BatchData::new(shaderpass, image));
                self.batch_map.insert(
                    (shaderpass.pipeline().into(), image),
                    self.batches.len() - 1,
                );
                self.batches.len() - 1
            }
        };

        (idx, &mut self.batches[idx])
    }

    /// Draws all batches
    pub fn draw(
        &mut self,
        cmd: &CommandBuffer,
        current_frame: usize,
        global_set: DescriptorSet,
        frame_set: DescriptorSet,
        dynamic_offsets: &[u32],
        square: &Mesh,
        images: &ResourceCache<Image>,
    ) -> Result<(), Error> {
        // Indirect buffer is not large enough
        if self.object_count > self.capacity {
            self.resize_indirect_buffer(nearest_power_2(self.object_count as _) as _)?;
        }

        // Rebuild indirect buffers for this frame
        if self.dirty[current_frame] {
            self.build_indirect(current_frame)?;
        }

        for batch in &self.batches {
            let image = images.get(batch.image)?;
            cmd.bind_descriptor_sets(
                batch.pipeline_layout,
                0,
                &[global_set, frame_set, image.set()],
                dynamic_offsets,
            );

            cmd.bind_pipeline(batch.pipeline);
            cmd.bind_vertexbuffer(0, square.vertex_buffer());
            cmd.bind_indexbuffer(square.index_buffer(), 0);

            cmd.draw_indexed_indirect(
                &self.indirect_buffers[current_frame],
                batch.offset * size_of::<IndirectObject>() as u64,
                batch.ids.len() as u32,
                size_of::<IndirectObject>() as u32,
            )
        }

        Ok(())
    }
}

pub type BatchKey = (vk::Pipeline, Handle<Image>);

/// A batch contains objects of the same shaderpass and material.
struct BatchData {
    pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,
    image: Handle<Image>,
    /// The number of draw calls in batch
    /// Indices into the object buffer for objects to draw
    ids: Vec<u32>,

    /// Offset into the indirect buffer
    offset: u64,
}

impl BatchData {
    fn new<T: ShaderPass>(shaderpass: &T, image: Handle<Image>) -> Self {
        Self {
            pipeline: shaderpass.pipeline().into(),
            pipeline_layout: shaderpass.pipeline_layout(),
            image,
            ids: Vec::new(),
            offset: 0,
        }
    }
}

struct ObjectBufferMarker {
    /// Index into the object buffer
    id: ObjectId,
}

/// Marks the entity as already being batched for this shaderpasss with the batch index and object buffer index.
struct BatchMarker<T> {
    _shaderpass: PhantomData<T>,
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
        capacity: ObjectId,
    ) -> Result<Self, Error> {
        let object_buffer = Buffer::new_uninit(
            context.clone(),
            BufferType::Storage,
            BufferAccess::MappedPersistent,
            size_of::<ObjectData>() as u64 * capacity as u64,
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
    mvp: ModelMatrix,
}

#[repr(C)]
struct IndirectObject {
    cmd: vk::DrawIndexedIndirectCommand,
}

fn create_indirect_buffer(
    context: Arc<VulkanContext>,
    capacity: ObjectId,
) -> Result<Buffer, ivy_vulkan::Error> {
    Buffer::new_uninit(
        context,
        BufferType::Indirect,
        BufferAccess::Mapped,
        capacity as u64 * size_of::<IndirectObject>() as u64,
    )
}

fn nearest_power_2(val: usize) -> usize {
    let mut result = 1;
    while result < val {
        result *= 2;
    }
    result
}
