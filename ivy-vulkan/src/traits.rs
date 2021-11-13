use crate::Result;
use ash::{
    vk::{self, SurfaceKHR},
    Instance,
};
use ivy_base::Extent;

pub trait IntoExtent {
    fn into_extent(&self) -> Extent;
}

pub trait FromExtent {
    fn from_extent(extent: Extent) -> Self;
}

impl FromExtent for vk::Extent2D {
    fn from_extent(val: Extent) -> Self {
        vk::Extent2D {
            width: val.width,
            height: val.height,
        }
    }
}

impl FromExtent for vk::Extent3D {
    fn from_extent(val: Extent) -> Self {
        vk::Extent3D {
            width: val.width,
            height: val.height,
            depth: 1,
        }
    }
}

impl FromExtent for vk::Offset2D {
    fn from_extent(val: Extent) -> Self {
        vk::Offset2D {
            x: val.width as i32,
            y: val.height as i32,
        }
    }
}

impl FromExtent for vk::Offset3D {
    fn from_extent(val: Extent) -> Self {
        vk::Offset3D {
            x: val.width as i32,
            y: val.height as i32,
            z: 1,
        }
    }
}

impl IntoExtent for vk::Extent2D {
    fn into_extent(&self) -> Extent {
        Extent {
            width: self.width,
            height: self.height,
        }
    }
}

/// Represents a backend for a vulkan context.
pub trait Backend {
    fn create_surface(&self, instance: &Instance) -> Result<SurfaceKHR>;
    fn framebuffer_size(&self) -> Extent;
    fn extensions(&self) -> Vec<String>;
}
