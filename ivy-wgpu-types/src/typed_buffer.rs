use std::{
    marker::PhantomData,
    mem::{self, size_of},
    ops::{Bound, Deref, RangeBounds},
};

use bytemuck::Pod;
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    Buffer, BufferAsyncError, BufferDescriptor, BufferSlice, BufferUsages, BufferView,
    CommandEncoder, CommandEncoderDescriptor, MapMode, Queue,
};

use crate::Gpu;

/// Type safe buffer
pub struct TypedBuffer<T> {
    buffer: Buffer,
    len: usize,
    label: String,
    _marker: PhantomData<T>,
}

impl<T> Deref for TypedBuffer<T> {
    type Target = Buffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl<T> TypedBuffer<T>
where
    T: Pod,
{
    pub fn new(gpu: &Gpu, label: impl Into<String>, usage: BufferUsages, data: &[T]) -> Self {
        let label = label.into();

        let buffer = gpu.device.create_buffer_init(&BufferInitDescriptor {
            label: Some(&label),
            contents: bytemuck::cast_slice(data),
            usage,
        });

        Self {
            buffer,
            len: data.len(),
            label,
            _marker: PhantomData,
        }
    }

    pub fn new_uninit(
        gpu: &Gpu,
        label: impl Into<String>,
        usage: BufferUsages,
        len: usize,
    ) -> Self {
        let label = label.into();
        let buffer = gpu.device.create_buffer(&BufferDescriptor {
            label: Some(&label),
            usage,
            size: (size_of::<T>() as u64 * len as u64),
            mapped_at_creation: false,
        });

        Self {
            buffer,
            len,
            label,
            _marker: PhantomData,
        }
    }

    pub fn new_uninit_aligned(
        gpu: &Gpu,
        label: impl Into<String>,
        usage: BufferUsages,
        len: usize,
        align: usize,
    ) -> Self {
        let label = label.into();
        let buffer = gpu.device.create_buffer(&BufferDescriptor {
            label: Some(&label),
            usage,
            size: ((size_of::<T>() as u64).max(align as u64) * len as u64),
            mapped_at_creation: false,
        });

        Self {
            buffer,
            len,
            label,
            _marker: PhantomData,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn copy_to_buffer(&self, encoder: &mut CommandEncoder, dst: &Self) {
        encoder.copy_buffer_to_buffer(
            &self.buffer,
            0,
            &dst.buffer,
            0,
            self.len() as u64 * mem::size_of::<T>() as u64,
        )
    }

    pub fn write(&self, queue: &Queue, offset: usize, data: &[T]) {
        assert!(
            self.len() >= offset + data.len(),
            "write {}:{} out of bounds",
            offset,
            data.len()
        );

        let offset = offset as u64 * mem::size_of::<T>() as u64;

        queue.write_buffer(self.buffer(), offset, bytemuck::cast_slice(data));
    }

    pub fn resize(&mut self, gpu: &Gpu, new_len: usize, preserve_contents: bool) {
        tracing::debug!(?new_len, "resize");
        let mut encoder = gpu
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some(&self.label),
            });

        let buffer = gpu.device.create_buffer(&BufferDescriptor {
            label: Some(&self.label),
            usage: self.buffer.usage(),
            size: (size_of::<T>() as u64 * new_len as u64),
            mapped_at_creation: false,
        });

        if preserve_contents {
            encoder.copy_buffer_to_buffer(
                self.buffer(),
                0,
                &buffer,
                0,
                self.len() as u64 * mem::size_of::<T>() as u64,
            );

            gpu.queue.submit([encoder.finish()]);
        }

        self.len = new_len;
        self.buffer = buffer;
    }

    pub fn slice(&self, bounds: impl RangeBounds<usize>) -> BufferSlice<'_> {
        let start = match bounds.start_bound() {
            Bound::Included(&bound) => Bound::Included(bound as u64 * size_of::<T>() as u64),
            Bound::Excluded(&bound) => Bound::Excluded(bound as u64 * size_of::<T>() as u64),
            Bound::Unbounded => Bound::Unbounded,
        };

        let end = match bounds.end_bound() {
            Bound::Included(&bound) => Bound::Included(bound as u64 * size_of::<T>() as u64),
            Bound::Excluded(&bound) => Bound::Excluded(bound as u64 * size_of::<T>() as u64),
            Bound::Unbounded => Bound::Unbounded,
        };

        self.buffer.slice((start, end))
    }

    pub async fn map(
        &self,
        gpu: &Gpu,
        bounds: impl RangeBounds<u64>,
    ) -> Result<BufferView, BufferAsyncError> {
        let slice = self.buffer.slice(bounds);
        let (tx, rx) = futures::channel::oneshot::channel();

        tracing::info!("mapping");
        slice.map_async(MapMode::Read, move |result| {
            tracing::debug!("mapped buffer");
            tx.send(result).ok();
        });

        tracing::info!("polling");
        gpu.device.poll(wgpu::MaintainBase::Wait);

        rx.await.unwrap()?;

        let view = slice.get_mapped_range();
        Ok(view)
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }
}
