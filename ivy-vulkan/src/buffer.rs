//! A buffer represents a piece of memory that can be accessed by the GPU and used to store and
//! write data. Buffers
use crate::{commands::*, context::VulkanContext, descriptors::DescriptorBindable, Error, Result};

use gpu_allocator::{
    vulkan::{self, *},
    MemoryLocation,
};
use ivy_base::Extent;
use std::{
    ffi::c_void,
    mem::{self, size_of},
    ptr::{copy_nonoverlapping, NonNull},
    sync::Arc,
};

use ash::vk;
use vk::DeviceSize;

/// Re-export
pub use vk::BufferUsageFlags as BufferUsage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// Defines the expected access pattern of a buffer
pub enum BufferAccess {
    /// Buffer data will be set once or rarely and frequently times
    /// Uses temporary staging buffers and optimizes for GPU read access
    Staged,

    /// Buffer data is often updated and frequently used
    /// Uses temporarily mapped host memory
    Mapped,
}

/// Higher level construct abstracting buffer and buffer memory for index,
/// vertex and uniform use
/// buffer access
pub struct Buffer {
    context: Arc<VulkanContext>,
    buffer: vk::Buffer,
    allocation: Option<vulkan::Allocation>,

    usage: BufferUsage,
    access: BufferAccess,
    size: DeviceSize,
}

impl Buffer {
    /// Creates a new buffer with size and uninitialized contents.
    pub fn new_uninit<T: Sized>(
        context: Arc<VulkanContext>,
        usage: BufferUsage,
        access: BufferAccess,
        len: DeviceSize,
    ) -> Result<Self> {
        let size = len * size_of::<T>() as u64;

        let location = match access {
            BufferAccess::Staged => MemoryLocation::GpuOnly,
            BufferAccess::Mapped => MemoryLocation::CpuToGpu,
        };

        let usage = match access {
            BufferAccess::Staged => usage | BufferUsage::TRANSFER_DST,
            _ => usage,
        };

        let device = context.device();

        // Create the main GPU side buffer
        let buffer_info = vk::BufferCreateInfo::builder()
            .size(size)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let buffer = unsafe { device.create_buffer(&buffer_info, None)? };

        let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };

        let allocator = context.allocator();

        // Create the buffer
        let allocation = allocator.write().allocate(&AllocationCreateDesc {
            name: "Buffer",
            requirements,
            location,
            linear: true,
        })?;

        unsafe { device.bind_buffer_memory(buffer, allocation.memory(), allocation.offset())? };

