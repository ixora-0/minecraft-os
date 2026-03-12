#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(tests_integration::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(abi_x86_interrupt)]

use bootloader_api::info::BootInfo;
use core::panic::PanicInfo;
use core::ptr::NonNull;
use kernel;
use kernel::serial_println;
use lazy_static::lazy_static;
use tests_integration::{QemuExitCode, exit_qemu, test_panic_handler};
use volatile::VolatilePtr;
use x86_64::structures::idt::InterruptDescriptorTable;
use x86_64::structures::idt::InterruptStackFrame;

bootloader_api::entry_point!(test_kernel_main, config = &kernel::BOOTLOADER_CONFIG);
/// Entry point for `cargo test`
fn test_kernel_main(_boot_info: &'static mut BootInfo) -> ! {
    kernel::gdt::init();
    init_test_idt();
    stack_overflow();

    panic!("Execution continued after stack overflow");
}

#[allow(unconditional_recursion)]
fn stack_overflow() {
    stack_overflow(); // for each recursion, the return address is pushed
    // prevent tail recursion optimizations
    let mut x = 0u8;
    unsafe {
        let _ = VolatilePtr::new(NonNull::from(&mut x)).read();
    }
}

lazy_static! {
    static ref TEST_IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        unsafe {
            idt.double_fault
                .set_handler_fn(test_double_fault_handler)
                .set_stack_index(kernel::gdt::DOUBLE_FAULT_IST_INDEX);
        }

        idt
    };
}
pub fn init_test_idt() {
    TEST_IDT.load();
}
extern "x86-interrupt" fn test_double_fault_handler(
    _stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}
