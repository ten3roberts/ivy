use derive_more::{Deref, From, Into};
use std::{mem::size_of, sync::Arc};

use crate::Error;
use hecs::{Entity, World};
use ivy_vulkan::{Buffer, BufferAccess, BufferType, VulkanContext};

use crate::camera::{Camera, CameraData};

/// Manages the GPU side data for cameras.
/// *Note*: There can only exist on `CameraManager`
pub struct CameraManager {
    camera_buffers: Vec<Buffer>,
    max_camera_index: u32,
    // The offset for each camera satisfying alignment requirements.
    dynamic_offset: u32,
    max_capacity: u32,
}

impl CameraManager {
    /// Creates a new camera manager.
    pub fn new(
        context: Arc<VulkanContext>,
        max_capacity: u32,
        frames_in_flight: usize,
    ) -> Result<Self, Error> {
        let dynamic_offset = (size_of::<CameraData>() as u32)
            .max(context.limits().min_uniform_buffer_offset_alignment as u32);

        let camera_buffers = (0..frames_in_flight)
            .map(|_| {
                Buffer::new_uninit(
                    context.clone(),
                    BufferType::UniformDynamic,
                    BufferAccess::Mapped,
                    max_capacity as u64 * dynamic_offset as u64,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            camera_buffers,
            max_camera_index: 0,
            dynamic_offset,
            max_capacity,
        })
    }

    /// Returns the dynamic offset per camera.
    pub fn dynamic_offset(&self) -> u32 {
        self.dynamic_offset
    }

    /// Returns the dynamic offset of camera.
    pub fn offset_of(&self, world: &World, camera: Entity) -> Result<u32, Error> {
        Ok(**world.get::<CameraIndex>(camera)? * self.dynamic_offset)
    }

    /// Registers a camera entity to the GPU side buffer.
    pub fn register(
        &mut self,
        world: &mut World,
        camera: Entity,
        // descriptor_layout_cache: &mut DescriptorLayoutCache,
    ) -> Result<CameraIndex, Error> {
        if self.max_camera_index >= self.max_capacity {
            return Err(Error::CameraLimit(self.max_capacity));
        }

        let index = self.max_camera_index.into();

        self.max_camera_index += 1;
        world.insert_one(camera, index)?;

        Ok(index)
    }

    /// Registers all unregistered camera entities.
    pub fn register_cameras(
        &mut self,
        world: &mut World,
        // descriptor_layout_cache: &mut DescriptorLayoutCache,
    ) -> Result<(), Error> {
        let ids = world
            .query_mut::<&Camera>()
            .without::<CameraIndex>()
            .into_iter()
            .map(|(e, _)| e)
            .into_iter()
            .collect::<Vec<_>>();

        ids.into_iter()
            .try_for_each(|e| self.register(world, e).map(|_| ()))?;

        Ok(())
    }

    /// Get a reference to the camera manager's camera buffers.
    pub fn buffers(&self) -> &[Buffer] {
        self.camera_buffers.as_slice()
    }

    /// Get a reference to camera manager's camera buffer by index, on for each frame in flight.
    pub fn buffer(&self, index: usize) -> &Buffer {
        &self.camera_buffers[index]
    }

    /// Update GPU side data for all registered cameras.
    pub fn update_camera_data(&mut self, world: &World, current_frame: usize) -> Result<(), Error> {
        let dynamic_offset = self.dynamic_offset;

        self.camera_buffers[current_frame].write(
            self.max_camera_index as u64,
            0,
            |ptr: *mut u8| {
                world.query::<(&Camera, &CameraIndex)>().iter().for_each(
                    |(_e, (camera, idx))| unsafe {
                        let data =
                            ptr.offset(idx.0 as isize * dynamic_offset as isize) as *mut CameraData;
                        *data = CameraData::new(camera.viewproj())
                    },
                )
            },
        )?;

        Ok(())
    }

    /// Return the max capacity of cameras.
    pub fn max_capacity(&self) -> u32 {
        self.max_capacity
    }
}

#[derive(Copy, Clone, From, Into, Deref)]
pub struct CameraIndex(u32);
