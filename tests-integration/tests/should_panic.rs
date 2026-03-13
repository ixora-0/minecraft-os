#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(tests_integration::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use kernel::{serial_print, serial_println};
use tests_integration::exit_qemu;
use tests_qemu_exit_code::QemuExitCode;

use bootloader_api::info::BootInfo;

bootloader_api::entry_point!(test_kernel_main, config = &kernel::BOOTLOADER_CONFIG);
/// Entry point for `cargo test`
fn test_kernel_main(_boot_info: &'static mut BootInfo) -> ! {
    should_fail();
    serial_println!("[test did not panic]");
    exit_qemu(QemuExitCode::Failed);
    kernel::hlt_loop()
}

fn should_fail() {
    serial_print!("should_panic::should_fail...\t");
    assert_eq!(0, 1);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}
