use std::marker::PhantomData;

use ash::vk::{Pipeline, PipelineLayout};
use ivy_assets::Asset;
use ivy_vulkan::PipelineInfo;

use super::{BatchId, ObjectId};

/// A batch contains objects of the same shaderpass and material.
pub struct BatchData<K> {
    pipeline: Pipeline,
    layout: PipelineLayout,
    pass: Asset<PipelineInfo>,
    pub(crate) key: K,
    /// Number of entities in batches before
    pub(crate) first_instance: u32,

    pub max_count: u32,
    /// The number of drawable objects in this batch
    pub instance_count: u32,

    /// Set to frames_in_flight when the batch is dirty.
    dirty: usize,
}

impl<O> BatchData<O> {
    pub fn new(
        pipeline: Pipeline,
        layout: PipelineLayout,
        pass: Asset<PipelineInfo>,
        key: O,
    ) -> Self {
        Self {
            pipeline,
            first_instance: 0,
            instance_count: 0,
            max_count: 0,
            layout,
            pass,
            key,
            dirty: 0,
        }
    }

    #[inline]
    pub fn pipeline(&self) -> Pipeline {
        self.pipeline
    }

    #[inline]
    pub fn layout(&self) -> PipelineLayout {
        self.layout
    }

    #[inline]
    pub fn key(&self) -> &O {
        &self.key
    }

    #[inline]
    pub fn shaderpass<T>(&self) -> &Asset<PipelineInfo> {
        &self.pass
    }

    #[inline]
    pub fn first_instance(&self) -> u32 {
        self.first_instance
    }

    #[inline]
    pub fn instance_count(&self) -> u32 {
        self.instance_count
    }

    /// Returns an iterator over the batch's object buffer ids
    #[inline]
    pub fn ids(&self) -> BatchIdIterator<O> {
        BatchIdIterator::new(self)
    }

    /// Set to true if the batch has been modified
    /// Is not set if entities withing the batch are modified.
    pub fn dirty(&self) -> usize {
        self.dirty
    }

    /// Removes one from dirty count.
    pub fn subtract_dirty(&mut self) {
        self.dirty -= 1;
    }

    /// Sets dirty count to `count`.
    pub fn set_dirty(&mut self, count: usize) {
        self.dirty = count;
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ObjectBufferMarker<K> {
    /// Index into the object buffer
    id: ObjectId,
    marker: PhantomData<K>,
}

/// Marker is send + sync
unsafe impl<K> Send for ObjectBufferMarker<K> {}
unsafe impl<K> Sync for ObjectBufferMarker<K> {}

/// Marks the entity as already being batched for this shaderpasss with the batch index and object buffer index.
pub struct BatchMarker<Obj, Pass> {
    pub(crate) batch_id: BatchId,
    pub(crate) marker: PhantomData<(Obj, Pass)>,
}

/// Marker is send + sync
unsafe impl<Obj, Pass> Sync for BatchMarker<Obj, Pass> {}
unsafe impl<Obj, Pass> Send for BatchMarker<Obj, Pass> {}

pub struct BatchIdIterator<Obj> {
    pub max: u32,
    pub curr: u32,
    pub marker: PhantomData<Obj>,
}

impl<Obj> BatchIdIterator<Obj> {
    pub fn new(batch: &BatchData<Obj>) -> Self {
        Self {
            curr: batch.first_instance,
            max: batch.instance_count + batch.first_instance,
            marker: PhantomData,
        }
    }
}

impl<Obj> Iterator for BatchIdIterator<Obj> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.curr >= self.max {
            None
        } else {
            let ret = self.curr;
            self.curr += 1;
            Some(ret)
        }
    }
}
