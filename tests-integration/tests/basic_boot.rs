#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(tests_integration::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use kernel;
use tests_integration::test_panic_handler;

use bootloader_api::{entry_point, info::BootInfo};

entry_point!(test_kernel_main);
/// Entry point for `cargo test`
fn test_kernel_main(_boot_info: &'static mut BootInfo) -> ! {
    kernel::init();
    test_main();
    kernel::hlt_loop()
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}
