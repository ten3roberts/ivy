use arrayvec::ArrayVec;
use std::io::{Read, Seek};

use crate::descriptors;
use ash::version::DeviceV1_0;
use ash::vk;
use ash::Device;
use descriptors::*;

use crate::Error;

pub const MAX_SETS: usize = 4;
pub const MAX_PUSH_CONSTANTS: usize = 4;

pub struct ShaderModule {
    pub reflect_module: spirv_reflect::ShaderModule,
    // pub stage: vk::ShaderStageFlags,
    pub module: vk::ShaderModule,
}

impl ShaderModule {
    pub fn new<R: Read + Seek>(device: &Device, code: &mut R) -> Result<Self, Error> {
        let code = ash::util::read_spv(code).map_err(|e| Error::Io(e, None))?;

        let create_info = vk::ShaderModuleCreateInfo::builder().code(&code);
        let module = unsafe { device.create_shader_module(&create_info, None)? };
        let reflect_module = spirv_reflect::create_shader_module(unsafe {
            std::slice::from_raw_parts(create_info.p_code as *const u8, create_info.code_size)
        })
        .map_err(|msg| Error::SpirvReflection(msg))?;

        Ok(Self {
            module,
            reflect_module,
        })
    }

    pub fn destroy(self, device: &Device) {
        unsafe { device.destroy_shader_module(self.module, None) };
    }
}

impl AsRef<spirv_reflect::ShaderModule> for ShaderModule {
    fn as_ref(&self) -> &spirv_reflect::ShaderModule {
        &self.reflect_module
    }
}

impl AsRef<vk::ShaderModule> for ShaderModule {
    fn as_ref(&self) -> &vk::ShaderModule {
        &self.module
    }
}

impl Into<vk::ShaderModule> for &ShaderModule {
    fn into(self) -> vk::ShaderModule {
        self.module
    }
}

/// Creates a pipeline layout from shader reflection.
pub fn reflect<S: AsRef<spirv_reflect::ShaderModule>>(
    device: &Device,
    modules: &[S],
    layout_cache: &mut DescriptorLayoutCache,
) -> Result<vk::PipelineLayout, Error> {
    let mut sets: [DescriptorLayoutInfo; MAX_SETS] = Default::default();

    let mut push_constant_ranges: ArrayVec<[vk::PushConstantRange; MAX_PUSH_CONSTANTS]> =
        ArrayVec::new();

    for module in modules {
        let module = module.as_ref();

        let stage_flags = vk::ShaderStageFlags::from_raw(module.get_shader_stage().bits());
        let bindings = module
            .enumerate_descriptor_bindings(None)
            .map_err(|msg| Error::SpirvReflection(msg))?;

        for binding in bindings {
            sets[binding.set as usize].add(descriptors::DescriptorSetBinding {
                binding: binding.binding,
                descriptor_type: map_descriptortype(binding.descriptor_type),
                descriptor_count: binding.count,
                stage_flags,
                p_immutable_samplers: std::ptr::null(),
            })
        }

        let push_constants = module
            .enumerate_push_constant_blocks(None)
            .map_err(|msg| Error::SpirvReflection(msg))?;

        for push_constant in push_constants {
            push_constant_ranges.push(vk::PushConstantRange {
                stage_flags,
                offset: push_constant.offset,
                size: push_constant.size,
            })
        }
    }

    let set_layouts = sets
        .iter_mut()
        .take_while(|set| set.bindings().len() > 0)
        .map(|set| layout_cache.get(set))
        .collect::<Result<ArrayVec<[_; MAX_SETS]>, _>>()?;

    let create_info = vk::PipelineLayoutCreateInfo {
        set_layout_count: set_layouts.len() as u32,
        p_set_layouts: set_layouts.as_ptr(),
        push_constant_range_count: push_constant_ranges.len() as u32,
        p_push_constant_ranges: push_constant_ranges.as_ptr(),
        ..Default::default()
    };

    let pipeline_layout = unsafe { device.create_pipeline_layout(&create_info, None)? };

    Ok(pipeline_layout)
}

// Maps descriptor type from spir-v reflect to ash::vk types
fn map_descriptortype(
    ty: spirv_reflect::types::descriptor::ReflectDescriptorType,
) -> vk::DescriptorType {
    match ty {
        spirv_reflect::types::ReflectDescriptorType::Undefined => unreachable!(),
        spirv_reflect::types::ReflectDescriptorType::Sampler => vk::DescriptorType::SAMPLER,
        spirv_reflect::types::ReflectDescriptorType::CombinedImageSampler => {
            vk::DescriptorType::COMBINED_IMAGE_SAMPLER
        }
        spirv_reflect::types::ReflectDescriptorType::SampledImage => {
            vk::DescriptorType::SAMPLED_IMAGE
        }
        spirv_reflect::types::ReflectDescriptorType::StorageImage => {
            vk::DescriptorType::STORAGE_IMAGE
        }
        spirv_reflect::types::ReflectDescriptorType::UniformTexelBuffer => {
            vk::DescriptorType::UNIFORM_TEXEL_BUFFER
        }
        spirv_reflect::types::ReflectDescriptorType::StorageTexelBuffer => {
            vk::DescriptorType::STORAGE_TEXEL_BUFFER
        }
        spirv_reflect::types::ReflectDescriptorType::UniformBuffer => {
            vk::DescriptorType::UNIFORM_BUFFER
        }
        spirv_reflect::types::ReflectDescriptorType::StorageBuffer => {
            vk::DescriptorType::STORAGE_BUFFER
        }
        spirv_reflect::types::ReflectDescriptorType::UniformBufferDynamic => {
            vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC
        }
        spirv_reflect::types::ReflectDescriptorType::StorageBufferDynamic => {
            vk::DescriptorType::STORAGE_BUFFER_DYNAMIC
        }
        spirv_reflect::types::ReflectDescriptorType::InputAttachment => {
            vk::DescriptorType::INPUT_ATTACHMENT
        }
        spirv_reflect::types::ReflectDescriptorType::AccelerationStructureNV => {
            vk::DescriptorType::ACCELERATION_STRUCTURE_NV
        }
    }
}
