use std::{marker::PhantomData, ops::RangeBounds};

use bytemuck::Pod;
use wgpu::{Buffer, BufferSlice, BufferUsages, Queue};

use super::{
    allocator::{Allocation, BufferAllocator},
    Gpu, TypedBuffer,
};

pub struct SubBuffer<T> {
    block: Allocation,
    _marker: PhantomData<T>,
}

impl<T> SubBuffer<T> {
    pub fn size(&self) -> usize {
        self.block.size()
    }

    pub fn offset(&self) -> usize {
        self.block.start()
    }
}

impl<T> std::hash::Hash for SubBuffer<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.block.hash(state);
    }
}

impl<T> Eq for SubBuffer<T> {}

impl<T> PartialEq for SubBuffer<T> {
    fn eq(&self, other: &Self) -> bool {
        self.block.eq(&other.block)
    }
}

impl<T> Clone for SubBuffer<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for SubBuffer<T> {}

impl<T> std::fmt::Debug for SubBuffer<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubBuffer")
            .field("block", &self.block)
            .finish()
    }
}

pub struct MultiBuffer<T> {
    label: String,
    buffer: TypedBuffer<T>,
    allocator: BufferAllocator,
}

impl<T> MultiBuffer<T>
where
    T: Pod,
{
    pub fn new(gpu: &Gpu, label: impl Into<String>, usage: BufferUsages, capacity: usize) -> Self {
        let label = label.into();
        let buffer = TypedBuffer::new_uninit(gpu, &label, usage, capacity);
        let allocator = BufferAllocator::new(capacity);

        Self {
            buffer,
            allocator,
            label,
        }
    }

    pub fn grow(&mut self, gpu: &Gpu, size: usize) {
        let size = (self.buffer.len() + size.next_power_of_two()).next_power_of_two();
        tracing::debug!(?size, "grow");
        self.allocator.grow_to(size);

        self.buffer.resize(gpu, self.allocator.total_size());
    }

    pub fn allocate(&mut self, len: usize) -> Option<SubBuffer<T>> {
        Some(SubBuffer {
            block: self.allocator.allocate(len)?,
            _marker: PhantomData,
        })
    }

    pub fn try_reallocate(
        &mut self,
        sub_buffer: SubBuffer<T>,
        new_len: usize,
    ) -> Option<SubBuffer<T>> {
        if sub_buffer.block.size() == new_len {
            Some(SubBuffer {
                block: sub_buffer.block,
                _marker: PhantomData,
            })
        } else {
            tracing::debug!("reallocating {sub_buffer:?} to {new_len}");
            self.deallocate(sub_buffer);
            self.allocate(new_len)
        }
    }

    pub fn deallocate(&mut self, sub_buffer: SubBuffer<T>) {
        self.allocator.deallocate(sub_buffer.block)
    }

    pub fn get(&self, sub_buffer: &SubBuffer<T>) -> BufferSlice {
        self.buffer
            .slice(sub_buffer.offset()..sub_buffer.offset() + sub_buffer.size())
    }

    pub fn write(&self, queue: &Queue, allocation: &SubBuffer<T>, data: &[T]) {
        assert!(
            data.len() <= allocation.size(),
            "write exceeds allocation {} > {}",
            data.len(),
            allocation.size()
        );

        self.buffer.write(queue, allocation.block.start(), data);
    }

    pub fn slice(&self, bounds: impl RangeBounds<usize>) -> BufferSlice {
        self.buffer.slice(bounds)
    }

    pub fn buffer(&self) -> &Buffer {
        self.buffer.buffer()
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn label(&self) -> &str {
        self.label.as_ref()
    }
}