        Ok(Self {
            size,
            context,
            buffer,
            allocation: Some(allocation),
            usage,
            access,
        })
    }

    /// Creates a new buffer and fills it with vertex data using staging
    /// buffer. Buffer will be the same size as provided data.
    pub fn new<T>(
        context: Arc<VulkanContext>,
        usage: BufferUsage,
        access: BufferAccess,
        data: &[T],
    ) -> Result<Self> {
        let mut buffer = Self::new_uninit::<T>(context, usage, access, data.len() as u64)?;

        // Fill the buffer with provided data
        buffer.fill(0, data)?;

        Ok(buffer)
    }

    /// Creates a new buffer and fills it with vertex data using staging
    /// buffer. Buffer will be the same size as provided data.
    pub fn new_iter<T>(
        context: Arc<VulkanContext>,
        usage: BufferUsage,
        access: BufferAccess,
        len: DeviceSize,
        iter: impl IntoIterator<Item = T>,
    ) -> Result<Self> {
        let mut buffer = Self::new_uninit::<T>(context, usage, access, len)?;

        // Fill the buffer with provided data
        buffer.write_iter(0, iter.into_iter())?;

        Ok(buffer)
    }

    pub fn mapped_ptr<T: Sized>(&self) -> Option<NonNull<T>> {
        self.allocation
            .as_ref()
            .and_then(|val| val.mapped_ptr())
            .map(|val| val.cast::<T>())
    }

    pub fn mapped_slice<T: Sized>(&self) -> Option<&[T]> {
        self.allocation
            .as_ref()
            .and_then(|val| val.mapped_ptr())
            .map(|val| unsafe {
                std::slice::from_raw_parts(
                    val.cast::<T>().as_ptr(),
                    self.size as usize / size_of::<T>(),
                )
            })
    }

    pub fn mapped_slice_mut<T: Sized>(&mut self) -> Option<&mut [T]> {
        self.allocation
            .as_ref()
            .and_then(|val| val.mapped_ptr())
            .map(|val| unsafe {
                std::slice::from_raw_parts_mut(
                    val.cast::<T>().as_ptr(),
                    self.size as usize / size_of::<T>(),
                )
            })
    }

    /// Update the buffer data by mapping memory and filling it using the
    /// provided closure.
    /// `len`: Specifies the number of items of T to map into slice. (is ignored with persistent
    /// access).
    /// `offset`: Specifies the offset in items T into buffer to map.
    pub fn write_slice<T, F, R>(
        &mut self,
        len: DeviceSize,
        offset: DeviceSize,
        write_func: F,
    ) -> Result<R>
    where
        F: FnOnce(&mut [T]) -> R,
        R: std::fmt::Debug,
    {
        let size = len * mem::size_of::<T>() as u64;
        self.write(size, offset * mem::size_of::<T>() as u64, |ptr| {
            write_func(unsafe { std::slice::from_raw_parts_mut(ptr.cast().as_ptr(), len as usize) })
        })
    }

    /// Fallible version of [`Self::write_iter`].
    pub fn try_write_iter<
        T: Copy + std::fmt::Debug,
        E,
        I: Iterator<Item = std::result::Result<T, E>>,
    >(
        &mut self,
        offset: usize,
        iter: I,
    ) -> Result<std::result::Result<(), E>> {
        match self.mapped_slice_mut::<T>() {
            Some(slice) => Ok({
                iter.zip(slice)
                    .try_for_each(move |(val, mapped)| -> std::result::Result<(), E> {
                        let val = val?;
                        *mapped = val;

                        Ok(())
                    })
            }),
            None => {
                let mut staging = Buffer::new_uninit::<u8>(
                    self.context.clone(),
                    BufferUsage::TRANSFER_SRC,
                    BufferAccess::Mapped,
                    self.size,
                )?;

                // Use the write function to write into the mapped memory
                let r = staging.try_write_iter(0, iter)?;

                copy(
                    self.context.transfer_pool(),
                    self.context.graphics_queue(),
                    staging.buffer(),
                    self.buffer,
                    self.size as _,
                    (offset * size_of::<T>()) as u64,
                )?;

                Ok(r)
            }
        }
    }

    /// Writes into the buffer starting at offset from the provided iterator.
    /// Offset is given in terms of elements.
    pub fn write_iter<T, I: Iterator<Item = T>>(&mut self, offset: usize, iter: I) -> Result<()> {
        match self.mapped_slice_mut::<T>() {
            Some(slice) => {
                iter.zip(slice).for_each(move |(val, mapped)| {
                    *mapped = val;
                });
                Ok(())
            }
            None => {
                let mut staging = Buffer::new_uninit::<u8>(
                    self.context.clone(),
                    BufferUsage::TRANSFER_SRC,
                    BufferAccess::Mapped,
                    self.size,
                )?;

                // Use the write function to write into the mapped memory
                let r = staging.write_iter(0, iter)?;

                copy(
                    self.context.transfer_pool(),
                    self.context.graphics_queue(),
                    staging.buffer(),
                    self.buffer,
                    self.size as _,
                    (offset * size_of::<T>()) as u64,
                )?;

                Ok(r)
            }
        }
    }

    /// Update the buffer data by mapping memory and filling it using the
    /// provided closure
    /// `size`: Specifies the number of bytes to map (is ignored with persistent
    /// access)
    /// `offset`: Specifies the offset in bytes into buffer to map
    pub fn write<F, R: std::fmt::Debug>(
        &mut self,
        size: DeviceSize,
        offset: DeviceSize,
        write_func: F,
    ) -> Result<R>
    where
        F: FnOnce(NonNull<c_void>) -> R,
    {
        if size > self.size {
            return Err(Error::BufferOverflow {
                size,
                max_size: self.size,
            });
        }
        match self.allocation.as_ref().and_then(|val| val.mapped_ptr()) {
            None => self.write_staged(size, offset, write_func),
            Some(ptr) => Ok(write_func(
                NonNull::new(unsafe { ptr.as_ptr().offset(offset as _) }).unwrap(),
            )),
        }
    }

    fn write_staged<F, R>(&self, size: DeviceSize, offset: DeviceSize, write_func: F) -> Result<R>
    where
        F: FnOnce(NonNull<c_void>) -> R,
        R: std::fmt::Debug,
    {
        let mut staging = Buffer::new_uninit::<u8>(
            self.context.clone(),
            BufferUsage::TRANSFER_SRC,
            BufferAccess::Mapped,
            size,
        )?;

        // Use the write function to write into the mapped memory
        let r = staging.write(size, offset, write_func)?;

        copy(
            self.context.transfer_pool(),
            self.context.graphics_queue(),
            staging.buffer(),
            self.buffer,
            size as _,
            offset,
        )?;

        Ok(r)
    }

    /// Fills the buffer with provided data
    /// data can not be larger in size than maximum buffer size
    pub fn fill<T: Sized>(&mut self, offset: DeviceSize, data: &[T]) -> Result<()> {
        match self.allocation.as_ref().and_then(|val| val.mapped_ptr()) {
            Some(ptr) => unsafe {
                copy_nonoverlapping(data.as_ptr(), ptr.cast().as_ptr(), data.len());
            },
            None => self.fill_staged(offset, data)?,
        }

        Ok(())
    }

    fn fill_staged<T: Sized>(&self, offset: DeviceSize, data: &[T]) -> Result<()> {
        let staging = Buffer::new(
            self.context.clone(),
            BufferUsage::TRANSFER_SRC,
            BufferAccess::Mapped,
            data,
        )?;

        copy(
            self.context.transfer_pool(),
            self.context.graphics_queue(),
            staging.buffer(),
            self.buffer,
            staging.size,
            offset,
        )?;

        Ok(())
    }

    pub fn size(&self) -> DeviceSize {
        self.size
    }

    /// Returns the raw vk buffer
    pub fn buffer(&self) -> vk::Buffer {
        self.buffer
    }

    /// Returns the buffer type
    pub fn access(&self) -> BufferAccess {
        self.access
    }

    /// Returns the buffer type
    pub fn usage(&self) -> BufferUsage {
        self.usage
    }
}

