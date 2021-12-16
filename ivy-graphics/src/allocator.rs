use std::marker::PhantomData;

/// Manage allocation of arbitrary long chunks of memory from an externally backed memory pool.
///
/// This is useful for fitting many things inside a vulkan buffer.
pub struct Allocator<T> {
    free: Vec<BufferAllocation<T>>,
    capacity: usize,
}

impl<T> Allocator<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            free: vec![BufferAllocation::new(capacity, 0)],
            capacity,
        }
    }

    /// Allocate `len` elements from the allocator.
    ///
    /// Return None if no contiguous free space was found of len
    pub fn allocate(&mut self, len: usize) -> Option<BufferAllocation<T>> {
        let (index, block) = self
            .free
            .iter_mut()
            .enumerate()
            .find(|(_, block)| block.len >= len)?;

        if block.len == len {
            Some(self.free.remove(index))
        } else {
            let mut block = std::mem::replace(
                block,
                BufferAllocation::new(block.len - len, block.offset + len),
            );

            block.len = len;
            Some(block)
        }
    }

    /// Free an allocation.
    ///
    /// Behaviour is undefined if the allocation was not originally from self
    pub fn free(&mut self, block: BufferAllocation<T>) {
        let index = self
            .free
            .iter()
            .position(|val| val.offset > block.offset)
            .unwrap_or(self.free.len());

        if index != 0 && index != self.free.len() {
            match &mut self.free[index - 1..=index] {
                [a, b] => {
                    if a.offset + a.len + block.len == b.offset {
                        a.len += block.len + b.len;
                        self.free.remove(index);
                    } else if a.offset + a.len == block.offset {
                        a.len += block.len;
                    } else if block.offset + block.len == b.offset {
                        b.len += block.len;
                        b.offset = block.len;
                    } else {
                        self.free.insert(index, block);
                    }
                }
                _ => unreachable!(),
            }
        }
    }

    /// Doubles the available size
    pub fn grow_double(&mut self) {
        self.grow(self.capacity)
    }

    /// Fit additionalelements
    pub fn grow(&mut self, additional: usize) {
        match self.free.pop() {
            Some(val) => self
                .free
                .push(BufferAllocation::new(val.len + additional, val.offset)),
            None => self
                .free
                .push(BufferAllocation::new(additional, self.capacity)),
        }

        self.capacity += additional;
    }

    /// Get a reference to the allocator's capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferAllocation<T> {
    len: usize,
    offset: usize,
    marker: PhantomData<T>,
}

impl<T> BufferAllocation<T> {
    fn new(len: usize, offset: usize) -> Self {
        Self {
            len,
            offset,
            marker: PhantomData,
        }
    }

    /// Get a reference to the buffer allocation's len.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Get a reference to the buffer allocation's offset.
    #[inline]
    pub fn offset(&self) -> usize {
        self.offset
    }
}
