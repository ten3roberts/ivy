use bytemuck::Pod;
use std::{marker::PhantomData, ops::RangeBounds, sync::Arc};

use bytemuck::Zeroable;
use wgpu::{util::DeviceExt, BufferUsages};

use crate::Gpu;

#[derive(Debug)]
pub struct Buffer<T> {
    gpu: Arc<Gpu>,
    buffer: wgpu::Buffer,
    len: u32,
    _marker: PhantomData<T>,
}

impl<T> Buffer<T>
where
    T: Pod + Zeroable,
{
    pub fn new(gpu: Arc<Gpu>, label: &str, usage: BufferUsages, data: &[T]) -> Self {
        let buffer = gpu
            .device()
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(data),
                usage,
            });

        Self {
            gpu,
            buffer,
            len: data.len() as _,
            _marker: PhantomData,
        }
    }

    pub fn write(&self, offset: u64, data: &[T]) {
        self.gpu
            .queue()
            .write_buffer(&self.buffer, offset, bytemuck::cast_slice(data))
    }

    pub fn slice(&self, bounds: impl RangeBounds<u64>) -> wgpu::BufferSlice {
        self.buffer.slice(bounds)
    }

    /// Get the buffer's len.
    #[must_use]
    pub fn len(&self) -> u32 {
        self.len
    }
}
