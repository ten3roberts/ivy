use crate::Result;
use hecs::{Entity, Fetch, Query, World};
use ivy_resources::{Handle, HandleUntyped, ResourceCache};
use ivy_vulkan::{descriptors::*, vk, Buffer, BufferAccess, BufferUsage, VulkanContext};

use std::{
    any::TypeId, collections::HashMap, marker::PhantomData, mem::size_of, ops::Deref, sync::Arc,
};

use crate::ShaderPass;

pub trait KeyQuery: Send + Sync + Query {
    type K: Key;
    fn into_key(&self) -> Self::K;
}

pub trait Key: std::hash::Hash + std::cmp::Eq + Copy {}

impl<T> Key for T where T: std::hash::Hash + std::cmp::Eq + Copy {}

type ObjectId = u32;

/// A mesh renderer using vkCmdDrawIndirectIndexed and efficient batching.
/// A query and key are provided. On register, all entites satisfying the
/// `KeyQuery` will be placed into the object buffer. Objects will then be
/// placed into the correct batch according to their shaderpass and key hash.
/// This means that if the key is made of a Material and Mesh, all objects with
/// the same pipeline, material, and mesh will be placed in the same batch.
pub struct BaseRenderer<K, Obj> {
    context: Arc<VulkanContext>,
    descriptor_allocator: DescriptorAllocator,
    frames: Vec<FrameData<Obj>>,
    passes: HashMap<TypeId, PassData<K>>,
    max_object_id: ObjectId,
    free_indices: Vec<ObjectId>,
    /// Maximum number of objects that fit inside objectbuffer
    capacity: ObjectId,
    /// Number of registered entities
    object_count: ObjectId,
    // Entities to insert into a pass and batch
    unbatched: Vec<Entity>,
}

