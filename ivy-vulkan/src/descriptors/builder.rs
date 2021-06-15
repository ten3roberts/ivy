use crate::{Buffer, BufferType, Error, Sampler, Texture};
use arrayvec::ArrayVec;
use ash::version::DeviceV1_0;
use ash::vk::WriteDescriptorSet;
use ash::vk::{self, ImageLayout};
use ash::Device;

use super::{DescriptorAllocator, DescriptorLayoutCache, DescriptorSetBinding};
use super::{DescriptorLayoutInfo, MAX_BINDINGS};
use vk::{DescriptorType, ShaderStageFlags};

pub struct DescriptorBuilder {
    // `bindings` and `writes` are of the same size.
    bindings: ArrayVec<[DescriptorSetBinding; MAX_BINDINGS]>,
    writes: ArrayVec<[WriteDescriptorSet; MAX_BINDINGS]>,
    buffer_infos: [vk::DescriptorBufferInfo; MAX_BINDINGS],
    image_infos: [vk::DescriptorImageInfo; MAX_BINDINGS],
    // Holds a map to where in the writes array each binding is, or MAX_BINDINGS
    used_bindings: [usize; MAX_BINDINGS],
    // If nothing is changed, the last layout aquired from cache
    cached_layout: Option<(vk::DescriptorSetLayout, DescriptorLayoutInfo)>,
}

impl Default for DescriptorBuilder {
    fn default() -> Self {
        Self {
            bindings: Default::default(),
            writes: Default::default(),
            buffer_infos: Default::default(),
            image_infos: Default::default(),
            used_bindings: [MAX_BINDINGS; MAX_BINDINGS],
            cached_layout: None,
        }
    }
}

impl DescriptorBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    fn add(&mut self, binding: DescriptorSetBinding, write: WriteDescriptorSet) {
        // Cached layout is no longer valid
        self.cached_layout = None;

        // Binding has not already been specified
        let binding_idx = &mut self.used_bindings[binding.binding as usize];

        if *binding_idx == MAX_BINDINGS {
            // Point binding index to end of array
            *binding_idx = self.bindings.len();
            self.bindings.push(binding);
            self.writes.push(write);
        }
        // Overwrite binding
        else {
            self.bindings[*binding_idx] = binding;
            self.writes[*binding_idx] = write;
        }
    }

    pub fn bind_uniform_buffer(
        &mut self,
        binding: u32,
        stage: ShaderStageFlags,
        uniform_buffer: &Buffer,
    ) -> &mut Self {
        assert_eq!(uniform_buffer.ty(), BufferType::Uniform);
        self.buffer_infos[binding as usize] = vk::DescriptorBufferInfo {
            buffer: *uniform_buffer.as_ref(),
            offset: 0,
            range: vk::WHOLE_SIZE,
        };

        let write = WriteDescriptorSet {
            dst_binding: binding,
            dst_array_element: 0,
            descriptor_count: 1,
            descriptor_type: DescriptorType::UNIFORM_BUFFER,
            p_buffer_info: &self.buffer_infos[binding as usize],
            ..Default::default()
        };

        let binding = DescriptorSetBinding {
            binding,
            descriptor_type: DescriptorType::UNIFORM_BUFFER,
            descriptor_count: 1,
            stage_flags: stage,
            p_immutable_samplers: std::ptr::null(),
        };

        self.add(binding, write);

        self
    }

    pub fn bind_storage_buffer(
        &mut self,
        binding: u32,
        stage: ShaderStageFlags,
        storage_buffer: &Buffer,
    ) -> &mut Self {
        assert_eq!(storage_buffer.ty(), BufferType::Storage);

        self.buffer_infos[binding as usize] = vk::DescriptorBufferInfo {
            buffer: *storage_buffer.as_ref(),
            offset: 0,
            range: vk::WHOLE_SIZE,
        };

        let write = WriteDescriptorSet {
            dst_binding: binding,
            dst_array_element: 0,
            descriptor_count: 1,
            descriptor_type: DescriptorType::STORAGE_BUFFER,
            p_buffer_info: &self.buffer_infos[binding as usize],
            ..Default::default()
        };

        let binding = DescriptorSetBinding {
            binding,
            descriptor_type: DescriptorType::STORAGE_BUFFER,
            descriptor_count: 1,
            stage_flags: stage,
            p_immutable_samplers: std::ptr::null(),
        };

        self.add(binding, write);

        self
    }

    /// Binds a combined image sampler descriptor type.
    /// The texture is expected to be in in SHADER_READ_ONLY_OPTIMAL.
    pub fn bind_combined_image_sampler(
        &mut self,
        binding: u32,
        stage: ShaderStageFlags,
        texture: &Texture,
        sampler: &Sampler,
    ) -> &mut Self {
        self.image_infos[binding as usize] = vk::DescriptorImageInfo {
            sampler: sampler.into(),
            image_view: texture.into(),
            image_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        };

        let write = WriteDescriptorSet {
            dst_binding: binding,
            dst_array_element: 0,
            descriptor_count: 1,
            descriptor_type: DescriptorType::COMBINED_IMAGE_SAMPLER,
            p_image_info: &self.image_infos[binding as usize],
            ..Default::default()
        };

        let binding = DescriptorSetBinding {
            binding,
            descriptor_type: DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 1,
            stage_flags: stage,
            p_immutable_samplers: std::ptr::null(),
        };

        self.add(binding, write);

        self
    }

    /// Allocates and writes descriptor set into `set`. Can be chained.
    pub fn build(
        &mut self,
        device: &Device,
        cache: &mut DescriptorLayoutCache,
        allocator: &mut DescriptorAllocator,
        set: &mut vk::DescriptorSet,
    ) -> Result<&mut Self, Error> {
    *set = self.build_one(device, cache, allocator)?;
    Ok(self)
    }

    pub fn build_one(&mut self, device: &Device, cache: &mut DescriptorLayoutCache, allocator: &mut DescriptorAllocator) -> Result<vk::DescriptorSet, Error> {
        let mut layout = Default::default();

        self.layout(cache, &mut layout)?;

        let layout_info = &self.cached_layout.as_ref().unwrap().1;

        // Allocate the descriptor sets
        let set = allocator.allocate(layout, &layout_info, 1)?[0];

        self.writes
            .iter_mut()
            .for_each(|write| write.dst_set = set);

        unsafe { device.update_descriptor_sets(&self.writes, &[]) };

        Ok(set)
    }

    /// Returns the descriptor set layout by writing to `layout`. Uses the provided cache to fetch
    /// or create the appropriate layout.
    pub fn layout(
        &mut self,
        cache: &mut DescriptorLayoutCache,
        layout: &mut vk::DescriptorSetLayout,
    ) -> Result<&mut Self, Error> {
        // Create and store the layout if it isn't up to date
        if let Some(cached_layout) = self.cached_layout.as_ref() {
            *layout = cached_layout.0;
            Ok(self)
        } else {
            self.recache_layout(cache)?;
            self.layout(cache, layout)
        }
    }

    fn recache_layout(&mut self, cache: &mut DescriptorLayoutCache) -> Result<(), Error> {
        let mut info = DescriptorLayoutInfo::new(&self.bindings);
        let cached_layout = cache.get(&mut info)?;
        self.cached_layout = Some((cached_layout, info));
        Ok(())
    }
}
