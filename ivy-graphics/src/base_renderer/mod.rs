use std::collections::{btree_map::Entry, BTreeMap};

use crate::Result;

mod batch;
mod batches;
mod pass;
pub use batch::*;
pub use batches::*;
use flax::{component::ComponentKey, Component, Debuggable};
use ivy_vulkan::{context::SharedVulkanContext, Shader, VertexDesc};
pub use pass::*;

pub trait KeyQuery: Send + Sync {
    type K: RendererKey;
    fn into_key(&self) -> Self::K;
}

pub trait RendererKey: std::hash::Hash + std::cmp::Eq + Clone {}

impl<T> RendererKey for T where T: std::hash::Hash + std::cmp::Eq + Clone {}

type ObjectId = u32;

/// A renderer that can be reused for multiple passes
pub struct BaseRenderer<K, Obj, V> {
    context: SharedVulkanContext,
    passes: BTreeMap<ComponentKey, BaseRendererPass<K, Obj, V>>,
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
        Ok(Self {
            capacity,
            context,
            passes: Default::default(),
            frames_in_flight,
        })
    }

    /// Returns the pass data for the shader pass.
    pub fn pass_mut(
        &mut self,
        shaderpass: Component<Shader>,
    ) -> Result<&mut BaseRendererPass<K, Obj, V>> {
        match self.passes.entry(shaderpass.key()) {
            Entry::Occupied(entry) => Ok(entry.into_mut()),
            Entry::Vacant(entry) => Ok(entry.insert(BaseRendererPass::new(
                shaderpass,
                self.context.clone(),
                self.capacity,
                self.frames_in_flight,
            )?)),
        }
    }

    /// Returns the pass data for the shader pass.
    pub fn pass(&self, pass: Component<Shader>) -> &BaseRendererPass<K, Obj, V> {
        self.passes.get(&pass.key()).expect("Pass does not exist")
    }

    /// Get a reference to the base renderer's context.
    pub fn context(&self) -> &SharedVulkanContext {
        &self.context
    }
}

pub(crate) type BatchId = u32;

flax::component! {
    pub batch_id(pass_id): BatchId => [ Debuggable ],
}
