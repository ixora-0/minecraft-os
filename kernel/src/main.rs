#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(kernel::test_runner)]

use bootloader_api::BootInfo;
use core::panic::PanicInfo;

bootloader_api::entry_point!(kernel_main);
fn kernel_main(_boot_info: &'static mut BootInfo) -> ! {
    kernel::hlt_loop();
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    kernel::hlt_loop();
}
