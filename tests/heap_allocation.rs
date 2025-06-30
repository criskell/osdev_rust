#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(osdev_rust::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use alloc::{boxed::Box, vec::Vec};
use bootloader::{BootInfo, entry_point};
use core::panic::PanicInfo;
use osdev_rust::allocator::HEAP_SIZE;

entry_point!(main);

fn main(boot_info: &'static BootInfo) -> ! {
    use osdev_rust::allocator;
    use osdev_rust::memory::{self, BootInfoFrameAllocator};
    use x86_64::VirtAddr;

    osdev_rust::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    test_main();
    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    osdev_rust::test_panic_handler(info);
}

#[test_case]
fn simple_allocation() {
    let heap_value_1 = Box::new(42);
    let heap_value_2 = Box::new(43);

    assert_eq!(*heap_value_1, 42);
    assert_eq!(*heap_value_2, 43);
}

#[test_case]
fn large_vec() {
    let n = 1000;
    let mut vec = Vec::new();

    for i in 0..n {
        vec.push(i);
    }

    assert_eq!(vec.iter().sum::<u64>(), (n - 1) * n / 2);
}

#[test_case]
fn many_boxes() {
    for i in 0..HEAP_SIZE {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
}
