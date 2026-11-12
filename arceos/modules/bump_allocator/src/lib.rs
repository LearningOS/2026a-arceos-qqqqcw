#![no_std]

use allocator::{BaseAllocator, ByteAllocator, PageAllocator};

/// Early memory allocator
/// Use it before formal bytes-allocator and pages-allocator can work!
/// This is a double-end memory range:
/// - Alloc bytes forward
/// - Alloc pages backward
///
/// [ bytes-used | avail-area | pages-used ]
/// |            | -->    <-- |            |
/// start       b_pos        p_pos       end
///
/// For bytes area, 'count' records number of allocations.
/// When it goes down to ZERO, free bytes-used area.
/// For pages area, it will never be freed!
///
pub struct EarlyAllocator<const SIZE: usize> {
    start: usize,
    end: usize,
    b_pos: usize,
    p_pos: usize,
    count: usize,
}

impl<const SIZE: usize> EarlyAllocator<SIZE> {
    pub const fn new() -> Self {
        Self {
            start: 0,
            end: 0,
            b_pos: 0,
            p_pos: 0,
            count: 0,
        }
    }
}

impl<const SIZE: usize> BaseAllocator for EarlyAllocator<SIZE> {
    fn init(&mut self, start: usize, size: usize) {
        self.start = start;
        self.end = start + size;
        self.b_pos = start;
        self.p_pos = self.end;
        self.count = 0;
    }

    fn add_memory(&mut self, _start: usize, _size: usize) -> allocator::AllocResult {
        Err(allocator::AllocError::NoMemory)
    }
}

impl<const SIZE: usize> ByteAllocator for EarlyAllocator<SIZE> {
    fn alloc(
        &mut self,
        layout: core::alloc::Layout,
    ) -> allocator::AllocResult<core::ptr::NonNull<u8>> {
        let align = layout.align();
        let alloc_start = (self.b_pos + align - 1) & !(align - 1);
        let alloc_end = alloc_start
            .checked_add(layout.size())
            .ok_or(allocator::AllocError::NoMemory)?;

        if alloc_end > self.p_pos {
            return Err(allocator::AllocError::NoMemory);
        }

        self.b_pos = alloc_end;
        self.count += 1;
        core::ptr::NonNull::new(alloc_start as *mut u8).ok_or(allocator::AllocError::NoMemory)
    }

    fn dealloc(&mut self, pos: core::ptr::NonNull<u8>, layout: core::alloc::Layout) {
        self.count = self.count.saturating_sub(1);
        if self.count == 0 {
            self.b_pos = self.start;
        } else if pos.as_ptr() as usize + layout.size() == self.b_pos {
            self.b_pos = pos.as_ptr() as usize;
        }
    }

    fn total_bytes(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    fn used_bytes(&self) -> usize {
        self.b_pos.saturating_sub(self.start)
    }

    fn available_bytes(&self) -> usize {
        self.p_pos.saturating_sub(self.b_pos)
    }
}

impl<const SIZE: usize> PageAllocator for EarlyAllocator<SIZE> {
    const PAGE_SIZE: usize = SIZE;

    fn alloc_pages(
        &mut self,
        num_pages: usize,
        align_pow2: usize,
    ) -> allocator::AllocResult<usize> {
        let size = num_pages
            .checked_mul(SIZE)
            .ok_or(allocator::AllocError::NoMemory)?;

        let align = if align_pow2.is_power_of_two() {
            align_pow2.max(SIZE)
        } else if align_pow2 > 0 && align_pow2 < 32 {
            (1usize << align_pow2).max(SIZE)
        } else {
            SIZE
        };

        if self.p_pos < size {
            return Err(allocator::AllocError::NoMemory);
        }

        let target = self.p_pos - size;
        let alloc_start = target & !(align - 1);

        if alloc_start < self.b_pos {
            return Err(allocator::AllocError::NoMemory);
        }

        self.p_pos = alloc_start;
        Ok(alloc_start)
    }

    fn dealloc_pages(&mut self, _pos: usize, _num_pages: usize) {
        // As documented: "For pages area, it will never be freed!"
    }

    fn total_pages(&self) -> usize {
        self.total_bytes() / SIZE
    }

    fn used_pages(&self) -> usize {
        self.end.saturating_sub(self.p_pos) / SIZE
    }

    fn available_pages(&self) -> usize {
        self.available_bytes() / SIZE
    }
}