#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(osdev_rust::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

use osdev_rust::println;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    test_main();

    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    osdev_rust::test_panic_handler(info)
}

#[test_case]
fn test_println() {
    println!("Hello, World!");
}
