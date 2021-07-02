//! A device represents an abstraction for the vulkan GPU driver and is the primary way of
//! communicating with the GPU.

use super::{swapchain, Error};
use crate::Result;
use ash::{
    extensions::khr::Surface,
    vk::{self, SurfaceKHR},
};
use ash::{version::DeviceV1_0, version::InstanceV1_0};
use ash::{Device, Instance};
use std::{
    collections::HashSet,
    ffi::{CStr, CString},
    sync::Arc,
};

pub struct QueueFamilies {
    graphics: Option<u32>,
    present: Option<u32>,
    transfer: Option<u32>,
}

impl QueueFamilies {
    pub fn find(
        instance: &Instance,
        device: vk::PhysicalDevice,
        surface_loader: &Surface,
        surface: SurfaceKHR,
    ) -> Result<QueueFamilies> {
        let family_properties =
            unsafe { instance.get_physical_device_queue_family_properties(device) };
        let mut queue_families = QueueFamilies {
            graphics: None,
            present: None,
            transfer: None,
        };

        for (i, family) in family_properties.iter().enumerate() {
            if family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                queue_families.graphics = Some(i as u32);
            }

            if unsafe {
                surface_loader.get_physical_device_surface_support(device, i as u32, surface)?
            } {
                queue_families.present = Some(i as u32);
            }

            if family.queue_flags.contains(vk::QueueFlags::TRANSFER) {
                queue_families.transfer = Some(i as u32);
            }
        }

        Ok(queue_families)
    }

    pub fn graphics(&self) -> Option<u32> {
        self.graphics
    }

    pub fn present(&self) -> Option<u32> {
        self.present
    }

    pub fn transfer(&self) -> Option<u32> {
        self.transfer
    }

    pub fn has_graphics(&self) -> bool {
        self.graphics.is_some()
    }

    pub fn has_present(&self) -> bool {
        self.present.is_some()
    }

    pub fn has_transfer(&self) -> bool {
        self.transfer.is_some()
    }
}

type Score = usize;

const DEVICE_EXTENSIONS: &[&str] = &["VK_KHR_swapchain", "VK_KHR_shader_draw_parameters"];

/// Represents a physical device along with the queried properties, features, and queue families
pub struct PhysicalDeviceInfo {
    pub physical_device: vk::PhysicalDevice,
    pub name: String,
    pub score: Score,
    pub queue_families: QueueFamilies,
    pub limits: vk::PhysicalDeviceLimits,
    pub features: vk::PhysicalDeviceFeatures,
    pub properties: vk::PhysicalDeviceProperties,
}

// Rates physical device suitability
fn rate_physical_device(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
    surface_loader: &Surface,
    surface: SurfaceKHR,
    extensions: &[CString],
) -> Option<PhysicalDeviceInfo> {
    let properties = unsafe { instance.get_physical_device_properties(physical_device) };
    let features = unsafe { instance.get_physical_device_features(physical_device) };

    // Save the device name
    let name = unsafe {
        CStr::from_ptr(properties.device_name.as_ptr())
            .to_string_lossy()
            .to_string()
    };

    // Current device does not support one or more extensions
    if !get_missing_extensions(instance, physical_device, extensions)
        .ok()?
        .is_empty()
    {
        return None;
    }

    // Ensure swapchain capabilites
    let swapchain_support =
        swapchain::query_support(surface_loader, surface, physical_device).ok()?;

    // Swapchain support isn't adequate
    if swapchain_support.formats.is_empty() || swapchain_support.present_modes.is_empty() {
        return None;
    }

    let queue_families =
        QueueFamilies::find(instance, physical_device, surface_loader, surface).ok()?;

    // Graphics queue is required
    if !queue_families.has_graphics() {
        return None;
    }

    // Present queue is required
    if !queue_families.has_present() {
        return None;
    }

    // Device is valid

    let mut score: Score = 0;

    if properties.device_type == vk::PhysicalDeviceType::DISCRETE_GPU {
        score += 10000;
    }

    if features.sampler_anisotropy == vk::TRUE {
        score += 10000;
    }

    score += properties.limits.max_image_dimension2_d as Score;
    score += properties.limits.max_push_constants_size as Score;

    Some(PhysicalDeviceInfo {
        physical_device,
        name,
        score,
        features,
        properties,
        limits: properties.limits,
        queue_families,
    })
}

