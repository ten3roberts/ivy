use std::{collections::HashMap, marker::PhantomData, ops::Deref, sync::Arc};

use crate::{KeyQuery, RendererKey};

use super::*;

use ash::vk::{DescriptorSet, ShaderStageFlags};
use hecs::{Entity, Fetch, Query, World};
use ivy_resources::{Handle, HandleUntyped, ResourceCache};
use ivy_vulkan::{
    descriptors::{DescriptorBuilder, IntoSet},
    device, Buffer, BufferAccess, BufferUsage, VulkanContext,
};

/// Represents a single typed shaderpass. Each object belonging to the pass is
/// grouped into batches
pub struct PassData<K, Obj> {
    context: Arc<VulkanContext>,
    batches: Vec<BatchData<K>>,
    /// Ordered access of batches
    ordered_batches: Vec<BatchId>,
    // Map from key to index in batches
    batch_map: HashMap<(HandleUntyped, K), BatchId>,
    frames_in_flight: usize,
    unbatched: Vec<(Entity, HandleUntyped, K)>,
    object_count: u32,

    object_buffers: Vec<Buffer>,
    capacity: u32,
    sets: Vec<DescriptorSet>,

    /// Set to true if any batch has been added or removed.
    /// Is not set if entities withing the batch are modified.
    dirty: bool,

    marker: PhantomData<Obj>,
}

