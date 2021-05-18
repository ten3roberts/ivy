use std::{collections::HashMap, sync::Arc};

use arrayvec::ArrayVec;
use ash::version::DeviceV1_0;
use ash::vk;
use ash::Device;
use std::hash::{Hash, Hasher};

use crate::Error;

use super::DescriptorSetBinding;
use super::MAX_BINDINGS;

pub use vk::DescriptorSetLayout;

#[derive(Clone, Debug)]
pub struct DescriptorLayoutInfo {
    // The bindings for the layout
    bindings: ArrayVec<[DescriptorSetBinding; MAX_BINDINGS]>,
    sorted: bool,
}

impl DescriptorLayoutInfo {
    pub fn new(bindings: &[DescriptorSetBinding]) -> Self {
        Self {
            bindings: bindings.iter().copied().collect(),
            sorted: false,
        }
    }

    /// Return a reference to the descriptor layout info's bindings.
    pub fn bindings(&self) -> &[DescriptorSetBinding] {
        &self.bindings
    }

    pub fn add(&mut self, binding: DescriptorSetBinding) {
        self.bindings.push(binding);
    }

    /// Ensures the bindings are sorted
    fn ensure_sorted(&mut self) {
        if self.sorted {
            return;
        }
        self.bindings.sort_by_key(|binding| binding.binding);
        self.sorted = true;
    }
}

impl Default for DescriptorLayoutInfo {
    fn default() -> Self {
        Self {
            bindings: ArrayVec::new(),
            sorted: false,
        }
    }
}

impl Hash for DescriptorLayoutInfo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for binding in &self.bindings {
            binding.binding.hash(state);
        }
    }
}

impl PartialEq for DescriptorLayoutInfo {
    fn eq(&self, other: &Self) -> bool {
        for (a, b) in self.bindings.iter().zip(&other.bindings) {
            if a.binding != b.binding
                || a.descriptor_type != b.descriptor_type
                || b.descriptor_count != b.descriptor_count
                || a.stage_flags != b.stage_flags
            {
                return false;
            }
        }

        return true;
    }
}

impl Eq for DescriptorLayoutInfo {}

pub struct DescriptorLayoutCache {
    device: Arc<Device>,
    layouts: HashMap<DescriptorLayoutInfo, DescriptorSetLayout>,
}

impl DescriptorLayoutCache {
    pub fn new(device: Arc<Device>) -> Self {
        Self {
            device,
            layouts: HashMap::new(),
        }
    }

    /// Gets the descriptor set layout matching info. If layout does not already exist it is
    /// created. Takes info as mutable since it needs to be sorted.
    pub fn get(&mut self, info: &mut DescriptorLayoutInfo) -> Result<DescriptorSetLayout, Error> {
        info.ensure_sorted();

        if let Some(layout) = self.layouts.get(&info) {
            return Ok(*layout);
        } else {
            let info = info.clone();
            // Create layout
            let layout = create(&self.device, &info)?;
            Ok(*self.layouts.entry(info).or_insert(layout))
        }
    }

    /// Clears and destroys all cached layouts. This is often not needed as there's no limit to
    /// allocating descriptors from the same layout.
    pub fn clear(&mut self) {
        for (_, layout) in self.layouts.drain() {
            destroy(&self.device, layout);
        }
    }
}

impl Drop for DescriptorLayoutCache {
    fn drop(&mut self) {
        self.clear()
    }
}

pub fn create(device: &Device, info: &DescriptorLayoutInfo) -> Result<DescriptorSetLayout, Error> {
    let create_info = vk::DescriptorSetLayoutCreateInfo {
        binding_count: info.bindings.len() as u32,
        p_bindings: info.bindings.as_ptr(),
        ..Default::default()
    };

    let layout = unsafe { device.create_descriptor_set_layout(&&create_info, None)? };
    Ok(layout)
}

pub fn destroy(device: &Device, layout: DescriptorSetLayout) {
    unsafe { device.destroy_descriptor_set_layout(layout, None) }
}
