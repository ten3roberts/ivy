use std::{
    collections::{hash_map::Entry, HashMap},
    marker::PhantomData,
    ops::{Index, IndexMut},
};

use flax::Component;
use ivy_resources::{Handle, HandleUntyped, Resources};
use ivy_vulkan::{
    context::SharedVulkanContext, PassInfo, Pipeline, PipelineInfo, Shader, VertexDesc,
};

use crate::{BatchData, BatchMarker, RendererKey, Result};

use super::BatchId;

pub struct Batches<K> {
    context: SharedVulkanContext,
    frames_in_flight: usize,
    batches: Vec<BatchData<K>>,
    /// Ordered access of batches
    ordered: Vec<BatchId>,
    // Map from key to index in batches
    batch_map: HashMap<(Handle<PipelineInfo>, K), BatchId>,
    pipeline_cache: HashMap<PipelineInfo, Pipeline>,
    /// Set to true if any batch has been added or removed.
    /// Is not set if entities withing the batch are modified.
    dirty: bool,
}

impl<K: RendererKey> Batches<K> {
    pub fn new(context: SharedVulkanContext, frames_in_flight: usize) -> Self {
        Self {
            context,
            frames_in_flight,
            batches: Vec::new(),
            ordered: Vec::new(),
            batch_map: HashMap::new(),
            pipeline_cache: HashMap::new(),
            dirty: false,
        }
    }

    pub fn get_batch<V: VertexDesc>(
        &mut self,
        resources: &Resources,
        pass: &Shader,
        key: K,
        pass_info: &PassInfo,
    ) -> Result<(BatchId, &mut BatchData<K>)> {
        let combined_key = (pass.pipeline_info, key);
        let idx = match self.batch_map.entry(combined_key) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => {
                // Create the batch

                let shaderpass = resources.get(pass.pipeline_info)?;
                let pipeline = self.pipeline_cache.entry(shaderpass.clone());

                let pipeline = match pipeline {
                    Entry::Occupied(entry) => entry.into_mut(),
                    Entry::Vacant(entry) => {
                        // Create pipeline
                        let pipeline =
                            Pipeline::new::<V>(self.context.clone(), &shaderpass, pass_info)?;

                        entry.insert(pipeline)
                    }
                };

                let batch = BatchData::new(
                    pipeline.pipeline(),
                    pipeline.layout(),
                    pass.pipeline_info,
                    key,
                );
                let idx = self.batches.len();
                self.batches.push(batch);
                self.dirty = true;
                self.ordered.push(idx as u32);
                *entry.insert(idx as u32)
            }
        };

        Ok((idx, &mut self.batches[idx as usize]))
    }

    pub fn get(&self, id: BatchId) -> Option<&BatchData<K>> {
        self.batches.get(id as usize)
    }

    pub fn get_mut(&mut self, id: BatchId) -> Option<&mut BatchData<K>> {
        self.batches.get_mut(id as usize)
    }

    pub fn insert_entity<O, V: VertexDesc>(
        &mut self,
        resources: &Resources,
        pass: &Shader,
        key: K,
        pass_info: &PassInfo,
    ) -> Result<BatchId> {
        let frames_in_flight = self.frames_in_flight;
        let (batch_id, batch) = self.get_batch::<V>(resources, pass, key, pass_info)?;
        batch.instance_count += 1;
        batch.max_count += 1;
        batch.set_dirty(frames_in_flight);

        Ok(batch_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &BatchData<K>> {
        self.batches.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut BatchData<K>> {
        self.batches.iter_mut()
    }

    /// Get a mutable reference to the batches's ordered batches.
    pub fn ordered_mut(&mut self) -> &mut Vec<BatchId> {
        &mut self.ordered
    }

    /// Get a reference to the batches's ordered batches.
    pub fn ordered(&self) -> &[BatchId] {
        self.ordered.as_ref()
    }

    /// Get a reference to the batches's batches.
    pub fn batches(&self) -> &[BatchData<K>] {
        self.batches.as_ref()
    }

    /// Get the batches's dirty.
    pub fn dirty(&self) -> bool {
        self.dirty
    }

    pub fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty
    }
}

impl<K> Index<usize> for Batches<K> {
    type Output = BatchData<K>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.batches[index]
    }
}

impl<K> IndexMut<usize> for Batches<K> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.batches[index]
    }
}

impl<K> Batches<K>
where
    K: Ord + RendererKey,
{
    /// Sorts batches if dirty and clears the dirty flag
    pub fn sort_batches_if_dirty(&mut self) {
        if self.dirty() {
            self.sort_batches()
        }
    }

    /// Sorts the batches by the key
    pub fn sort_batches(&mut self) {
        let batches = &self.batches;
        self.ordered
            .sort_unstable_by_key(|val| &batches[*val as usize].key);
        self.set_dirty(false)
    }

    /// Returns the batches in order from last sort.
    /// Note: [`Self::sort_batches`] or [`Self::sort_batches_if_dirty`] needs to be called
    /// to ensure proper order.
    pub fn ordered_batches(&self) -> OrderedBatchIterator<K> {
        OrderedBatchIterator {
            batches: self.batches(),
            ordered_batches: self.ordered().iter(),
        }
    }
}

pub struct OrderedBatchIterator<'a, K> {
    batches: &'a [BatchData<K>],
    ordered_batches: std::slice::Iter<'a, BatchId>,
}

impl<'a, K> Iterator for OrderedBatchIterator<'a, K> {
    type Item = &'a BatchData<K>;

    fn next(&mut self) -> Option<Self::Item> {
        self.ordered_batches
            .next()
            .map(|idx| &self.batches[*idx as usize])
    }
}

impl<'a, K> DoubleEndedIterator for OrderedBatchIterator<'a, K> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.ordered_batches
            .next_back()
            .map(|idx| &self.batches[*idx as usize])
    }
}
