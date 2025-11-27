use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;
use x86_64::{
    VirtAddr,
    structures::paging::{
        FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB, mapper::MapToError,
    },
};

#[allow(unused_imports)]
use crate::allocator::bump::BumpAllocator;
use crate::allocator::fixed_size_block::FixedSizeBlockAllocator;
#[allow(unused_imports)]
use crate::allocator::linked_list::LinkedListAllocator;

pub mod bump;
pub mod fixed_size_block;
pub mod linked_list;

pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 100 * 1024; // 100 KiB

pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);

        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;

        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush();
        };
    }

    unsafe {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    Ok(())
}

#[global_allocator]
static ALLOCATOR: Locked<FixedSizeBlockAllocator> = Locked::new(FixedSizeBlockAllocator::new());

pub struct Dummy;

unsafe impl GlobalAlloc for Dummy {
    unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
        null_mut()
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        panic!("dealloc should be never called")
    }
}

pub struct Locked<A> {
    inner: spin::Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Locked {
            inner: spin::Mutex::new(inner),
        }
    }

    pub fn lock(&self) -> spin::MutexGuard<A> {
        self.inner.lock()
    }
}

#[allow(dead_code)]
fn align_up_slow(addr: usize, align: usize) -> usize {
    let remainder = addr % align;

    if remainder == 0 {
        addr
    } else {
        addr - remainder + align
    }
}

/// Require that `align` is power of two.
fn align_up(addr: usize, align: usize) -> usize {
    // align é uma potência de dois, portanto tem apenas um único bit setado
    // align - 1 deixa todos os bits menores desligados
    // ! deixa todos os bits setados com exceção daqueles que são menores que align

    // fazer um AND com um endereço e essa máscara, alinhamos o endereço para baixo, devido que
    // desligamos todos os bits abaixo

    // para fazer um alinhamento pra cima, somamos com align - 1 antes de aplicar a máscara.
    (addr + align - 1) & !(align - 1)
}
