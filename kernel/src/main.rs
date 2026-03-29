#![no_std]
#![no_main]

extern crate alloc;
use bootloader_api::BootInfo;
use core::panic::PanicInfo;
use embedded_graphics::draw_target::DrawTarget;
use glam::Vec3;
use kernel::{
    BOOTLOADER_CONFIG,
    allocator::{self},
    logger::{self, init_logger},
    memory::{self, BootInfoFrameAllocator},
    ps2,
    rendering::{GLOBAL_RENDERER, init_global_renderer},
};
use kernel_core::rendering::Color;
use pc_keyboard::KeyCode;
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

    x86_64::instructions::interrupts::enable();
    // kernel::acpi::shutdown();

    use kernel_core::game;
    let mut camera = game::Camera::default();
    camera.set_position(-3.0, 5.0, -2.0);
    // yaw=-50 deg
    camera.yaw = -50.0_f32.to_radians();
    // yaw=--35 deg
    camera.pitch = -35.0_f32.to_radians();

    let (pixel_format, bytes_per_pixel) = {
        let mut renderer_guard = kernel::rendering::GLOBAL_RENDERER.lock();
        let renderer = renderer_guard
            .get_mut()
            .expect(kernel::rendering::EXPECT_MSG_FRAMEBUFFER_NOT_INITIALIZED);
        (renderer.info.pixel_format, renderer.info.bytes_per_pixel)
    };
    let mut screen = game::Screen::new(20, 20, 160 * 4, 90 * 4, pixel_format, bytes_per_pixel);
    let mut mesh = {
        let world = game::world::WORLD.lock();
        game::world::get_world_mesh(&world)
    };

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
    log::trace!("Entering loop");

    const MOUSE_SENSITIVITY: f32 = 0.005;
    const SPEED: f32 = 1.0;
    const PI: f32 = core::f32::consts::PI;
    let (mut previous_mouse_x, mut previous_mouse_y) = {
        let mouse = ps2::PS2_MOUSE.lock();
        (mouse.x, mouse.y)
    };
    loop {
        // mouse
        let (dx, dy) = {
            let mouse = ps2::PS2_MOUSE.lock();
            let dx = mouse.x - previous_mouse_x;
            let dy = mouse.y - previous_mouse_y;
            previous_mouse_x = mouse.x;
            previous_mouse_y = mouse.y;
            (dx, dy)
        };
        camera.yaw += dx as f32 * MOUSE_SENSITIVITY;
        camera.pitch -= dy as f32 * MOUSE_SENSITIVITY;
        camera.pitch = camera.pitch.clamp(-PI / 2.0 + 0.01, PI / 2.0 - 0.01);

        // keyboard
        let key_states = {
            let keyboard = ps2::PS2_KEYBOARD.lock();
            keyboard.key_states
        };
        {
            // wasd moves on XZ plane
            let forward = camera.forward();
            let forward = Vec3::new(forward.x, 0.0, forward.z).normalize(); //  project onto XZ
            let right = forward.cross(Vec3::Y);
            if key_states.is_pressed(KeyCode::W) {
                camera.position += forward * SPEED;
            }
            if key_states.is_pressed(KeyCode::S) {
                camera.position -= forward * SPEED;
            }
            if key_states.is_pressed(KeyCode::A) {
                camera.position -= right * SPEED;
            }
            if key_states.is_pressed(KeyCode::D) {
                camera.position += right * SPEED;
            }

            // up down
            if key_states.is_pressed(KeyCode::Spacebar) {
                camera.position += Vec3::Y * SPEED;
            }
            if key_states.is_pressed(KeyCode::LShift) {
                camera.position -= Vec3::Y * SPEED;
            }
        }

        // rerender
        {
            screen.render(&camera, &mut mesh);
            let mut renderer_guard = kernel::rendering::GLOBAL_RENDERER.lock();
            let renderer = renderer_guard
                .get_mut()
                .expect(kernel::rendering::EXPECT_MSG_FRAMEBUFFER_NOT_INITIALIZED);
            screen.flush(renderer);
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    log::error!("{}", info);
    kernel::hlt_loop();
}
