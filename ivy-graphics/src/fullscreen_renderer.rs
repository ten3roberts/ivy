use std::{
    any::TypeId,
    collections::{btree_map::Entry, BTreeMap},
};

use crate::{Error, Renderer, Result};
use ash::vk::DescriptorSet;
use ivy_resources::Resources;
use ivy_vulkan::{context::SharedVulkanContext, shaderpass::ShaderPass, PassInfo, Pipeline};

// Renders a fullscreen quad using the supplied shader pass and descriptors
pub struct FullscreenRenderer {
    pipeline: BTreeMap<TypeId, Pipeline>,
}

impl FullscreenRenderer {
    pub fn new() -> Self {
        Self {
            pipeline: BTreeMap::new(),
        }
    }
}

impl Renderer for FullscreenRenderer {
    type Error = Error;
    fn draw<Pass: ShaderPass>(
        &mut self,
        _world: &mut hecs::World,
        resources: &Resources,
        cmd: &ivy_vulkan::commands::CommandBuffer,
        sets: &[DescriptorSet],
        pass_info: &PassInfo,
        offsets: &[u32],
        _current_frame: usize,
    ) -> Result<()> {
        let pipeline = match self.pipeline.entry(TypeId::of::<Pass>()) {
            Entry::Vacant(entry) => {
                let context = resources.get_default::<SharedVulkanContext>()?;
                let pass = resources.get_default::<Pass>()?;
                let val = Pipeline::new::<()>(context.clone(), pass.pipeline(), pass_info)?;

                entry.insert(val)
            }
            Entry::Occupied(entry) => entry.into_mut(),
        };

        cmd.bind_pipeline(pipeline.pipeline());

        if !sets.is_empty() {
            cmd.bind_descriptor_sets(pipeline.layout(), 0, sets, offsets);
        }

        cmd.draw(3, 1, 0, 0);

        Ok(())
    }
}
