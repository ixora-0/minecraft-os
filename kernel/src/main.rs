#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(kernel::test_runner)]

use bootloader_api::BootInfo;
use core::panic::PanicInfo;
use embedded_graphics::draw_target::DrawTarget;
use kernel::{
    logger::init_logger,
    rendering::{Color, Renderer, init_framebuffer},
};

bootloader_api::entry_point!(kernel_main);
fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    {
        // free the doubly wrapped framebuffer from the boot info struct
        let frame_buffer_optional = &mut boot_info.framebuffer;

        // free the wrapped framebuffer from the FFI-safe abstraction provided by bootloader_api
        let frame_buffer_option = frame_buffer_optional.as_mut();

        // unwrap the framebuffer
        let framebuffer = frame_buffer_option.unwrap();
        init_framebuffer(framebuffer);
    }
    let mut renderer = Renderer::new();
    renderer.clear(Color::BLACK);
    init_logger();
    log::info!("Hello, World!");
    kernel::hlt_loop();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    log::error!("{}", info);
    kernel::hlt_loop();
}
