use derive_more::{Deref, From, Into};
use std::{mem::size_of, sync::Arc};

use hecs::{Entity, World};
use ivy_graphics::Error;
use ivy_vulkan::{Buffer, BufferAccess, BufferType, VulkanContext};

use crate::camera::{Camera, CameraData};

/// Manages the GPU side data for cameras.
/// *Note*: There can only exist on `CameraManager`
pub struct CameraManager {
    camera_buffers: Vec<Buffer>,
    max_camera_index: u32,
    // The offset for each camera satisfying alignment requirements.
    dynamic_offset: u32,
    max_cameras: u32,
}

impl CameraManager {
    /// Creates a new camera manager.
    pub fn new(
        context: Arc<VulkanContext>,
        max_cameras: u32,
        frames_in_flight: usize,
    ) -> Result<Self, Error> {
        let dynamic_offset = (size_of::<CameraData>() as u32)
            .max(context.limits().min_uniform_buffer_offset_alignment as u32);

        dbg!(dynamic_offset);

        let camera_buffers = (0..frames_in_flight)
            .map(|_| {
                Buffer::new_uninit(
                    context.clone(),
                    BufferType::UniformDynamic,
                    BufferAccess::Mapped,
                    max_cameras as u64 * dynamic_offset as u64,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        // let descriptor_allocator = DescriptorAllocator::new(
        //     context.device().clone(),
        //     max_cameras * frames_in_flight as u32,
        // );

        // let sets = Vec::with_capacity(max_cameras as usize * frames_in_flight);

        Ok(Self {
            camera_buffers,
            max_camera_index: 0,
            dynamic_offset,
            max_cameras,
        })
    }

    /// Returns the dynamic offset per camera
    pub fn dynamic_offset(&self) -> u32 {
        self.dynamic_offset
    }

    /// Registers a camera entity to the GPU side buffer.
    pub fn register(
        &mut self,
        world: &mut World,
        entity: Entity,
        // descriptor_layout_cache: &mut DescriptorLayoutCache,
    ) -> Result<CameraIndex, Error> {
        let index = self.max_camera_index.into();

        // self.sets.extend(
        //     self.camera_buffers
        //         .iter()
        //         .map(|buffer| -> Result<DescriptorSet, Error> {
        //             Ok(DescriptorBuilder::new()
        //                 .bind_uniform_sub_buffer(
        //                     0,
        //                     ShaderStageFlags::VERTEX,
        //                     self.max_camera_index as u64 * size_of::<CameraData>() as u64,
        //                     size_of::<CameraData>() as u64,
        //                     buffer,
        //                 )
        //                 .build_one(
        //                     self.context.device(),
        //                     descriptor_layout_cache,
        //                     &mut self.descriptor_allocator,
        //                 )?
        //                 .0)
        //         })
        //         .collect::<Result<Vec<_>, _>>()?,
        // );

        self.max_camera_index += 1;
        world.insert_one(entity, index)?;

        Ok(index)
    }

    /// Registers all unregistered camera entities.
    pub fn register_cameras(
        &mut self,
        world: &mut World,
        // descriptor_layout_cache: &mut DescriptorLayoutCache,
    ) {
        let ids = world
            .query_mut::<&Camera>()
            .without::<CameraIndex>()
            .into_iter()
            .map(|(e, _)| e)
            .into_iter()
            .collect::<Vec<_>>();

        ids.into_iter().for_each(|e| {
            let _ = self.register(world, e);
        });
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
}

#[derive(Copy, Clone, From, Into, Deref)]
pub struct CameraIndex(u32);
