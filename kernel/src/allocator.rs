use core::alloc::GlobalAlloc;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering};

use linked_list_allocator::LockedHeap;
use x86_64::VirtAddr;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB, mapper::MapToError,
};
pub const HEAP_START: *mut u8 = 0x_4444_4444_0000 as *mut u8;
pub const HEAP_SIZE: usize = 2 * 1024 * 1024; // 2 MiB

#[global_allocator]
pub static ALLOCATOR: HeapWithTracker = HeapWithTracker::new();

pub struct HeapWithTracker {
    inner: LockedHeap,
    allocated_bytes: AtomicUsize,
}

impl HeapWithTracker {
    pub const fn new() -> Self {
        HeapWithTracker {
            inner: LockedHeap::empty(),
            allocated_bytes: AtomicUsize::new(0),
        }
    }

    pub fn get_allocated_bytes(&self) -> usize {
        self.allocated_bytes.load(Ordering::SeqCst)
    }

    pub unsafe fn init(&self, start: *mut u8, size: usize) {
        unsafe { self.inner.lock().init(start, size) };
    }
}

unsafe impl GlobalAlloc for HeapWithTracker {
    /// copied from `LockedHeap`'s `alloc`
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        self.allocated_bytes
            .fetch_add(layout.size(), Ordering::SeqCst);
        self.inner
            .lock()
            .allocate_first_fit(layout)
            .ok()
            .map_or(0 as *mut u8, |allocation| allocation.as_ptr())
    }

    /// copied from `LockedHeap`'s `dealloc`
    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        self.allocated_bytes
            .fetch_sub(layout.size(), Ordering::SeqCst);
        unsafe {
            self.inner
                .lock()
                .deallocate(NonNull::new_unchecked(ptr), layout)
        }
    }
}

pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + (HEAP_SIZE as u64) - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe { mapper.map_to(page, frame, flags, frame_allocator)?.flush() };
    }

    unsafe {
        ALLOCATOR.init(HEAP_START, HEAP_SIZE);
    }

    Ok(())
}
