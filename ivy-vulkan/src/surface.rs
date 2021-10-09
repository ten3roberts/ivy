use crate::{Extent, Result};
use ash::{extensions::khr::Surface, vk::SurfaceKHR, Entry, Instance};

pub fn create_loader(entry: &Entry, instance: &Instance) -> Surface {
    Surface::new(entry, instance)
}

pub trait Backend {
    fn create_surface(&self, instance: &Instance) -> Result<SurfaceKHR>;
    fn framebuffer_size(&self) -> Extent;
    fn extensions(&self) -> Vec<String>;
}

pub fn destroy(surface_loader: &Surface, surface: SurfaceKHR) {
    unsafe { surface_loader.destroy_surface(surface, None) };
}
