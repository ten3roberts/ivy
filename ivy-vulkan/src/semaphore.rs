use super::Error;
use ash::version::DeviceV1_0;
use ash::vk;
use ash::Device;

pub fn create(device: &Device) -> Result<vk::Semaphore, Error> {
    let create_info = vk::SemaphoreCreateInfo {
        s_type: vk::StructureType::SEMAPHORE_CREATE_INFO,
        p_next: std::ptr::null(),
        flags: vk::SemaphoreCreateFlags::default(),
    };

    let semaphore = unsafe { device.create_semaphore(&create_info, None)? };
    Ok(semaphore)
}

pub fn destroy(device: &Device, semaphore: vk::Semaphore) {
    unsafe { device.destroy_semaphore(semaphore, None) }
}
