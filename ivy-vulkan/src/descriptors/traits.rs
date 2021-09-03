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
    fn bind_resource<'a>(
        &self,
        binding: u32,
        stage: ShaderStageFlags,
        builder: &'a mut DescriptorBuilder,
    ) -> &'a mut DescriptorBuilder;
}

impl DescriptorBindable for ImageView {
    fn bind_resource<'a>(
        &self,
        binding: u32,
        stage: ShaderStageFlags,
        builder: &'a mut DescriptorBuilder,
    ) -> &'a mut DescriptorBuilder {
        builder.bind_image(binding, stage, *self)
    }
}

impl DescriptorBindable for Sampler {
    fn bind_resource<'a>(
        &self,
        binding: u32,
        stage: ShaderStageFlags,
        builder: &'a mut DescriptorBuilder,
    ) -> &'a mut DescriptorBuilder {
        builder.bind_sampler(binding, stage, *self)
    }
}

/// Helper trait for binding a list of resources for multiple frames in flight
pub trait MultiDescriptorBindable {
    fn bind_resource_for<'a>(
        &self,
        binding: u32,
        stage: ShaderStageFlags,
        builder: &'a mut DescriptorBuilder,
        current_frame: usize,
    ) -> &'a mut DescriptorBuilder;
}

impl<T> MultiDescriptorBindable for &[T]
where
    T: DescriptorBindable,
{
    fn bind_resource_for<'a>(
        &self,
        binding: u32,
        stage: ShaderStageFlags,
        builder: &'a mut DescriptorBuilder,
        current_frame: usize,
    ) -> &'a mut DescriptorBuilder {
        self[current_frame].bind_resource(binding, stage, builder)
    }
}

impl<T> MultiDescriptorBindable for T
where
    T: DescriptorBindable,
{
    fn bind_resource_for<'a>(
        &self,
        binding: u32,
        stage: ShaderStageFlags,
        builder: &'a mut DescriptorBuilder,
        _: usize,
    ) -> &'a mut DescriptorBuilder {
        self.bind_resource(binding, stage, builder)
    }
}
