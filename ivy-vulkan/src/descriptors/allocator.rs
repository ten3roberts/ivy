use ash::version::DeviceV1_0;
use ash::vk;
use ash::Device;
use smallvec::SmallVec;
use std::{collections::HashMap, iter::repeat, sync::Arc};

use crate::Error;

pub use vk::DescriptorSetLayout;

use super::DescriptorLayoutInfo;

struct Pool {
    pool: vk::DescriptorPool,
    set_count: u32,
    allocated: u32,
}

impl Pool {
    /// Creates a new fresh pool
    fn new(
        device: &Device,
        set_count: u32,
        sizes: &[vk::DescriptorPoolSize],
    ) -> Result<Self, Error> {
        let create_info = vk::DescriptorPoolCreateInfo {
            max_sets: set_count,
            pool_size_count: sizes.len() as u32,
            p_pool_sizes: sizes.as_ptr(),
            ..Default::default()
        };

        let pool = unsafe { device.create_descriptor_pool(&create_info, None)? };
        Ok(Self {
            pool,
            set_count,
            allocated: 0,
        })
    }
}

/// Manages descriptor allocations by automatically managing pools for each layout
pub struct DescriptorAllocator {
    device: Arc<Device>,
    sub_allocators: HashMap<DescriptorSetLayout, DescriptorLayoutAllocator>,
    set_count: u32,
}

impl DescriptorAllocator {
    /// Creates a new descriptor allocator. `set_count` represents the preferred descriptor count
    /// per pool. It is possible to allocate more than `set_count` at a time.
    pub fn new(device: Arc<Device>, set_count: u32) -> Self {
        Self {
            device,
            sub_allocators: HashMap::new(),
            set_count,
        }
    }

    /// Allocates descriptors by using the layout cache based on layout_info
    pub fn allocate(
        &mut self,
        layout: vk::DescriptorSetLayout,
        layout_info: &DescriptorLayoutInfo,
        set_count: u32,
    ) -> Result<Vec<vk::DescriptorSet>, Error> {
        let sub_allocator =
            self.sub_allocators
                .entry(layout)
                .or_insert(DescriptorLayoutAllocator::new(
                    self.device.clone(),
                    layout,
                    layout_info,
                    self.set_count,
                ));

        sub_allocator.allocate(set_count)
    }

    /// Resets all allocated pools and descriptor sets.
    pub fn reset(&mut self) -> Result<(), Error> {
        self.sub_allocators
            .iter_mut()
            .map(|(_, sub_allocator)| sub_allocator.reset())
            .collect::<Result<(), _>>()?;

        Ok(())
    }

    // Clears and destroys all allocated pools.
    pub fn clear(&mut self) {
        self.sub_allocators.clear();
    }

    /// Returns the number of descriptor pools allocated for `layout`.
    pub fn pool_count(&self, layout: vk::DescriptorSetLayout) -> Option<usize> {
        self.sub_allocators
            .get(&layout)
            .map(DescriptorLayoutAllocator::total_pool_count)
    }

    /// Returns the number of completely full descriptor pools for `layout`.
    pub fn full_pool_count(&self, layout: vk::DescriptorSetLayout) -> Option<usize> {
        self.sub_allocators
            .get(&layout)
            .map(DescriptorLayoutAllocator::full_pool_count)
    }
}

/// Creates a new descriptor allocator. Stores several pools contains `set_count` available
/// descriptors each. `sizes` describes the relative count for each descriptor size. Allocates new
/// pools when no free are available

/// Manages allocation for a single descriptor set layout
struct DescriptorLayoutAllocator {
    device: Arc<Device>,
    layout: DescriptorSetLayout,
    set_count: u32,
    /// A list of pools with atleast 1 descriptor remaining.
    pools: Vec<Pool>,
    /// A list of completely full pools.
    full_pools: Vec<Pool>,
    sizes: Vec<vk::DescriptorPoolSize>,
}

impl DescriptorLayoutAllocator {
    /// Creates a new descriptor allocator. Stores several pools contains `set_count` available
    /// descriptors each. `sizes` describes the relative
    pub fn new(
        device: Arc<Device>,
        layout: DescriptorSetLayout,
        layout_info: &DescriptorLayoutInfo,
        set_count: u32,
    ) -> Self {
        let sizes = layout_info
            .bindings()
            .iter()
            .map(|binding| vk::DescriptorPoolSize {
                ty: binding.descriptor_type,
                descriptor_count: set_count,
            })
            .collect();

        Self {
            device,
            layout,
            set_count,
            pools: Vec::new(),
            full_pools: Vec::new(),
            sizes,
        }
    }

    /// Allocates a descriptor set for each element in `layouts`. Will allocate a new pool if no free pools
    /// are available. Correctly handles when descriptor set count is more than preferred `set_count`.
    pub fn allocate(&mut self, set_count: u32) -> Result<Vec<vk::DescriptorSet>, Error> {
        let layouts = repeat(self.layout)
            .take(set_count as usize)
            .collect::<SmallVec<[_; 8]>>();

        let mut alloc_info = vk::DescriptorSetAllocateInfo {
            descriptor_pool: vk::DescriptorPool::null(),
            descriptor_set_count: layouts.len() as u32,
            p_set_layouts: layouts.as_ptr(),
            ..Default::default()
        };

        // Find a free pool or allocate a new one
        let (pool_idx, pool) = match self
            .pools
            .iter_mut()
            .enumerate()
            .find(|(_, pool)| pool.allocated + set_count <= pool.set_count)
        {
            Some(pool) => pool,
            None => self.allocate_pool(set_count.max(self.set_count))?,
        };

        // No free pool found. Allocate a new pool. Override set count if requested descriptor
        // count is more.
        // let pool = self.allocate_pool(self.set_count.max(alloc_info.descriptor_set_count))?;
        alloc_info.descriptor_pool = pool.pool;
        pool.allocated += set_count;

        if pool.allocated == pool.set_count {
            let pool = self.pools.swap_remove(pool_idx);
            self.full_pools.push(pool);
        }

        let sets = unsafe { self.device.allocate_descriptor_sets(&alloc_info)? };

        Ok(sets)
    }

    /// Resets all allocated pools and descriptor sets.
    pub fn reset(&mut self) -> Result<(), Error> {
        // Move all full pools into pools
        self.pools.extend(self.full_pools.drain(..));

        for pool in self.pools.iter_mut().filter(|pool| pool.allocated != 0) {
            pool.allocated = 0;
            unsafe {
                self.device
                    .reset_descriptor_pool(pool.pool, Default::default())?
            }
        }

        Ok(())
    }

    // Clears and destroys all allocated pools.
    pub fn clear(&mut self) {
        for pool in self.pools.drain(..).chain(self.full_pools.drain(..)) {
            unsafe { self.device.destroy_descriptor_pool(pool.pool, None) }
        }
    }

    /// Allocates a new pool with `set_count` descriptors. Ignores `self.set_count`
    fn allocate_pool(&mut self, set_count: u32) -> Result<(usize, &mut Pool), Error> {
        let pool = Pool::new(&self.device, set_count, &self.sizes)?;
        self.pools.push(pool);
        let idx = self.pools.len() - 1;
        Ok((idx, &mut self.pools[idx]))
    }

    // Diagnostics
    /// Returns the total number of allocated pools
    pub fn total_pool_count(&self) -> usize {
        self.pools.len() + self.full_pools.len()
    }

    pub fn full_pool_count(&self) -> usize {
        self.full_pools.len()
    }
}

impl Drop for DescriptorLayoutAllocator {
    fn drop(&mut self) {
        self.clear();
    }
}
