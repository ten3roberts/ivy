use crate::{Result, VulkanContext};
use std::sync::Arc;

use ash::version::DeviceV1_0;
use ash::vk;

// Re-export enums
pub use vk::Filter as FilterMode;
pub use vk::SamplerAddressMode as AddressMode;

/// Specification dictating how a sampler is created
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct SamplerInfo {
    pub address_mode: vk::SamplerAddressMode,
    /// Filter mode used for undersampling when there are fewer texels than pixels,
    /// e.g; scaling up
    pub mag_filter: vk::Filter,
    /// Filter mode used for oversampling when there are more texels than pixels,
    /// e.g; scaling down
    pub min_filter: vk::Filter,
    /// Set to true to map from 0..size instead of 0..1
    pub unnormalized_coordinates: bool,
    /// From 1.0 to 16.0
    /// Anisotropy is automatically disabled if value is set to 1.0
    pub anisotropy: f32,
    /// Number of mipmapping levels to use
    pub mip_levels: u32,
}

pub struct Sampler {
    context: Arc<VulkanContext>,
    sampler: vk::Sampler,
}

impl Sampler {
    // Creates a new sampler from the specified sampling options
    pub fn new(context: Arc<VulkanContext>, info: SamplerInfo) -> Result<Self> {
        let max_anisotropy = info.anisotropy.max(context.limits().max_sampler_anisotropy);
        let anisotropy_enable = if max_anisotropy > 1.0 {
            vk::TRUE
        } else {
            vk::FALSE
        };

        let create_info = vk::SamplerCreateInfo {
            s_type: vk::StructureType::SAMPLER_CREATE_INFO,
            p_next: std::ptr::null(),
            flags: vk::SamplerCreateFlags::default(),
            mag_filter: info.mag_filter,
            min_filter: info.min_filter,
            mipmap_mode: vk::SamplerMipmapMode::LINEAR,
            min_lod: 0.0,
            max_lod: info.mip_levels as f32,
            address_mode_u: info.address_mode,
            address_mode_v: info.address_mode,
            address_mode_w: info.address_mode,
            mip_lod_bias: 0.0,
            anisotropy_enable,
            max_anisotropy,
            compare_enable: vk::FALSE,
            compare_op: vk::CompareOp::ALWAYS,
            border_color: vk::BorderColor::INT_OPAQUE_BLACK,
            unnormalized_coordinates: info.unnormalized_coordinates as u32,
        };

        let sampler = unsafe { context.device().create_sampler(&create_info, None)? };
        Ok(Self { context, sampler })
    }

    pub fn sampler(&self) -> vk::Sampler {
        self.sampler
    }
}

impl AsRef<vk::Sampler> for Sampler {
    fn as_ref(&self) -> &vk::Sampler {
        &self.sampler
    }
}

impl From<&Sampler> for vk::Sampler {
    fn from(val: &Sampler) -> Self {
        val.sampler
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        unsafe {
            self.context.device().destroy_sampler(self.sampler, None);
        }
    }
}
