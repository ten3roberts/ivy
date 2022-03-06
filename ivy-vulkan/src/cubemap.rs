use ash::vk::{self, Extent3D, Image, SharingMode};
use gpu_allocator::{
    vulkan::{Allocation, AllocationCreateDesc},
    MemoryLocation,
};
use ivy_resources::{Handle, Resources};
use smallvec::SmallVec;

use crate::{context::SharedVulkanContext, traits::FromExtent, Texture, TextureInfo};

pub struct CubeMap {
    context: SharedVulkanContext,
    image: Image,
    views: SmallVec<[Handle<Texture>; 6]>,
    view: Handle<Texture>,
    allocation: Option<Allocation>,
}

impl Drop for CubeMap {
    fn drop(&mut self) {
        let allocator = self.context.allocator();

        let device = self.context.device();

        self.views.drain(..);

        // Destroy allocation if texture owns image
        if let Some(allocation) = self.allocation.take() {
            allocator.write().free(allocation).unwrap();
            unsafe { device.destroy_image(self.image, None) };
        }
    }
}
impl CubeMap {
    /// Creates a new unitialized texture
    /// Note, raw pixels must match format, width, and height
    pub fn new(
        context: SharedVulkanContext,
        resources: &Resources,
        info: &TextureInfo,
    ) -> crate::Result<Self> {
        let location = MemoryLocation::GpuOnly;

        let image_info = vk::ImageCreateInfo {
            image_type: vk::ImageType::TYPE_2D,
            format: info.format,
            extent: Extent3D::from_extent(info.extent),
            mip_levels: 1,
            array_layers: 1,
            samples: info.samples,
            tiling: vk::ImageTiling::OPTIMAL,
            usage: info.usage,
            sharing_mode: SharingMode::EXCLUSIVE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            ..Default::default()
        };

        let allocator = context.allocator();

        let device = context.device();
        let image = unsafe { device.create_image(&image_info, None)? };

        let requirements = unsafe { device.get_image_memory_requirements(image) };

        let allocation = allocator.write().allocate(&AllocationCreateDesc {
            name: "Image",
            requirements,
            location,
            linear: false,
        })?;

        unsafe { device.bind_image_memory(image, allocation.memory(), allocation.offset())? };
        let view = resources.insert(Texture::from_image(
            context.clone(),
            info,
            image,
            None,
            6,
            0,
        )?)?;

        let views = (0..6)
            .map(|i| -> crate::Result<_> {
                resources
                    .insert(Texture::from_image(
                        context.clone(),
                        info,
                        image,
                        None,
                        6,
                        i,
                    )?)
                    .map_err(|v| v.into())
            })
            .collect::<Result<_, _>>()?;

        Ok(Self {
            image,
            views,
            context,
            allocation: Some(allocation),
            view,
        })
    }

    /// Get a reference to the cube map's views.
    pub fn views(&self) -> &SmallVec<[Handle<Texture>; 6]> {
        &self.views
    }

    /// Get the cube map's view.
    pub fn view(&self) -> Handle<Texture> {
        self.view
    }
}
