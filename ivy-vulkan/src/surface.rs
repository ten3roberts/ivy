use ash::{extensions::khr::Surface, vk::SurfaceKHR, Entry, Instance};

pub fn create_loader(entry: &Entry, instance: &Instance) -> Surface {
    Surface::new(entry, instance)
}

pub fn destroy(surface_loader: &Surface, surface: SurfaceKHR) {
    unsafe { surface_loader.destroy_surface(surface, None) };
}