impl<K: RendererKey, Obj: 'static> PassData<K, Obj> {
    pub fn new(
        context: Arc<VulkanContext>,
        capacity: u32,
        frames_in_flight: usize,
    ) -> Result<Self> {
        let object_buffers =
            Self::create_object_buffers(context.clone(), capacity, frames_in_flight)?;

        let sets = Self::create_sets(&context, &object_buffers)?;

        Ok(Self {
            context,
            capacity,
            batches: Vec::new(),
            ordered_batches: Vec::new(),
            sets,
            object_count: 0,
            object_buffers,
            batch_map: HashMap::new(),
            frames_in_flight,
            unbatched: Vec::new(),
            dirty: false,
            marker: PhantomData,
        })
    }

    fn create_object_buffers(
        context: Arc<VulkanContext>,
        capacity: u32,
        frames_in_flight: usize,
    ) -> Result<Vec<Buffer>> {
        (0..frames_in_flight)
            .map(|_| {
                Buffer::new_uninit::<Obj>(
                    context.clone(),
                    BufferUsage::STORAGE_BUFFER,
                    BufferAccess::Mapped,
                    capacity as u64,
                )
                .map_err(|e| e.into())
            })
            .collect::<Result<Vec<_>>>()
    }

    fn create_sets(
        context: &VulkanContext,
        object_buffers: &[Buffer],
    ) -> Result<Vec<DescriptorSet>> {
        object_buffers
            .iter()
            .map(|b| {
                DescriptorBuilder::new()
                    .bind_buffer(0, ShaderStageFlags::VERTEX, b)?
                    .build(context)
                    .map_err(|e| e.into())
            })
            .collect::<Result<Vec<_>>>()
    }

    /// Resizes internal data to accomodate at least `capacity` objects.
    fn resize(&mut self, capacity: u32) -> Result<()> {
        device::wait_idle(self.context.device())?;

        let capacity = nearest_power_2(capacity);

        self.capacity = capacity;

        self.object_buffers =
            Self::create_object_buffers(self.context.clone(), capacity, self.frames_in_flight)?;

        self.sets = Self::create_sets(&self.context, &self.object_buffers)?;

        Ok(())
    }

    /// Collects all entities that have yet to be placed into a batch in the current
    /// pass.
    pub fn get_unbatched<'a, Pass, Q, O>(&mut self, world: &'a mut World)
    where
        Pass: ShaderPass,
        Q: 'a + KeyQuery<K = K> + Query,
        <Q as Query>::Fetch: Fetch<'a, Item = Q>,
        O: Query,
    {
        let query = world
            .query_mut::<(&Handle<Pass>, Q, O)>()
            .without::<BatchMarker<Obj, Pass>>();

        self.unbatched.extend(
            query
                .into_iter()
                .map(|(e, (pass, keyq, _))| (e, pass.into_untyped(), keyq.into_key())),
        );
    }

    /// Builds rendering batches for shaderpass `T` for all objects not yet batched.
    /// Note: [`Self::get_unbatched`] needs to be run before to collect unbatched
    /// entities, this is due to lifetime limitations on world mutations.
    pub fn build_batches<'a, Pass, Q>(
        &mut self,
        world: &mut World,
        passes: &ResourceCache<Pass>,
    ) -> Result<()>
    where
        Pass: ShaderPass,
        Q: KeyQuery<K = K>,
        <Q as Query>::Fetch: Fetch<'a, Item = Q>,
    {
        let frames_in_flight = self.frames_in_flight;
        let object_count = &mut self.object_count;
        let batch_map = &mut self.batch_map;
        let ordered_batches = &mut self.ordered_batches;
        let batches = &mut self.batches;
        let dirty = &mut self.dirty;

        // Insert a marker to track this enemy as attached to a batch
        self.unbatched
            .drain(0..)
            .try_for_each(|(e, pass, key)| -> Result<_> {
                let marker = Self::insert_entity(
                    batches,
                    ordered_batches,
                    batch_map,
                    dirty,
                    object_count,
                    pass,
                    key,
                    passes,
                    frames_in_flight,
                )?;

                world.insert_one(e, marker)?;

                Ok(())
            })?;

        Ok(())
    }

    /// Updates the GPU side data of pass
    pub fn update<'a, Pass>(
        &mut self,
        current_frame: usize,
        iter: impl IntoIterator<Item = (Entity, (&'a BatchMarker<Obj, Pass>, impl Into<Obj>))>,
    ) -> Result<()>
    where
        Pass: ShaderPass,
    {
        // Update batch offsets
        let mut instance_count = 0;

        self.batches.iter_mut().for_each(|batch| {
            batch.first_instance = instance_count;
            batch.curr = 0;
            instance_count += batch.instance_count;
        });

        if self.object_count > self.capacity {
            self.resize(self.object_count)?;
        }

        let batches = &mut self.batches;

        self.object_buffers[current_frame].write_slice::<Obj, _, _>(
            self.object_count as _,
            0,
            move |data| {
                iter.into_iter().for_each(|(_, (marker, obj))| {
                    let batch = &mut batches[marker.batch_id];
                    data[(batch.first_instance + batch.curr) as usize] = obj.into();
                    batch.curr += 1;
                })
            },
        )?;

        self.batches.iter_mut().for_each(|batch| {
            batch.instance_count = batch.curr;
        });

        Ok(())
    }

    /// Inserts a new entity into the correct batch. Note: The entity should not already exist in pass,
    /// behaviour is undefined. Marks the batch as dirty.
    fn insert_entity<Pass: ShaderPass>(
        batches: &mut Vec<BatchData<K>>,
        ordered_batches: &mut Vec<BatchId>,
        batch_map: &mut HashMap<(HandleUntyped, K), usize>,
        dirty: &mut bool,
        object_count: &mut ObjectId,
        pass: HandleUntyped,
        key: K,
        passes: &ResourceCache<Pass>,
        frames_in_flight: usize,
    ) -> Result<BatchMarker<Obj, Pass>> {
        let frames_in_flight = frames_in_flight;

        let (batch_id, batch) = Self::get_batch_internal(
            batches,
            ordered_batches,
            batch_map,
            dirty,
            passes,
            pass,
            key,
        )?;

        batch.instance_count += 1;
        batch.set_dirty(frames_in_flight);
        *object_count += 1;

        Ok(BatchMarker {
            batch_id,
            marker: PhantomData,
        })
    }

    /// Returns or creates the appropriate batch for the combined shaderpass and
    /// key
    pub fn get_batch<Pass>(
        &mut self,
        passes: &ResourceCache<Pass>,
        pass: Handle<Pass>,
        key: K,
    ) -> Result<&mut BatchData<K>>
    where
        Pass: ShaderPass,
    {
        Self::get_batch_internal(
            &mut self.batches,
            &mut self.ordered_batches,
            &mut self.batch_map,
            &mut self.dirty,
            passes,
            pass.into_untyped(),
            key,
        )
        .map(|val| val.1)
    }
    ///
    fn get_batch_internal<'b, U, Pass>(
        batches: &'b mut Vec<BatchData<K>>,
        ordered_batches: &mut Vec<BatchId>,
        batch_map: &mut HashMap<(HandleUntyped, K), usize>,
        dirty: &mut bool,
        passes: U,
        pass: HandleUntyped,
        key: K,
    ) -> Result<(usize, &'b mut BatchData<K>)>
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
                let last = batches.len() - 1;
                batch_map.insert(combined_key, last);
                ordered_batches.push(last);
                *dirty = true;
                last
            }
        };

        Ok((idx, &mut batches[idx]))
    }

    /// Get a reference to the pass data's batches.
    pub fn batches(&self) -> &[BatchData<K>] {
        &self.batches
    }

    /// Get a reference to the pass data's object count.
    pub fn object_count(&self) -> ObjectId {
        self.object_count
    }

    /// Get a reference to the pass data's object buffers.
    pub fn object_buffers(&self) -> &[Buffer] {
        &self.object_buffers
    }

    pub fn object_buffer(&self, current_frame: usize) -> &Buffer {
        &self.object_buffers[current_frame]
    }

    /// Set to true if any batch has been added or removed.
    /// Is not set if entities withing the batch are modified.
    pub fn dirty(&self) -> bool {
        self.dirty
    }

    pub fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty;
    }
}