impl AsRef<vk::Buffer> for Buffer {
    fn as_ref(&self) -> &vk::Buffer {
        &self.buffer
    }
}

impl From<&Buffer> for vk::Buffer {
    fn from(buffer: &Buffer) -> vk::Buffer {
        buffer.buffer
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        let allocator = self.context.allocator();
        let device = self.context.device();

        allocator
            .write()
            .free(self.allocation.take().unwrap())
            .unwrap();

        unsafe { device.destroy_buffer(self.buffer, None) }
    }
}

/// Copies the contents of one buffer to another
/// `commandpool`: pool to allocate transfer command buffer
/// Does not wait for operation to complete
pub fn copy(
    commandpool: &CommandPool,
    queue: vk::Queue,
    src_buffer: vk::Buffer,
    dst_buffer: vk::Buffer,
    size: DeviceSize,
    offset: DeviceSize,
) -> Result<()> {
    let region = vk::BufferCopy {
        src_offset: 0,
        dst_offset: offset,
        size,
    };

    commandpool.single_time_command(queue, |commandbuffer| {
        commandbuffer.copy_buffer(src_buffer, dst_buffer, &[region]);
    })
}

pub fn copy_to_image(
    commandpool: &CommandPool,
    queue: vk::Queue,
    buffer: vk::Buffer,
    image: vk::Image,
    layout: vk::ImageLayout,
    extent: Extent,
) -> Result<()> {
    let region = vk::BufferImageCopy {
        buffer_offset: 0,
        buffer_row_length: 0,
        buffer_image_height: 0,
        image_subresource: vk::ImageSubresourceLayers {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            mip_level: 0,
            base_array_layer: 0,
            layer_count: 1,
        },
        image_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
        image_extent: vk::Extent3D {
            width: extent.width,
            height: extent.height,
            depth: 1,
        },
    };

    commandpool.single_time_command(queue, |commandbuffer| {
        commandbuffer.copy_buffer_image(buffer, image, layout, &[region])
    })
}

impl DescriptorBindable for Buffer {
    fn bind_resource<'a>(
        &self,
        binding: u32,
        stage: vk::ShaderStageFlags,
        builder: &'a mut crate::descriptors::DescriptorBuilder,
    ) -> Result<&'a mut crate::descriptors::DescriptorBuilder> {
        builder.bind_buffer(binding, stage, self)
    }
}
