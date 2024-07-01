#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq)]
/// Maintain allocations into an external buffer
pub struct Allocation {
    start: usize,
    size: usize,
}

impl Allocation {
    #[inline(always)]
    fn continues_to(&self, right: &Allocation) -> bool {
        self.start + self.size == right.start
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn size(&self) -> usize {
        self.size
    }
}

pub struct BufferAllocator {
    free: Vec<Allocation>,
    total_size: usize,
}

impl BufferAllocator {
    pub fn new(size: usize) -> Self {
        Self {
            free: vec![Allocation { start: 0, size }],
            total_size: size,
        }
    }

    pub fn grow(&mut self, size: usize) {
        self.deallocate(Allocation {
            start: self.total_size,
            size,
        });

        self.total_size += size;
    }

    pub fn grow_to(&mut self, size: usize) {
        self.deallocate(Allocation {
            start: self.total_size,
            size: size - self.total_size,
        });

        self.total_size = size;
    }
    pub fn allocate(&mut self, size: usize) -> Option<Allocation> {
        tracing::debug!("Allocating {size}, free_list: {:?}", self.free);

        let (idx, block) = self
            .free
            .iter_mut()
            .enumerate()
            .filter(|(_, v)| v.size >= size)
            .min_by_key(|(_, v)| v.size - size)?;

        if block.size == size {
            Some(self.free.remove(idx))
        } else {
            // Split off
            let start = block.start;
            *block = Allocation {
                start: block.start + size,
                size: block.size - size,
            };

            Some(Allocation { start, size })
        }
    }

    pub fn deallocate(&mut self, block: Allocation) {
        if block.size() == 0 {
            return;
        }

        if self.free.is_empty() {
            self.free.push(block);
            tracing::debug!("Pushing to end of free list");
            return;
        }

        let idx = self
            .free
            .binary_search_by_key(&block.start, |v| v.start)
            .expect_err("Block is not in free list");

        if idx == 0 {
            // merge right
            let r = &mut self.free[0];
            if block.continues_to(r) {
                r.start -= block.size;
                assert_eq!(r.start, block.start);
                r.size += block.size;
            } else {
                self.free.insert(0, block);
            }
        } else if idx != self.free.len() {
            let [l, r] = &mut self.free[idx - 1..=idx] else {
                unreachable!()
            };

            if l.continues_to(&block) && block.continues_to(r) {
                l.size += block.size + r.size;
                self.free.remove(idx);
            } else if l.continues_to(&block) {
                l.size += block.size;
            } else if block.continues_to(r) {
                r.start -= block.size;
            } else {
                self.free.insert(idx, block);
            }
        } else {
            assert_eq!(idx, self.free.len());
            assert_ne!(idx, 0);

            let l = &mut self.free[idx - 1];

            if l.continues_to(&block) {
                l.size += block.size;
            } else {
                self.free.insert(idx, block);
            }
        }
    }

    pub fn total_size(&self) -> usize {
        self.total_size
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_alloc() {
        let mut allocator = BufferAllocator::new(128);

        let b1 = allocator.allocate(4).unwrap();
        assert_eq!(b1, Allocation { start: 0, size: 4 });
        let b2 = allocator.allocate(8).unwrap();
        assert_eq!(b2, Allocation { start: 4, size: 8 });

        assert_eq!(
            allocator.free,
            [Allocation {
                start: 12,
                size: 116
            }]
        );

        allocator.deallocate(b2);
        assert_eq!(
            allocator.free,
            [Allocation {
                start: 4,
                size: 124
            }]
        );
        allocator.deallocate(b1);
        assert_eq!(
            allocator.free,
            [Allocation {
                start: 0,
                size: 128
            }]
        );
    }

    #[test]
    fn test_alloc_mid() {
        let mut allocator = BufferAllocator::new(128);

        let b0 = allocator.allocate(8).unwrap();
        let b1 = allocator.allocate(4).unwrap();
        assert_eq!(b1, Allocation { start: 8, size: 4 });

        let b2 = allocator.allocate(32).unwrap();
        assert_eq!(
            b2,
            Allocation {
                start: 12,
                size: 32
            }
        );

        let b3 = allocator.allocate(6).unwrap();
        assert_eq!(b3, Allocation { start: 44, size: 6 });

        assert_eq!(
            allocator.free,
            [Allocation {
                start: 50,
                size: 78
            }]
        );

        allocator.deallocate(b3);
        assert_eq!(
            allocator.free,
            [Allocation {
                start: 44,
                size: 84
            }]
        );

        allocator.deallocate(b1);
        assert_eq!(
            allocator.free,
            [
                Allocation { start: 8, size: 4 },
                Allocation {
                    start: 44,
                    size: 84
                }
            ]
        );

        allocator.deallocate(b2);

        assert_eq!(
            allocator.free,
            [Allocation {
                start: 8,
                size: 120
            },]
        );

        allocator.deallocate(b0);

        assert_eq!(
            allocator.free,
            [Allocation {
                start: 0,
                size: 128
            },]
        );
    }
}
