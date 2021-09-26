use std::{any::TypeId, collections::HashMap, sync::Arc};

use crate::{Result, ShaderPass};

mod batch;
mod pass;
pub use batch::*;
use hecs::Query;
use ivy_vulkan::VulkanContext;
pub use pass::*;

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
    passes: HashMap<TypeId, PassData<K, Obj>>,
    frames_in_flight: usize,
    capacity: u32,
}

impl<Obj, K: 'static> BaseRenderer<K, Obj>
where
    K: Key,
    Obj: 'static,
{
    pub fn new(
        context: Arc<VulkanContext>,
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
    pub fn pass_mut<Pass: ShaderPass>(&mut self) -> Result<&mut PassData<K, Obj>> {
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
    pub fn pass<Pass: ShaderPass>(&self) -> &PassData<K, Obj> {
        self.passes
            .get(&TypeId::of::<Pass>())
            .expect("Pass does not exist")
    }
}

type BatchId = usize;