impl<K, Obj> PassData<K, Obj>
where
    K: Ord + RendererKey,
    Obj: 'static,
{
    /// Sorts batches if dirty and clears the dirty flag
    pub fn sort_batches_if_dirty(&mut self) {
        if self.dirty() {
            let batches = &self.batches;
            self.ordered_batches
                .sort_unstable_by_key(|val| &batches[*val].key);
            self.set_dirty(false)
        }
    }

    /// Sorts the batches by the key
    pub fn sort_batches(&mut self) {
        let batches = &self.batches;
        self.ordered_batches
            .sort_unstable_by_key(|val| &batches[*val].key);
    }

    /// Returns the batches in order from last sort.
    /// Note: [`Self::sort_batches`] or [`Self::sort_batches_if_dirty`] needs to be called
    /// to ensure proper order.
    pub fn ordered_batches(&self) -> OrderedBatchIterator<K> {
        OrderedBatchIterator {
            batches: &self.batches,
            ordered_batches: self.ordered_batches.iter(),
        }
    }
}

pub struct OrderedBatchIterator<'a, K> {
    batches: &'a Vec<BatchData<K>>,
    ordered_batches: std::slice::Iter<'a, BatchId>,
}

impl<'a, K> Iterator for OrderedBatchIterator<'a, K> {
    type Item = &'a BatchData<K>;

    fn next(&mut self) -> Option<Self::Item> {
        self.ordered_batches.next().map(|idx| &self.batches[*idx])
    }
}

impl<'a, K> DoubleEndedIterator for OrderedBatchIterator<'a, K> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.ordered_batches
            .next_back()
            .map(|idx| &self.batches[*idx])
    }
}

impl<K, Obj> IntoSet for PassData<K, Obj> {
    fn set(&self, current_frame: usize) -> DescriptorSet {
        self.sets[current_frame]
    }

    fn sets(&self) -> &[DescriptorSet] {
        &self.sets
    }
}

fn nearest_power_2(val: u32) -> u32 {
    let mut result = 1;
    while result < val {
        result *= 2;
    }
    result
}
