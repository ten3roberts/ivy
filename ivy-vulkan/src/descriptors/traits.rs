use std::slice;

use ash::vk::{DescriptorSet, ImageView, Sampler, ShaderStageFlags};

use super::DescriptorBuilder;

// Traits for types holding one or more descriptor sets for use in rendering.
pub trait IntoSet {
    /// Get the descriptor set for the current frame
    fn set(&self, current_frame: usize) -> DescriptorSet {
        *self
            .sets()
            .get(current_frame)
            .unwrap_or_else(|| &self.sets()[0])
    }
    // Retrieve descriptor sets for all frames. May be less than frames_in_flight if the same set is
    // to be used
    fn sets(&self) -> &[DescriptorSet];
}

impl IntoSet for DescriptorSet {
    fn set(&self, _: usize) -> DescriptorSet {
        *self
    }

    fn sets(&self) -> &[DescriptorSet] {
        slice::from_ref(self)
    }
}

impl IntoSet for Vec<DescriptorSet> {
    fn set(&self, current_frame: usize) -> DescriptorSet {
        self[current_frame]
    }

    fn sets(&self) -> &[DescriptorSet] {
        self
    }
}

pub trait DescriptorBindable {
    fn bind_descriptor_resource<'a>(
        &self,
        binding: u32,
        stage: ShaderStageFlags,
        builder: &'a mut DescriptorBuilder,
    ) -> &'a mut DescriptorBuilder;
}

impl DescriptorBindable for ImageView {
    fn bind_descriptor_resource<'a>(
        &self,
        binding: u32,
        stage: ShaderStageFlags,
        builder: &'a mut DescriptorBuilder,
    ) -> &'a mut DescriptorBuilder {
        builder.bind_image(binding, stage, *self)
    }
}

impl DescriptorBindable for Sampler {
    fn bind_descriptor_resource<'a>(
        &self,
        binding: u32,
        stage: ShaderStageFlags,
        builder: &'a mut DescriptorBuilder,
    ) -> &'a mut DescriptorBuilder {
        builder.bind_sampler(binding, stage, *self)
    }
}
