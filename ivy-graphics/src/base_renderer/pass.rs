use crate::{components, KeyQuery, RendererKey};
use std::marker::PhantomData;

use super::*;

use ash::vk::{DescriptorSet, ShaderStageFlags};
use flax::{entity_ids, Entity, Fetch, FetchItem, Query, World};
use ivy_vulkan::{
    context::SharedVulkanContext,
    descriptors::{DescriptorBuilder, IntoSet},
    device, Buffer, BufferAccess, BufferUsage, PassInfo, VertexDesc, VulkanContext,
};

/// A single shader pass in the renderer
///
/// Each object registered within the pass will subsequently be grouped into batches using the
/// generic key, such as material and mesh.
///
/// This allows even more efficient rendering using instancing
pub struct BaseRendererPass<K, Obj, V> {
    shaderpass: Component<Shader>,
    context: SharedVulkanContext,
    frames_in_flight: usize,
    unbatched: Vec<(Entity, Shader, K)>,
    object_count: u32,

    batches: Batches<K>,
    object_buffers: Vec<Buffer>,
    capacity: u32,
    sets: Vec<DescriptorSet>,

    marker: PhantomData<(Obj, V)>,
}

impl<V: VertexDesc, K: RendererKey, ObjectData: 'static> BaseRendererPass<K, ObjectData, V> {
    pub fn new(
        shaderpass: Component<Shader>,
        context: SharedVulkanContext,
        capacity: u32,
        frames_in_flight: usize,
    ) -> Result<Self> {
        let object_buffers =
            Self::create_object_buffers(context.clone(), capacity, frames_in_flight)?;

        let sets = Self::create_sets(&context, &object_buffers)?;

        Ok(Self {
            shaderpass,
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
                Buffer::new_uninit::<ObjectData>(
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

    /// Registers new entities for batching
    pub fn register<Q>(&mut self, world: &World, key: Q)
    where
        Q: for<'x> Fetch<'x>,
        for<'x> <Q as FetchItem<'x>>::Item: Into<K>,
    {
        self.unbatched.extend(
            Query::new((entity_ids(), self.shaderpass, key))
                .borrow(world)
                .iter()
                .map(|(e, pass, key)| (e, pass.clone(), key.into())),
        );
    }

    /// Builds batches for all unbatched objects
    /// Builds rendering batches for shaderpass `T` for all objects not yet batched.
    /// Note: [`Self::get_unbatched`] needs to be run before to collect unbatched
    /// entities, this is due to lifetime limitations on world mutations.
    pub fn build_batches(&mut self, world: &mut World, pass_info: &PassInfo) -> Result<()> {
        let batches = &mut self.batches;
        let object_count = &mut self.object_count;
        // Insert a marker to track this enemy as attached to a batch
        self.unbatched
            .drain(0..)
            .try_for_each(|(e, pass, key)| -> Result<_> {
                let batch_id = batches.insert_entity::<ObjectData, V>(&pass, key, pass_info)?;
                *object_count += 1;

                world
                    .set(e, super::batch_id(self.shaderpass.id()), batch_id)
                    .unwrap();

                Ok(())
            })?;

        Ok(())
    }

    /// Updates the GPU side data of pass
    pub fn update<'a>(
        &mut self,
        current_frame: usize,
        data: impl Iterator<Item = (Entity, BatchId, ObjectData)>,
        // iter: impl IntoIterator<Item = (Entity, (&'a BatchMarker<ObjectData, Pass>, impl Into<ObjectData>))>,
    ) -> Result<()> {
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
        self.object_buffers[current_frame].write_slice::<ObjectData, _, _>(
            self.object_count as _,
            0,
            move |dst| {
                data.into_iter().for_each(|(_, batch_id, obj)| {
                    let batch = &mut batches[batch_id as usize];

                    assert!(batch.instance_count <= batch.max_count);

                    dst[batch.first_instance as usize + batch.instance_count as usize] = obj.into();

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

impl<V, K, Obj> BaseRendererPass<K, Obj, V>
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

impl<V, K, Obj> IntoSet for BaseRendererPass<V, K, Obj> {
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
