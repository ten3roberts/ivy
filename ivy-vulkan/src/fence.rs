//! Fences provide CPU to GPU synchronization where the CPU can wait for a fence to be signaled.
//! Fences are useful when ensuring or checking completion of submitted commandbuffers.
use crate::Result;
use ash::vk;
use ash::Device;
pub use vk::Fence;

pub fn create(device: &Device, signaled: bool) -> Result<Fence> {
    let create_info = vk::FenceCreateInfo {
        s_type: vk::StructureType::FENCE_CREATE_INFO,
        p_next: std::ptr::null(),
        flags: if signaled {
            vk::FenceCreateFlags::SIGNALED
        } else {
            vk::FenceCreateFlags::default()
        },
    };

    let fence = unsafe { device.create_fence(&create_info, None)? };
    Ok(fence)
}

pub fn wait(device: &Device, fences: &[Fence], wait_all: bool) -> Result<()> {
    unsafe { device.wait_for_fences(fences, wait_all, std::u64::MAX)? }
    Ok(())
}

pub fn reset(device: &Device, fences: &[Fence]) -> Result<()> {
    unsafe { device.reset_fences(fences)? }
    Ok(())
}

pub fn destroy(device: &Device, fence: Fence) {
    unsafe { device.destroy_fence(fence, None) }
}
