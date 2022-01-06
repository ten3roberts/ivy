use crate::descriptors::{DescriptorAllocator, DescriptorLayoutCache};
use crate::traits::Backend;
use crate::{commands::CommandPool, device::QueueFamilies, Result, *};
use ash::extensions::ext::DebugUtils;
use ash::extensions::khr::Surface;
use ash::vk;

use gpu_allocator::vulkan::{Allocator, AllocatorCreateDesc};
use parking_lot::RwLock;
use std::sync::Arc;

pub type SharedVulkanContext = Arc<VulkanContext>;

pub struct VulkanContext {
    _entry: ash::Entry,
    instance: ash::Instance,
    device: Arc<ash::Device>,
    physical_device: vk::PhysicalDevice,
    queue_families: QueueFamilies,
    debug_utils: Option<(DebugUtils, vk::DebugUtilsMessengerEXT)>,

    descriptor_layout_cache: DescriptorLayoutCache,
    descriptor_allocator: DescriptorAllocator,
    surface_loader: Surface,
    surface: Option<vk::SurfaceKHR>,

    swapchain_loader: ash::extensions::khr::Swapchain,

    graphics_queue: vk::Queue,
    present_queue: vk::Queue,
    allocator: Option<RwLock<Allocator>>,

    /// CommandPool for allocatig transfer command buffers
    /// Wrap in option to drop early
    transfer_pool: Option<CommandPool>,

    limits: vk::PhysicalDeviceLimits,
    msaa_samples: vk::SampleCountFlags,
}

impl VulkanContext {
    pub fn new<T>(backend: &T) -> Result<Self>
    where
        T: Backend,
    {
        let entry = entry::create()?;
        let instance = instance::create(
            &entry,
            &backend.extensions(),
            "Vulkan Application",
            "Custom",
        )?;

        // Create debug utils if validation layers are enabled
        let debug_utils = if instance::ENABLE_VALIDATION_LAYERS {
            Some(debug_utils::create(&entry, &instance)?)
        } else {
            None
        };

        // debug_utils::create(&entry, &instance)?;
        let surface_loader = surface::create_loader(&entry, &instance);

        let surface = backend.create_surface(&instance)?;

        let (device, pdevice_info) = device::create(
            &instance,
            Some((&surface_loader, surface)),
            instance::get_layers(),
        )?;

        let swapchain_loader = Swapchain::create_loader(&instance, &device);

        // Get the physical device limits
        let limits = device::get_limits(&instance, pdevice_info.physical_device);

        let graphics_queue =
            device::get_queue(&device, pdevice_info.queue_families.graphics().unwrap(), 0);
        let present_queue =
            device::get_queue(&device, pdevice_info.queue_families.present().unwrap(), 0);

        let allocator_info = AllocatorCreateDesc {
            physical_device: pdevice_info.physical_device,
            device: (*device).clone(),
            instance: instance.clone(),
            debug_settings: Default::default(),
            buffer_device_address: false,
        };

        let allocator = Some(RwLock::new(Allocator::new(&allocator_info)?));

        let transfer_pool = CommandPool::new(
            device.clone(),
            pdevice_info.queue_families.graphics().unwrap(),
            true,
            true,
        )?;

        let msaa_samples = get_max_msaa_samples(
            limits.framebuffer_color_sample_counts & limits.sampled_image_color_sample_counts,
        );

        let descriptor_layout_cache = DescriptorLayoutCache::new(device.clone());
        let descriptor_allocator = DescriptorAllocator::new(device.clone(), 64);

        Ok(VulkanContext {
            _entry: entry,
            instance,
            device,
            physical_device: pdevice_info.physical_device,
            queue_families: pdevice_info.queue_families,
            debug_utils,
            descriptor_layout_cache,
            descriptor_allocator,
            surface_loader,
            surface: Some(surface),
            swapchain_loader,
            graphics_queue,
            present_queue,
            allocator,
            transfer_pool: Some(transfer_pool),
            limits,
            msaa_samples,
        })
    }

