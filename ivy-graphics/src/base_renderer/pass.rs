use crate::{KeyQuery, RendererKey};
use std::marker::PhantomData;

use super::*;

use ash::vk::{DescriptorSet, ShaderStageFlags};
use hecs::{Entity, Fetch, Query, World};
use itertools::Itertools;
use ivy_resources::{Handle, HandleUntyped, Resources};
use ivy_vulkan::{
    context::SharedVulkanContext,
    descriptors::{DescriptorBuilder, IntoSet},
    device, Buffer, BufferAccess, BufferUsage, PassInfo, VertexDesc, VulkanContext,
};

/// Represents a single typed shaderpass. Each object belonging to the pass is
/// grouped into batches
pub struct PassData<K, Obj, V> {
    context: SharedVulkanContext,
    frames_in_flight: usize,
    unbatched: Vec<(Entity, HandleUntyped, K)>,
    object_count: u32,

    batches: Batches<K>,
    object_buffers: Vec<Buffer>,
    capacity: u32,
    sets: Vec<DescriptorSet>,

    marker: PhantomData<(Obj, V)>,
}

impl<V: VertexDesc, K: RendererKey, Obj: 'static> PassData<K, Obj, V> {
    pub fn new(
        context: SharedVulkanContext,
        capacity: u32,
        frames_in_flight: usize,
    ) -> Result<Self> {
        let object_buffers =
            Self::create_object_buffers(context.clone(), capacity, frames_in_flight)?;

        let sets = Self::create_sets(&context, &object_buffers)?;

        Ok(Self {
            batches: Batches::new(context.clone(), frames_in_flight),
            context,
            capacity,
            sets,
            object_count: 0,
            object_buffers,
            frames_in_flight,
            unbatched: Vec::new(),
            marker: PhantomData,
        })
    }

    fn create_object_buffers(
        context: SharedVulkanContext,
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
    pub fn register<'a, Pass, Q, O>(&mut self, world: &'a mut World)
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
        resources: &Resources,
        pass_info: &PassInfo,
    ) -> Result<()>
    where
        Pass: ShaderPass,
        Q: KeyQuery<K = K>,
        <Q as Query>::Fetch: Fetch<'a, Item = Q>,
    {
        let batches = &mut self.batches;
        let object_count = &mut self.object_count;
        // Insert a marker to track this enemy as attached to a batch
        self.unbatched
            .drain(0..)
            .try_for_each(|(e, pass, key)| -> Result<_> {
                let marker =
                    batches.insert_entity::<Obj, Pass, V>(resources, pass, key, pass_info)?;
                *object_count += 1;

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
        if self.object_count > self.capacity {
            self.resize(self.object_count)?;
        }

        // Update batch offsets
        let mut total_offset = 0;

        self.batches.iter_mut().for_each(|batch| {
            batch.first_instance = total_offset;
            batch.instance_count = 0;
            total_offset += batch.max_count;
        });

        let batches = &mut self.batches;
        self.object_buffers[current_frame].write_slice::<Obj, _, _>(
            self.object_count as _,
            0,
            move |data| {
                iter.into_iter().for_each(|(_, (marker, obj))| {
                    let batch = &mut batches[marker.batch_id as usize];

                    if batch.instance_count == batch.max_count {
                        eprintln!("Growing beyond");
                    }

                    assert!(batch.instance_count <= batch.max_count);
                    data[batch.first_instance as usize + batch.instance_count as usize] =
                        obj.into();

                    batch.instance_count += 1;
                })
            },
        )?;

        // println!(
        //     "Batches: {}",
        //     self.batches
        //         .iter()
        //         .format_with(", ", |v, f| f(&format_args!(
        //             "{}:{} off: {}",
        //             v.instance_count, v.max_count, v.first_instance
        //         )))
        // );

        Ok(())
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

    /// Get a reference to the pass data's batches.
    pub fn batches(&self) -> &Batches<K> {
        &self.batches
    }
}

impl<V, K, Obj> PassData<K, Obj, V>
where
    K: Ord + RendererKey,
    V: VertexDesc,
    Obj: 'static,
{
    /// Sorts batches if dirty and clears the dirty flag
    pub fn sort_batches_if_dirty(&mut self) {
        self.batches.sort_batches_if_dirty()
    }

    /// Sorts the batches by the key
    pub fn sort_batches(&mut self) {
        self.batches.sort_batches()
    }

    /// Returns the batches in order from last sort.
    /// Note: [`Self::sort_batches`] or [`Self::sort_batches_if_dirty`] needs to be called
    /// to ensure proper order.
    pub fn ordered_batches(&self) -> OrderedBatchIterator<K> {
        self.batches.ordered_batches()
    }
}

impl<V, K, Obj> IntoSet for PassData<V, K, Obj> {
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
