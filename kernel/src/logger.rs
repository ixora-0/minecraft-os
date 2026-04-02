use core::fmt::Write;
use glam::{IVec2, USizeVec2};
use log::Level;
use spin::Mutex;
use x86_64::instructions::interrupts;

use crate::rendering::{self, TextBox};
use crate::serial_println;
use kernel_core::rendering::{Color, Rectangle};

pub static LOGGER: TextBoxLogger = TextBoxLogger::default();

pub struct TextBoxLogger {
    // mutex to allow interior mutability
    // so enable_rendering doesn't need &mut self
    // which avoids static mut
    text_box: Mutex<Option<TextBox>>,
}

impl TextBoxLogger {
    pub const fn default() -> Self {
        TextBoxLogger {
            text_box: Mutex::new(None),
        }
    }
    pub fn enable_rendering(&self) {
        let text_box = rendering::with_global_renderer(|renderer| {
            let (width, height) = (700, 350);
            TextBox::new(
                Rectangle {
                    top_left: IVec2::new(10, renderer.info.height as i32 - height as i32 - 10),
                    size: USizeVec2::new(width, height),
                },
                renderer.info.pixel_format,
                renderer.info.bytes_per_pixel,
            )
        });
        let mut text_box_ref = self.text_box.lock();
        *text_box_ref = Some(text_box);
    }
}

impl log::Log for TextBoxLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        serial_println!("{:5}: {}", record.level(), record.args());

        let mut need_flush = false;
        interrupts::without_interrupts(|| {
            if let Some(text_box) = self.text_box.lock().as_mut() {
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
                need_flush = true;
            }
        });

        if need_flush {
            self.flush();
        }
    }

    fn flush(&self) {
        if let Some(text_box) = self.text_box.lock().as_mut() {
            text_box.render();
            rendering::with_global_renderer_mut(|renderer| {
                text_box.flush(renderer);
            });
        }
    }
}

pub fn init_logger() {
    log::set_logger(&LOGGER).expect("Logger already set");
    log::set_max_level(log::LevelFilter::Debug);
}
pub fn enable_rendering() {
    LOGGER.enable_rendering();
}
