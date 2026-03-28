#![no_std]
#![no_main]

extern crate alloc;
use bootloader_api::BootInfo;
use core::panic::PanicInfo;
use embedded_graphics::draw_target::DrawTarget;
use kernel::{
    BOOTLOADER_CONFIG,
    allocator::{self},
    logger::{self, init_logger},
    memory::{self, BootInfoFrameAllocator},
    rendering::{GLOBAL_RENDERER, init_global_renderer},
};
use kernel_core::rendering::Color;
use x86_64::VirtAddr;

bootloader_api::entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);
fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    init_logger();

    log::trace!("Initializing kernel");
    kernel::init(); // init interrupts
    // --- MEMORY ---
    log::trace!("Initializing frame allocator");
    let mut mapper = match boot_info.physical_memory_offset.into_option() {
        Some(physical_memory_offset) => {
            log::trace!("Physical memory offset: 0x{:X?}", physical_memory_offset);
            unsafe { memory::init(VirtAddr::new(physical_memory_offset)) }
        }
        None => panic!("Physical memory offset not provided"),
    };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_regions) };
    log::trace!("Initializing heap");
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    // --- RENDERER ---
    log::trace!("Initializing renderer");
    {
        // free the doubly wrapped framebuffer from the boot info struct
        let frame_buffer_optional = &mut boot_info.framebuffer;

        // free the wrapped framebuffer from the FFI-safe abstraction provided by bootloader_api
        let frame_buffer_option = frame_buffer_optional.as_mut();

        // unwrap the framebuffer
        let framebuffer = frame_buffer_option.unwrap();
        init_global_renderer(framebuffer);
    }
    // clear screen
    {
        let mut renderer_guard = GLOBAL_RENDERER.lock();
        let renderer = renderer_guard.get_mut().expect("msg");
        renderer.clear(Color::LIGHT_GRAY);
    }
    logger::enable_rendering();
    log::info!("Hello, World!");

    const ASCII: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ\nabcdefghijklmnopqrstuvwxyz\n0123456789\n!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~";
    log::info!("ASCII:\n{}", ASCII);

    // --- ACPI ---
    let rsdp_addr = match boot_info.rsdp_addr.into_option() {
        Some(rsdp_addr) => rsdp_addr,
        None => panic!("No RSDP was found (BIOS) or reported (UEFI)"),
    };
    kernel::acpi::init(rsdp_addr);

    kernel::ps2::init();

    {
        let allocated = kernel::allocator::ALLOCATOR.get_allocated_bytes();
        let available = allocator::HEAP_SIZE;
        log::info!(
            "Alocated bytes after init sequence: {} / {} ({}%)",
            allocated,
            available,
            (allocated as f32 / available as f32 * 100.0) as u32
        );
    }

    x86_64::instructions::interrupts::enable();
    // kernel::acpi::shutdown();

    use kernel_core::game;
    let mut camera = game::Camera::default();
    camera.set_position(-3.0, 5.0, -2.0);
    // yaw=-50 deg
    camera.yaw = -50.0_f32.to_radians();
    // yaw=--35 deg
    camera.pitch = -35.0_f32.to_radians();

    let screen = game::Screen::new(20, 20, 160 * 4, 90 * 4);
    let mut mesh = {
        let world = game::world::WORLD.lock();
        game::world::get_world_mesh(&world)
    };

    {
        let mut renderer_guard = kernel::rendering::GLOBAL_RENDERER.lock();
        let renderer = renderer_guard.get_mut().expect("lol");
        screen.render(&camera, &mut mesh, renderer);
    }

    log::trace!("Entering loop");
    kernel::hlt_loop();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    log::error!("{}", info);
    kernel::hlt_loop();
}
