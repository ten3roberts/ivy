use crate::Result;
use std::{cmp::Ord, collections::HashMap, sync::Arc};

use arrayvec::ArrayVec;
use ash::vk;
use ash::Device;
use parking_lot::RwLock;
use std::hash::{Hash, Hasher};

use super::DescriptorSetBinding;
use super::MAX_BINDINGS;

pub use vk::DescriptorSetLayout;

#[derive(Clone, Debug)]
pub struct DescriptorLayoutInfo {
    // The bindings for the layout
    bindings: ArrayVec<DescriptorSetBinding, MAX_BINDINGS>,
}

// Impl send and sync because of the immutable sampler in bindings not implementing it
unsafe impl Send for DescriptorLayoutInfo {}
unsafe impl Sync for DescriptorLayoutInfo {}

impl DescriptorLayoutInfo {
    pub fn new(bindings: &[DescriptorSetBinding]) -> Self {
        let mut layout = Self {
            bindings: ArrayVec::new(),
        };

        for binding in bindings {
            layout.insert(*binding);
        }

        layout
    }

    /// Return a reference to the descriptor layout info's bindings.
    pub fn bindings(&self) -> &[DescriptorSetBinding] {
        &self.bindings
    }

    /// Insert a new binding.
    /// If binding index already exists, it will be replaced..
    pub fn insert(&mut self, binding: DescriptorSetBinding) {
        let mut len = self.bindings.len();
        let mut mid = (len / 2).min(1);

        loop {
            if len < 1 {
                if mid == self.bindings.len() || self.bindings[mid].binding > binding.binding {
                    self.bindings.insert(mid, binding);
                } else {
                    self.bindings.insert(mid + 1, binding);
                }
                break;
            }

            match self.bindings[mid].binding.cmp(&binding.binding) {
                std::cmp::Ordering::Less => {
                    len /= 2;
                    mid += (len as f32 / 2.0).floor() as usize;
                }
                std::cmp::Ordering::Equal => {
                    self.bindings[mid] = binding;
                    break;
                }
                std::cmp::Ordering::Greater => {
                    len /= 2;
                    mid -= (len as f32 / 2.0).ceil() as usize;
                }
            }
        }
    }

    pub fn extend<I: Iterator<Item = DescriptorSetBinding>>(&mut self, bindings: I) {
        self.bindings.extend(bindings);
    }

    /// Returns the layout binding at binding index.
    /// Uses a binary search.
    pub fn get(&mut self, binding: u32) -> Option<DescriptorSetBinding> {
        let mut len = self.bindings.len();
        let mut mid = (len / 2).max(1);

        loop {
            if mid >= self.bindings.len() || len == 0 {
                return None;
            }

            match self.bindings[mid].binding.cmp(&binding) {
                std::cmp::Ordering::Less => {
                    len /= 2;
                    mid += (len as f32 / 2.0).ceil() as usize;
                }
                std::cmp::Ordering::Equal => return Some(self.bindings[mid]),
                std::cmp::Ordering::Greater => {
                    len /= 2;
                    mid -= (len as f32 / 2.0).ceil() as usize;
                }
            }
        }
    }
}

impl Default for DescriptorLayoutInfo {
    fn default() -> Self {
        Self {
            bindings: ArrayVec::new(),
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
                || a.descriptor_count != b.descriptor_count
                || a.stage_flags != b.stage_flags
            {
                return false;
            }
        }

        true
    }
}

impl Eq for DescriptorLayoutInfo {}

pub struct DescriptorLayoutCache {
    device: Arc<Device>,
    layouts: RwLock<HashMap<DescriptorLayoutInfo, DescriptorSetLayout>>,
}

impl DescriptorLayoutCache {
    pub fn new(device: Arc<Device>) -> Self {
        Self {
            device,
            layouts: RwLock::new(HashMap::new()),
        }
    }

    /// Gets the descriptor set layout matching info. If layout does not already exist it is
    /// created.
    pub fn get(&self, info: &DescriptorLayoutInfo) -> Result<DescriptorSetLayout> {
        let guard = self.layouts.read();
        if let Some(layout) = guard.get(&info) {
            Ok(*layout)
        } else {
            drop(guard);

            let info = info.clone();
            // Create layout
            let layout = create(&self.device, &info)?;
            Ok(*self.layouts.write().entry(info).or_insert(layout))
        }
    }

    /// Clears and destroys all cached layouts. This is often not needed as there's no limit to
    /// allocating descriptors from the same layout.
    pub fn clear(&mut self) {
        for (_, layout) in self.layouts.write().drain() {
            destroy(&self.device, layout);
        }
    }
}

impl Drop for DescriptorLayoutCache {
    fn drop(&mut self) {
        self.clear()
    }
}

pub fn create(device: &Device, info: &DescriptorLayoutInfo) -> Result<DescriptorSetLayout> {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    /// Test the order is sorted after insertion
    fn layout_info_add() {
        let mut layout = DescriptorLayoutInfo::new(&[]);

        let mut bindings = [DescriptorSetBinding::default(); 6];

        bindings[0] = DescriptorSetBinding {
            binding: 0,
            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: 1,
            ..Default::default()
        };

        layout.insert(bindings[0]);

        eprintln!("Layout: {:?}", layout);

        bindings[2] = DescriptorSetBinding {
            binding: 2,
            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: 1,
            ..Default::default()
        };

        layout.insert(bindings[2]);

        eprintln!("Layout: {:?}", layout);

        bindings[1] = DescriptorSetBinding {
            binding: 1,
            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: 1,
            ..Default::default()
        };

        layout.insert(bindings[1]);

        eprintln!("Layout: {:?}", layout);

        bindings[3] = DescriptorSetBinding {
            binding: 3,
            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: 1,
            ..Default::default()
        };

        layout.insert(bindings[3]);

        eprintln!("Layout: {:?}", layout);

        bindings[5] = DescriptorSetBinding {
            binding: 5,
            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: 1,
            ..Default::default()
        };

        layout.insert(bindings[5]);

        eprintln!("Layout: {:?}", layout);

        bindings[4] = DescriptorSetBinding {
            binding: 4,
            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: 1,
            ..Default::default()
        };

        layout.insert(bindings[4]);

        layout.insert(DescriptorSetBinding {
            binding: 4,
            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 1,
            ..Default::default()
        });

        eprintln!("Layout: {:?}", layout);

        assert!(layout
            .bindings()
            .iter()
            .map(|val| val.binding)
            .eq([0, 1, 2, 3, 4, 5].iter().cloned()));

        for binding in &bindings {
            assert_eq!(
                Some(binding.binding),
                layout.get(binding.binding).map(|val| val.binding)
            )
        }

        assert_eq!(None, layout.get(9).map(|_| ()));
    }
}