    // Returns the device
    #[inline]
    pub fn device(&self) -> &Arc<ash::Device> {
        &self.device
    }

    pub fn physical_device(&self) -> vk::PhysicalDevice {
        self.physical_device
    }

    pub fn queue_families(&self) -> &QueueFamilies {
        &self.queue_families
    }

    pub fn present_queue(&self) -> vk::Queue {
        self.present_queue
    }

    pub fn graphics_queue(&self) -> vk::Queue {
        self.graphics_queue
    }

    pub fn surface(&self) -> Option<vk::SurfaceKHR> {
        self.surface
    }

    pub fn surface_loader(&self) -> &Surface {
        &self.surface_loader
    }

    pub fn instance(&self) -> &ash::Instance {
        &self.instance
    }

    #[inline]
    pub fn allocator(&self) -> &RwLock<Allocator> {
        self.allocator.as_ref().unwrap()
    }

    pub fn limits(&self) -> &vk::PhysicalDeviceLimits {
        &self.limits
    }

    /// Returns a commandpool that can be used to allocate for transfer
    /// operations
    pub fn transfer_pool(&self) -> &CommandPool {
        self.transfer_pool
            .as_ref()
            .expect("Transfer pool is only None when dropped")
    }

    /// Returns the maximum number of samples for framebuffer color attachments
    pub fn msaa_samples(&self) -> vk::SampleCountFlags {
        self.msaa_samples
    }

    /// Get a reference to the vulkan context's swapchain loader.
    pub fn swapchain_loader(&self) -> &ash::extensions::khr::Swapchain {
        &self.swapchain_loader
    }

    /// Get a reference to the vulkan context's descriptor layout cache.
    #[inline]
    pub fn layout_cache(&self) -> &DescriptorLayoutCache {
        &self.descriptor_layout_cache
    }

    /// Get a reference to the vulkan context's descriptor allocator.
    #[inline]
    pub fn descriptor_allocator(&self) -> &DescriptorAllocator {
        &self.descriptor_allocator
    }

    pub fn wait_idle(&self) -> Result<()> {
        unsafe { self.device().device_wait_idle().map_err(|e| e.into()) }
    }
}

impl Drop for VulkanContext {
    fn drop(&mut self) {
        device::wait_idle(&self.device).unwrap();
        self.descriptor_allocator.clear();
        self.descriptor_layout_cache.clear();

        // Destroy the transfer pool before device destruction
        self.transfer_pool.take();

        self.allocator.take();

        // Destroy the device
        device::destroy(&self.device);

        // Destroy debug utils if present
        if let Some((debug_utils, debug_messenger)) = self.debug_utils.take() {
            debug_utils::destroy(&debug_utils, debug_messenger)
        }

        if let Some(surface) = self.surface {
            surface::destroy(&self.surface_loader, surface);
        }

        instance::destroy(&self.instance);
    }
}

fn get_max_msaa_samples(sample_counts: vk::SampleCountFlags) -> vk::SampleCountFlags {
    if sample_counts.contains(vk::SampleCountFlags::TYPE_64) {
        vk::SampleCountFlags::TYPE_64
    } else if sample_counts.contains(vk::SampleCountFlags::TYPE_32) {
        vk::SampleCountFlags::TYPE_32
    } else if sample_counts.contains(vk::SampleCountFlags::TYPE_16) {
        vk::SampleCountFlags::TYPE_16
    } else if sample_counts.contains(vk::SampleCountFlags::TYPE_8) {
        vk::SampleCountFlags::TYPE_8
    } else if sample_counts.contains(vk::SampleCountFlags::TYPE_4) {
        vk::SampleCountFlags::TYPE_4
    } else if sample_counts.contains(vk::SampleCountFlags::TYPE_2) {
        vk::SampleCountFlags::TYPE_2
    } else {
        vk::SampleCountFlags::TYPE_1
    }
}
