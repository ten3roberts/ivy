use crate::context::SharedVulkanContext;
use crate::descriptors::DescriptorBindable;
use crate::{Error, Result, Texture};
use ash::vk;
use ivy_resources::LoadResource;
#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};

#[derive(Hash, Eq, Default, Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
/// Wraps vk::SamplerAddressMode
pub struct AddressMode(i32);

impl AddressMode {
    pub const REPEAT: Self = Self(0);
    pub const MIRRORED_REPEAT: Self = Self(1);
    pub const CLAMP_TO_EDGE: Self = Self(2);
    pub const CLAMP_TO_BORDER: Self = Self(3);

    pub fn from_raw(val: i32) -> Self {
        Self(val)
    }
}

#[derive(Hash, Eq, Default, Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
/// Wraps vk::Filter
pub struct FilterMode(i32);

impl FilterMode {
    pub const NEAREST: Self = Self(0);
    pub const LINEAR: Self = Self(1);

    pub fn from_raw(val: i32) -> Self {
        Self(val)
    }
}

impl From<AddressMode> for vk::SamplerAddressMode {
    fn from(val: AddressMode) -> Self {
        Self::from_raw(val.0)
    }
}

impl From<FilterMode> for vk::Filter {
    fn from(val: FilterMode) -> Self {
        Self::from_raw(val.0)
    }
}
/// Specification dictating how a sampler is created
#[derive(Eq, Hash, PartialEq, Debug, Copy, Clone)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct SamplerInfo {
    pub address_mode: AddressMode,
    /// Filter mode used for undersampling when there are fewer texels than pixels,
    /// e.g; scaling up
    pub mag_filter: FilterMode,
    /// Filter mode used for oversampling when there are more texels than pixels,
    /// e.g; scaling down
    pub min_filter: FilterMode,
    /// Set to true to map from 0..size instead of 0..1
    pub unnormalized_coordinates: bool,
    /// From 1.0 to 16.0
    /// Anisotropy is automatically disabled if value is set to 1.
    /// Note: If using FilterMode::NEAREST, it is reccomended to disable anisotropy
    pub anisotropy: u32,
    /// Number of mipmapping levels to use
    pub mip_levels: u32,
}

impl SamplerInfo {
    /// Returns a sampler that does not interpolate, useful for pixel art
    pub fn pixelated() -> Self {
        Self {
            mag_filter: FilterMode::NEAREST,
            min_filter: FilterMode::NEAREST,
            anisotropy: 1,
            mip_levels: 1,
            ..Default::default()
        }
    }
}

impl Default for SamplerInfo {
    fn default() -> Self {
        Self {
            address_mode: AddressMode::REPEAT,
            mag_filter: FilterMode::LINEAR,
            min_filter: FilterMode::LINEAR,
            unnormalized_coordinates: false,
            anisotropy: 8,
            mip_levels: 4,
        }
    }
}

pub struct Sampler {
    context: SharedVulkanContext,
    sampler: vk::Sampler,
}

impl Sampler {
    // Creates a new sampler from the specified sampling options
    pub fn new(context: SharedVulkanContext, info: &SamplerInfo) -> Result<Self> {
        let anisotropy_enable = if info.anisotropy != 1 {
            vk::TRUE
        } else {
            vk::FALSE
        };

        let max_anisotropy = (info.anisotropy as f32).max(context.limits().max_sampler_anisotropy);

        let create_info = vk::SamplerCreateInfo {
            s_type: vk::StructureType::SAMPLER_CREATE_INFO,
            p_next: std::ptr::null(),
            flags: vk::SamplerCreateFlags::default(),
            mag_filter: info.mag_filter.into(),
            min_filter: info.min_filter.into(),
            mipmap_mode: vk::SamplerMipmapMode::LINEAR,
            min_lod: 0.0,
            max_lod: info.mip_levels as f32,
            address_mode_u: info.address_mode.into(),
            address_mode_v: info.address_mode.into(),
            address_mode_w: info.address_mode.into(),
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

impl DescriptorBindable for Sampler {
    fn bind_resource<'a>(
        &self,
        binding: u32,
        stage: vk::ShaderStageFlags,
        builder: &'a mut crate::descriptors::DescriptorBuilder,
    ) -> Result<&'a mut crate::descriptors::DescriptorBuilder> {
        Ok(builder.bind_sampler(binding, stage, self))
    }
}

impl DescriptorBindable for (Texture, Sampler) {
    fn bind_resource<'a>(
        &self,
        binding: u32,
        stage: vk::ShaderStageFlags,
        builder: &'a mut crate::descriptors::DescriptorBuilder,
    ) -> Result<&'a mut crate::descriptors::DescriptorBuilder> {
        Ok(builder.bind_combined_image_sampler(binding, stage, &self.0, &self.1))
    }
}

impl LoadResource for Sampler {
    type Info = SamplerInfo;

    type Error = Error;

    fn load(resources: &ivy_resources::Resources, info: &Self::Info) -> Result<Self> {
        let context = resources.get_default::<SharedVulkanContext>()?;
        Self::new(context.clone(), info)
    }
}
