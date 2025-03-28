#![feature(custom_test_frameworks)]
#![test_runner(osdev_rust::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![no_std]
#![no_main]

use core::panic::PanicInfo;
use osdev_rust::println;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("Hello World!");

    osdev_rust::init();

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
