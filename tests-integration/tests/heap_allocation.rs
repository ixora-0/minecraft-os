#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(tests_integration::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use core::panic::PanicInfo;
use kernel::{
    self, allocator,
    memory::{self, BootInfoFrameAllocator},
};
use tests_integration::test_panic_handler;
use x86_64::VirtAddr;

use bootloader_api::info::BootInfo;

bootloader_api::entry_point!(test_kernel_main, config = &kernel::BOOTLOADER_CONFIG);
fn test_kernel_main(boot_info: &'static mut BootInfo) -> ! {
    kernel::init();

    // memory
    let mut mapper = match boot_info.physical_memory_offset.into_option() {
        Some(physical_memory_offset) => unsafe {
            memory::init(VirtAddr::new(physical_memory_offset))
        },
        None => panic!("Physical memory offset not provided"),
    };

    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_regions) };
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    test_main();
    kernel::hlt_loop()
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}

#[test_case]
fn simple_allocation() {
    use alloc::boxed::Box;
    let heap_value_1 = Box::new(41);
    let heap_value_2 = Box::new(13);
    assert_eq!(*heap_value_1, 41);
    assert_eq!(*heap_value_2, 13);
}

#[test_case]
fn large_vec() {
    use alloc::vec::Vec;
    let n = 1000;
    let mut vec = Vec::new();
    for i in 0..n {
        vec.push(i);
    }
    assert_eq!(vec.iter().sum::<u64>(), (n - 1) * n / 2);
}

#[test_case]
fn many_boxes() {
    use alloc::boxed::Box;
    use kernel::allocator::HEAP_SIZE;

    for i in 0..HEAP_SIZE {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
}
