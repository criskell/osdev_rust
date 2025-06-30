#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(osdev_rust::test_runner)]
#![reexport_test_harness_main = "test_main"]

use alloc::{boxed::Box, rc::Rc, vec, vec::Vec};
use bootloader::{BootInfo, entry_point};
use core::panic::PanicInfo;
use osdev_rust::println;

extern crate alloc;

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use osdev_rust::allocator;
    use osdev_rust::memory::{self, BootInfoFrameAllocator};
    use x86_64::VirtAddr;

    println!("Hello World!");

    osdev_rust::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    let heap_value = Box::new(42);
    println!("heap_value at {:p}", heap_value);

    let mut vec = Vec::new();

    for i in 0..500 {
        vec.push(i);
    }

    println!("vec at {:p}", vec.as_slice());

    let reference_counted = Rc::new(vec![1, 2, 3]);
    let cloned_reference = reference_counted.clone();
    println!(
        "current reference count: {}",
        Rc::strong_count(&cloned_reference),
    );
    core::mem::drop(reference_counted);
    println!(
        "reference count is {} now",
        Rc::strong_count(&cloned_reference)
    );

    #[cfg(test)]
    test_main();

    println!("It did not crash!");

    osdev_rust::hlt_loop();
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);

    osdev_rust::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    osdev_rust::test_panic_handler(info)
}