fn get_missing_extensions(
    instance: &Instance,
    device: vk::PhysicalDevice,
    extensions: &[CString],
) -> Result<Vec<CString>> {
    let available = unsafe { instance.enumerate_device_extension_properties(device)? };

    Ok(extensions
        .iter()
        .filter(|ext| {
            available
                .iter()
                .find(|avail| unsafe {
                    CStr::from_ptr(avail.extension_name.as_ptr()) == ext.as_c_str()
                })
                .is_none()
        })
        .cloned()
        .collect())
}

// Picks an appropriate physical device
fn pick_physical_device(
    instance: &Instance,
    surface_loader: &Surface,
    surface: SurfaceKHR,
    extensions: &[CString],
) -> Result<PhysicalDeviceInfo> {
    let devices = unsafe { instance.enumerate_physical_devices()? };

    devices
        .into_iter()
        .filter_map(|d| rate_physical_device(instance, d, surface_loader, surface, &extensions))
        .max_by_key(|v| v.score)
        .ok_or(Error::UnsuitableDevice)
}

/// Creates a logical device by choosing the best appropriate physical device
pub fn create(
    instance: &Instance,
    surface_loader: &Surface,
    surface: SurfaceKHR,
    layers: &[&str],
) -> Result<(Arc<Device>, PhysicalDeviceInfo)> {
    let extensions = DEVICE_EXTENSIONS
        .iter()
        .map(|s| CString::new(*s))
        .collect::<std::result::Result<Vec<_>, _>>()
        .unwrap();

    let pdevice_info = pick_physical_device(instance, surface_loader, surface, &extensions)?;

    let mut unique_queue_families = HashSet::new();
    unique_queue_families.insert(pdevice_info.queue_families.graphics().unwrap());
    unique_queue_families.insert(pdevice_info.queue_families.present().unwrap());

    let queue_create_infos: Vec<_> = unique_queue_families
        .iter()
        .map(|index| {
            vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(*index)
                .queue_priorities(&[1.0f32])
                .build()
        })
        .collect();

    // Get layers
    let layers = layers
        .iter()
        .map(|s| CString::new(*s))
        .collect::<std::result::Result<Vec<_>, _>>()
        .unwrap();

    let layer_names_raw = layers
        .iter()
        .map(|layer| layer.as_ptr() as *const i8)
        .collect::<Vec<_>>();

    let extension_names_raw = extensions
        .iter()
        .map(|ext| ext.as_ptr() as *const i8)
        .collect::<Vec<_>>();

    // TODO May not be present on all devices
    let enabled_features = vk::PhysicalDeviceFeatures {
        sampler_anisotropy: pdevice_info.features.sampler_anisotropy,
        multi_draw_indirect: vk::TRUE,
        ..Default::default()
    };

    let create_info = vk::DeviceCreateInfo::builder()
        .queue_create_infos(&queue_create_infos)
        .enabled_extension_names(&extension_names_raw)
        .enabled_layer_names(&layer_names_raw)
        .enabled_features(&enabled_features);

    let device =
        unsafe { instance.create_device(pdevice_info.physical_device, &create_info, None)? };
    Ok((Arc::new(device), pdevice_info))
}

pub fn get_limits(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
) -> vk::PhysicalDeviceLimits {
    let properties = unsafe { instance.get_physical_device_properties(physical_device) };

    properties.limits
}

pub fn wait_idle(device: &Device) -> Result<()> {
    unsafe { device.device_wait_idle()? }
    Ok(())
}

pub fn queue_wait_idle(device: &Device, queue: vk::Queue) -> Result<()> {
    unsafe { device.queue_wait_idle(queue)? }
    Ok(())
}

pub fn get_queue(device: &Device, family_index: u32, index: u32) -> vk::Queue {
    unsafe { device.get_device_queue(family_index, index) }
}

pub fn destroy(device: &Device) {
    unsafe { device.destroy_device(None) };
}
