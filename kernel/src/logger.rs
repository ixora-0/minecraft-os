use crate::rendering;
use crate::serial_println;
use bootloader_api::info::FrameBufferInfo;
use core::{
    fmt::Write,
    slice,
    sync::atomic::{AtomicBool, Ordering},
};
use glam::{IVec2, USizeVec2};
use kernel_core::rendering::{Color, Frame, Rectangle, Renderer, TextBox};
use log::Level;
use spin::Mutex;
use x86_64::instructions::interrupts;

pub static LOGGER: TextBoxLogger = TextBoxLogger::default();

pub struct TextBoxLogger {
    // mutex to allow interior mutability
    // so enable_rendering doesn't need &mut self
    // which avoids static mut
    text_box: Mutex<Option<TextBox>>,
    backup_surface: Mutex<Option<BackupSurface>>,
    needs_flush: AtomicBool,
}

#[derive(Clone, Copy)]
struct BackupSurface {
    ptr: *mut u8,
    info: FrameBufferInfo,
}

unsafe impl Send for BackupSurface {}
unsafe impl Sync for BackupSurface {}

impl TextBoxLogger {
    pub const fn default() -> Self {
        TextBoxLogger {
            text_box: Mutex::new(None),
            backup_surface: Mutex::new(None),
            needs_flush: AtomicBool::new(false),
        }
    }
    pub fn enable_rendering(&self) {
        let (text_box, backup) = rendering::with_global_renderer_mut(|renderer| {
            let (width, height) = (700, 350);
            let info = renderer.info();
            let ptr = renderer.framebuffer_ptr();
            let text_box = TextBox::new(Rectangle {
                top_left: IVec2::new(10, info.height as i32 - height as i32 - 10),
                size: USizeVec2::new(width, height),
            });
            (text_box, BackupSurface { ptr, info })
        });
        let mut text_box_ref = self.text_box.lock();
        *text_box_ref = Some(text_box);
        *self.backup_surface.lock() = Some(backup);
        self.needs_flush.store(true, Ordering::Relaxed);
    }
}

impl log::Log for TextBoxLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        serial_println!("{:5}: {}", record.level(), record.args());

        let mut should_flush = self.needs_flush.load(Ordering::Relaxed);
        if let Some(mut guard) = self.text_box.try_lock() {
            if let Some(text_box) = guard.as_mut() {
                let prev_color = text_box.get_current_text_color();
                let color = match record.level() {
                    Level::Error => Color::RED,
                    Level::Warn => Color::YELLOW,
                    Level::Info => Color::WHITE,
                    Level::Debug => Color::LIGHT_GRAY,
                    Level::Trace => Color::DARK_GRAY,
                };
                writeln!(
                    text_box,
                    "{}{:5}: {}{}",
                    color.fg(),
                    record.level(),
                    record.args(),
                    prev_color.fg()
                )
                .unwrap();
                should_flush = true;
            }
        }

        // writeln! only push the bytes to the text box
        // needs to render and flush to global framebuffer
        if should_flush {
            self.needs_flush.store(true, Ordering::Relaxed);
            // interrupts are disabled usually when logging from an interrupt or panic handler
            // we should avoid doing heavy work in these cases
            if interrupts::are_enabled() {
                self.flush();
            }
        }
    }

    fn flush(&self) {
        if !self.needs_flush.load(Ordering::Relaxed) {
            return;
        }

        let Some(mut guard) = self.text_box.try_lock() else {
            return;
        };

        let Some(text_box) = guard.as_mut() else {
            // textbox rendering disabled
            self.needs_flush.store(false, Ordering::Relaxed);
            return;
        };

        if let Some(rendered) = rendering::try_with_global_renderer_mut(|renderer| {
            let mut frame = renderer.frame();
            let rendered = render_text_box(text_box, &mut frame);
            if rendered {
                renderer.flush();
            }
            rendered
        }) {
            if rendered {
                self.needs_flush.store(false, Ordering::Relaxed);
            }
            return;
        }

        // trying to render log to screen but the global renderer is locked
        // this could happen when while rendering something else, a interrupt occurs,
        // and the handler tries to log. in this case, we do not flush the rendererd text,
        // and hope that the log will be shown on the screen the next time
        // global renderer render something or a new log is written
        if interrupts::are_enabled() {
            self.needs_flush.store(true, Ordering::Relaxed);
            return;
        }

        // global renderer is locked, and the request isn't from an interrupt
        // which probably means a panic occured while the global renderer was locked
        // and the panic handler wants to log something (ie. error log)
        // so we render directly to the backup surface
        if let Some(surface) = *self.backup_surface.lock() {
            unsafe {
                render_direct(text_box, surface);
            }
            self.needs_flush.store(false, Ordering::Relaxed);
        }
    }
}

pub fn init_logger() {
    log::set_logger(&LOGGER).expect("Logger already set");
    log::set_max_level(log::LevelFilter::Trace);
}
pub fn enable_rendering() {
    LOGGER.enable_rendering();
}

impl TextBoxLogger {
    pub fn render(&self, frame: &mut Frame) {
        let mut text_box_guard = self.text_box.lock();
        if let Some(text_box) = text_box_guard.as_mut() {
            render_text_box(text_box, frame);
        }
        self.needs_flush.store(false, Ordering::Relaxed);
    }
}

fn render_text_box(text_box: &mut TextBox, frame: &mut Frame) -> bool {
    let mut renderer = frame.renderer2d();
    text_box.render(&mut renderer);
    true
}

unsafe fn render_direct(text_box: &mut TextBox, surface: BackupSurface) {
    let buffer = unsafe { slice::from_raw_parts_mut(surface.ptr, surface.info.byte_len) };
    let mut renderer = Renderer::new(buffer, surface.info);
    text_box.render(&mut renderer);
}
