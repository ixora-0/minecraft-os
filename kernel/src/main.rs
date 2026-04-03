#![no_std]
#![no_main]

extern crate alloc;
use alloc::vec::Vec;
use bootloader_api::BootInfo;
use core::panic::PanicInfo;
use glam::{IVec2, USizeVec2, Vec3};
use kernel::ps2::keyboard::KeyboardEvent;
use kernel::{
    BOOTLOADER_CONFIG,
    allocator::{self},
    console::Console,
    logger::{self, init_logger},
    memory::{self, BootInfoFrameAllocator},
    ps2::{self},
    rendering::{self, init_global_renderer},
};
use kernel_core::{
    game::world,
    rendering::{Color, Rectangle},
};
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
    };
    // clear screen
    let global_renderer_info = rendering::with_global_renderer_mut(|renderer| {
        {
            let mut frame = renderer.frame();
            frame.clear_color(Color::LIGHT_GRAY);
            frame.clear_depth();
        }
        renderer.flush();
        renderer.info()
    });
    // calculate bounds for log and console panels.
    // console is positioned below log, both bottom-left of screen.
    let (logger_bounds, console_bounds) = {
        const LOG_WIDTH: usize = 800;
        const LOG_HEIGHT: usize = 450;
        const LEFT_MARGIN: usize = 12;
        const BOT_MARGIN: usize = 12;
        const CONSOLE_HEIGHT: usize = 48;
        const CONSOLE_GAP: usize = 6;

        let available_width = global_renderer_info.width.saturating_sub(LEFT_MARGIN);
        let available_height = global_renderer_info.height.saturating_sub(BOT_MARGIN);
        let log_width = LOG_WIDTH.min(available_width);
        let log_height = LOG_HEIGHT.min(available_height);
        let log_top =
            (global_renderer_info.height - CONSOLE_HEIGHT - BOT_MARGIN - CONSOLE_GAP - log_height)
                .max(0) as i32;
        let log_left = LEFT_MARGIN as i32;
        let console_top = log_top + log_height as i32 + CONSOLE_GAP as i32;
        (
            Rectangle {
                top_left: IVec2::new(log_left, log_top),
                size: USizeVec2::new(log_width, log_height),
            },
            Rectangle {
                top_left: IVec2::new(log_left, console_top),
                size: USizeVec2::new(log_width, CONSOLE_HEIGHT),
            },
        )
    };

    logger::enable_rendering(logger_bounds);
    log::info!("{:?}", global_renderer_info);

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
    camera.yaw = -50.0_f32.to_radians();
    camera.pitch = -35.0_f32.to_radians();

    let screen = {
        let width = global_renderer_info.width.min(1280);
        let x = (global_renderer_info.width - width) / 2;
        let height = global_renderer_info.height.min(720);
        let y = (global_renderer_info.height - height) / 2;
        game::Screen::new(x as i32, y as i32, width, height)
    };
    let mut mesh = {
        let world = game::world::WORLD.lock();
        game::world::get_world_mesh(&world)
    };

    let mut console = Console::new(console_bounds);
    let mut keyboard_events: Vec<KeyboardEvent> = Vec::with_capacity(64);

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

    // --- MAIN LOOP ---
    log::trace!("Entering loop");
    const MOUSE_SENSITIVITY: f32 = 0.0015;
    const SPEED: f32 = 0.15;
    const PI: f32 = core::f32::consts::PI;
    loop {
        // mouse
        let (dx, dy) = ps2::with_ps2_mouse_mut(|mouse| mouse.pop_delta());
        let clicks = ps2::with_ps2_mouse_mut(|mouse| mouse.pop_clicks());

        camera.yaw += dx as f32 * MOUSE_SENSITIVITY;
        camera.pitch -= dy as f32 * MOUSE_SENSITIVITY;
        camera.pitch = camera.pitch.clamp(-PI / 2.0 + 0.01, PI / 2.0 - 0.01);

        keyboard_events.clear();
        ps2::with_ps2_keyboard_mut(|keyboard| {
            while let Some(event) = keyboard.pop_event() {
                keyboard_events.push(event);
            }
        });

        for event in keyboard_events.iter().copied() {
            if !console.is_active() && matches!(event.code, KeyCode::T | KeyCode::Return) {
                console.set_active(true);
                continue;
            }

            if event.code == KeyCode::Oem8 {
                if console.is_visible() && logger::is_visible() {
                    console.set_visible(false);
                    logger::set_visible(false);
                } else {
                    console.set_visible(true);
                    logger::set_visible(true);
                }
                continue;
            }

            if let Some(command) = console.process_event(event) {
                let trimmed = command.trim();
                if !trimmed.is_empty() {
                    log::info!("[console] {}", trimmed);
                }
            }
        }

        // keyboard state for movement
        let key_states = ps2::with_ps2_keyboard(|keyboard| keyboard.key_states);
        if !console.is_active() {
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

        // targeted block
        let targeted_block = {
            let world = game::world::WORLD.lock();
            camera.looking_at_solid_block(&world, 5.0)
        };
        if clicks.left {
            if let Some((block_pos, ref _face)) = targeted_block {
                let mut world = game::world::WORLD.lock();
                world[block_pos.x][block_pos.y][block_pos.z] = false;
                // have to rebuild world mesh
                mesh = game::world::get_world_mesh(&world);
            }
        }
        if clicks.right {
            if let Some((block_pos, ref face)) = targeted_block {
                let offset = face.offset();
                let new = block_pos.wrapping_add_signed(offset);
                if world::is_in_bounds(new) {
                    let mut world = game::world::WORLD.lock();
                    world[new.x][new.y][new.z] = true;
                    mesh = game::world::get_world_mesh(&world);
                }
            }
        }

        kernel::rendering::with_global_renderer_mut(|renderer| {
            {
                let mut frame = renderer.frame();
                frame.clear_color(Color::LIGHT_GRAY);
                frame.clear_depth();

                screen.render(&mut frame, &camera, &mesh);
                if let Some((block_pos, _face)) = targeted_block {
                    screen.draw_block_outline(&mut frame, &camera, block_pos, Color::BLACK);
                }
                screen.draw_crosshair(&mut frame);
                logger::LOGGER.render(&mut frame);
                console.render(&mut frame);
            }
            renderer.flush();
        });
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    logger::set_visible(true);
    log::error!("{}", info);
    // log auto flush will fail because interrupts are disabled,
    // have to explicitly flush
    log::logger().flush();
    kernel::hlt_loop();
}