impl<'a, Q, F, Obj, K> BaseRenderer<Q, Obj>
where
    Q: KeyQuery + Query<Fetch = F> + ToOwned<Owned = K>,
    K: Key,
    F: Fetch<'a, Item = Q>,
    Obj: Send + Sync,
{
    pub fn new(
        context: Arc<VulkanContext>,
        capacity: ObjectId,
        frames_in_flight: usize,
    ) -> Result<Self> {
        let descriptor_allocator =
            DescriptorAllocator::new(context.device().clone(), frames_in_flight as u32);

        let frames = (0..frames_in_flight)
            .map(|_| FrameData::new(context.clone(), capacity))
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let passes = HashMap::new();

        Ok(Self {
            context,
            descriptor_allocator,
            frames,
            passes,
            max_object_id: 0,
            free_indices: Vec::new(),
            capacity,
            object_count: 0,
            unbatched: Vec::new(),
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
    /// Resizes the object storage buffer to fit `capacity` objects

    fn resize_object_buffer(&mut self, world: &mut World, capacity: ObjectId) -> Result<()> {
        self.free_indices.clear();
        self.max_object_id = 0;

        let len = self.frames.len();

        self.frames.clear();
        self.descriptor_allocator.reset()?;

        for _ in 0..len {
            self.frames
                .push(FrameData::new(self.context.clone(), capacity)?);
        }

        self.capacity = capacity;

        world
            .query_mut::<&mut ObjectBufferMarker>()
            .into_iter()
            .for_each(|(_, marker)| marker.id = self.get_object_id());

        Ok(())
    }

    /// Registers all entities with key into the the object buffer. Does not
    /// insert entities into pass, which is done with [`PassData::build_batches`].
    /// nothing if entities are already registered. Call this function after adding new entities to the world.
    /// # Failures
    /// Fails if object buffer cannot be reallocated to accomodate new entities.
    pub fn register_entities(&mut self, world: &'a mut World) -> Result<()> {
        let query = world.query_mut::<Q>().without::<ObjectBufferMarker>();

        self.unbatched
            .extend(query.into_iter().map(|(e, _)| e).collect::<Vec<_>>());

        Ok(())
    }

    /// Updates all registered entities gpu side data.
    /// Inserts not yet inserted entities
    pub fn update<T, G, U>(&mut self, world: &mut World, current_frame: usize) -> Result<()>
    where
        T: Query<Fetch = G>,
        G: for<'x> Fetch<'x, Item = U>,
        U: Into<Obj>,
    {
        let max_object_id = &mut self.max_object_id;
        let free_indices = &mut self.free_indices;

        // Add the marker to keep track of an entity's postion in the object
        // buffer
        self.unbatched.drain(0..).try_for_each(|e| {
            world.insert_one(
                e,
                ObjectBufferMarker {
                    id: {
                        if let Some(id) = free_indices.pop() {
                            id
                        } else {
                            let id = *max_object_id;
                            *max_object_id += 1;

                            id
                        }
                    },
                },
            )
        })?;

        if self.max_object_id > self.capacity {
            self.resize_object_buffer(world, nearest_power_2(self.object_count as _) as _)?;
        }

        let query = world.query_mut::<(T, &ObjectBufferMarker)>();

        let frame = &mut self.frames[current_frame];

        frame
            .object_buffer
            .write_slice(self.max_object_id as u64, 0, |data| {
                query.into_iter().for_each(|(_, (g, marker))| {
                    data[marker.id as usize] = g.into();
                });
            })?;

        Ok(())
    }

    /// Returns the pass data for the shaderpass.
    pub fn pass<Pass: ShaderPass>(&mut self) -> &mut PassData<Q> {
        let frames_in_flight = self.frames.len();

        self.passes
            .entry(TypeId::of::<Pass>())
            .or_insert_with(|| PassData::new(frames_in_flight))
    }

    pub fn set(&self, current_frame: usize) -> DescriptorSet {
        self.frames[current_frame].set
    }

    pub fn object_count(&self) -> ObjectId {
        self.object_count
    }

    pub fn capacity(&self) -> ObjectId {
        self.capacity
    }

    pub fn max_object_id(&self) -> ObjectId {
        self.max_object_id
    }
}

/// Represents a single typed shaderpass. Each object belonging to the pass is
/// grouped into batches
pub struct PassData<K> {
    batches: Vec<BatchData<K>>,
    // Map from key to index in batches
    batch_map: HashMap<(HandleUntyped, Q::Owned), usize>,
    object_count: ObjectId,
    frames_in_flight: usize,
    unbatched: Vec<(Entity, HandleUntyped, ObjectBufferMarker, Q::Owned)>,
}

impl<'a, Q, F, K> PassData<Q>
where
    Q: KeyQuery + Query<Fetch = F> + ToOwned<Owned = K>,
    K: Key,
    F: Fetch<'a, Item = Q>,
{
    pub fn new(frames_in_flight: usize) -> Self {
        Self {
            batches: Vec::new(),
            batch_map: HashMap::new(),
            object_count: 0,
            frames_in_flight,
            unbatched: Vec::new(),
        }
    }

    /// Builds rendering batches for shaderpass `T` for all objects not yet batched.
    /// Note: [`get_unbatched`] needs to be run before to collect unbatched
    /// entities, this is due to lifetime limitations on world mutations.
    pub fn build_batches<Pass, U>(&mut self, world: &'a mut World, passes: &U) -> Result<()>
    where
        U: Deref<Target = ResourceCache<Pass>>,
        Pass: ShaderPass,
    {
        let frames_in_flight = self.frames_in_flight;
        let object_count = &mut self.object_count;
        let batch_map = &mut self.batch_map;
        let batches = &mut self.batches;

        // Insert a marker to track this enemy as attached to a batch
        self.unbatched
            .drain(0..)
            .try_for_each(|(e, pass, marker, k)| -> Result<_> {
                let marker = Self::insert_entity(
                    batch_map,
                    batches,
                    object_count,
                    e,
                    pass,
                    marker,
                    k,
                    passes,
                    frames_in_flight,
                )?;
                world.insert_one(e, marker)?;

                Ok(())
            })?;

        Ok(())
    }

    /// Collects all entities added to the base renderer that have yet to be
    /// placed into a batch for this shaderpass.
    pub fn get_unbatched<Pass>(&mut self, world: &'a mut World)
    where
        Pass: ShaderPass,
    {
        let query = world
            .query_mut::<(&Handle<Pass>, &ObjectBufferMarker, Q)>()
            .without::<BatchMarker<Pass>>();

        self.unbatched
            .extend(query.into_iter().map(|(e, (pass, marker, key))| {
                (e, pass.into_untyped(), marker.to_owned(), key.to_owned())
            }));
    }

    /// Inserts a new entity into the correct batch. Note: The entity should not already exist in pass,
    /// behaviour is undefined. Marks the batch as dirty.
    fn insert_entity<Pass: ShaderPass>(
        batch_map: &mut HashMap<(HandleUntyped, Q::Owned), usize>,
        batches: &mut Vec<BatchData<Q::Owned>>,
        object_count: &mut ObjectId,
        entity: Entity,
        pass: HandleUntyped,
        marker: ObjectBufferMarker,
        owned_key: <Q as ToOwned>::Owned,
        passes: &ResourceCache<Pass>,
        frames_in_flight: usize,
    ) -> Result<(Entity, BatchMarker<Pass>)> {
        let frames_in_flight = frames_in_flight;

        let (_, batch) = Self::get_batch_internal(batch_map, batches, passes, pass, owned_key)?;

        batch.ids.push(marker.id);
        batch.dirty = frames_in_flight;
        *object_count += 1;

        Ok((entity, BatchMarker(PhantomData)))
    }

    /// Returns or creates the appropriate batch for the combined shaderpass and
    /// key
    pub fn get_batch<U, Pass>(
        &mut self,
        passes: U,
        pass: Handle<Pass>,
        key: K,
    ) -> Result<&mut BatchData<K>>
    where
        U: Deref<Target = ResourceCache<Pass>>,
        Pass: ShaderPass,
    {
        Self::get_batch_internal(
            &mut self.batch_map,
            &mut self.batches,
            passes,
            pass.into_untyped(),
            key,
        )
        .map(|val| val.1)
    }
    ///
    fn get_batch_internal<'b, U, Pass>(
        batch_map: &mut HashMap<(HandleUntyped, Q::Owned), usize>,
        batches: &'b mut Vec<BatchData<K>>,
        passes: U,
        pass: HandleUntyped,
        key: K,
    ) -> Result<(usize, &'b mut BatchData<Q::Owned>)>
    where
        U: Deref<Target = ResourceCache<Pass>>,
        Pass: ShaderPass,
    {
        let combined_key = (pass, key);

        let idx = match batch_map.get(&combined_key) {
            Some(val) => *val,
            None => {
                let shaderpass = passes.get(Handle::from_untyped(pass))?;

                let pipeline = shaderpass.pipeline();
                batches.push(BatchData::new(
                    pipeline.into(),
                    pipeline.layout().into(),
                    pass,
                    key,
                ));
                batch_map.insert(combined_key, batches.len() - 1);
                batches.len() - 1
            }
        };

        Ok((idx, &mut batches[idx]))
    }

    /// Get a reference to the pass data's batches.
    pub fn batches(&self) -> &[BatchData<Q::Owned>] {
        self.batches.as_slice()
    }

    /// Get a reference to the pass data's object count.
    pub fn object_count(&self) -> ObjectId {
        self.object_count
    }
}

/// A batch contains objects of the same shaderpass and material.
pub struct BatchData<K> {
    pipeline: vk::Pipeline,
    layout: vk::PipelineLayout,
    pass: HandleUntyped,
    owned_key: K,
    /// The number of draw calls in batch
    /// Indices into the object buffer for objects to draw
    ids: Vec<ObjectId>,

    /// Set to frames_in_flight when the batch is dirty.
    dirty: usize,
}

impl<O> BatchData<O> {
    fn new(
        pipeline: vk::Pipeline,
        layout: vk::PipelineLayout,
        pass: HandleUntyped,
        owned_key: O,
    ) -> Self {
        Self {
            pipeline,
            layout,
            pass,
            owned_key,
            ids: Vec::new(),
            dirty: 0,
        }
    }

    pub fn pipeline(&self) -> vk::Pipeline {
        self.pipeline
    }

    pub fn layout(&self) -> vk::PipelineLayout {
        self.layout
    }

    pub fn key(&self) -> &O {
        &self.owned_key
    }

    pub fn shaderpass<T>(&self) -> Handle<T> {
        Handle::from_untyped(self.pass)
    }

    /// Ids into the object buffer associated to this batch
    pub fn ids(&self) -> &[ObjectId] {
        &self.ids
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ObjectBufferMarker {
    /// Index into the object buffer
    id: ObjectId,
}

/// Marks the entity as already being batched for this shaderpasss with the batch index and object buffer index.
struct BatchMarker<T>(PhantomData<T>);

struct FrameData<T> {
    set: DescriptorSet,
    object_buffer: Buffer,
    marker: PhantomData<T>,
}

impl<T> FrameData<T> {
    pub fn new(context: Arc<VulkanContext>, capacity: ObjectId) -> Result<Self> {
        let object_buffer = Buffer::new_uninit(
            context.clone(),
            BufferUsage::STORAGE_BUFFER,
            BufferAccess::Mapped,
            size_of::<T>() as u64 * capacity as u64,
        )?;

        let set = DescriptorBuilder::new()
            .bind_buffer(0, vk::ShaderStageFlags::VERTEX, &object_buffer)?
            .build(&context)?;

        Ok(Self {
            set,
            object_buffer,
            marker: PhantomData,
        })
    }
}

fn nearest_power_2(val: usize) -> usize {
    let mut result = 1;
    while result < val {
        result *= 2;
    }
    result
}
