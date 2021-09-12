use crate::Result;
use ash::{
    extensions::khr::Surface,
    vk::{self, Handle, SurfaceKHR},
    Entry, Instance,
};

use glfw::Window;

pub fn create_loader(entry: &Entry, instance: &Instance) -> Surface {
    Surface::new(entry, instance)
}

/// Creates a vulkan surface from window
pub fn create(instance: &Instance, window: &Window) -> Result<SurfaceKHR> {
    let mut surface: u64 = 0_u64;
    let result = window.create_window_surface(
        instance.handle().as_raw() as _,
        std::ptr::null(),
        &mut surface,
    );

    if result != vk::Result::SUCCESS.as_raw() as u32 {
        return Err(vk::Result::from_raw(result as i32).into());
    }

    Ok(SurfaceKHR::from_raw(surface))
}

pub fn destroy(surface_loader: &Surface, surface: SurfaceKHR) {
    unsafe { surface_loader.destroy_surface(surface, None) };
}
