use std::{any::TypeId, collections::HashMap};

use crate::Result;

mod batch;
mod batches;
mod pass;
pub use batch::*;
pub use batches::*;
use hecs::Query;
use ivy_vulkan::{context::SharedVulkanContext, shaderpass::ShaderPass, VertexDesc};
pub use pass::*;

pub trait KeyQuery: Send + Sync + Query {
    type K: RendererKey;
    fn into_key(&self) -> Self::K;
}

pub trait RendererKey: std::hash::Hash + std::cmp::Eq + Copy {}

impl<T> RendererKey for T where T: std::hash::Hash + std::cmp::Eq + Copy {}

type ObjectId = u32;

/// A mesh renderer using vkCmdDrawIndirectIndexed and efficient batching.
/// A query and key are provided. On register, all entites satisfying the
/// `KeyQuery` will be placed into the object buffer. Objects will then be
/// placed into the correct batch according to their shaderpass and key hash.
/// This means that if the key is made of a Material and Mesh, all objects with
/// the same pipeline, material, and mesh will be placed in the same batch.
pub struct BaseRenderer<K, Obj, V> {
    context: SharedVulkanContext,
    passes: HashMap<TypeId, PassData<K, Obj, V>>,
    frames_in_flight: usize,
    capacity: u32,
}

impl<Obj, K: 'static, V: VertexDesc> BaseRenderer<K, Obj, V>
where
    K: RendererKey,
    Obj: 'static,
{
    pub fn new(
        context: SharedVulkanContext,
        capacity: ObjectId,
        frames_in_flight: usize,
    ) -> Result<Self> {
        let passes = HashMap::new();

        Ok(Self {
            capacity,
            context,
            passes,
            frames_in_flight,
        })
    }

    /// Returns the pass data for the shaderpass.
    pub fn pass_mut<Pass: ShaderPass>(&mut self) -> Result<&mut PassData<K, Obj, V>> {
        match self.passes.entry(TypeId::of::<Pass>()) {
            std::collections::hash_map::Entry::Occupied(entry) => Ok(entry.into_mut()),
            std::collections::hash_map::Entry::Vacant(entry) => Ok(entry.insert(PassData::new(
                self.context.clone(),
                self.capacity,
                self.frames_in_flight,
            )?)),
        }
    }

    /// Returns the pass data for the shaderpass.
    pub fn pass<Pass: ShaderPass>(&self) -> &PassData<K, Obj, V> {
        self.passes
            .get(&TypeId::of::<Pass>())
            .expect("Pass does not exist")
    }

    /// Get a reference to the base renderer's context.
    pub fn context(&self) -> &SharedVulkanContext {
        &self.context
    }
}

type BatchId = u32;
