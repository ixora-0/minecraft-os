use core::fmt::Write;
use spin::Mutex;
use x86_64::instructions::interrupts;

use crate::rendering::TextBox;
use crate::serial_println;
use embedded_graphics::geometry::{Point, Size};
use embedded_graphics::primitives::Rectangle;

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
        let text_box = TextBox::new(Rectangle {
            top_left: Point::new(50, 10),
            size: Size::new(1000, 700),
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
        interrupts::without_interrupts(|| {
            if let Some(text_box) = self.text_box.lock().as_mut() {
                writeln!(text_box, "{:5}: {}", record.level(), record.args()).unwrap();
            }
        });
    }

    fn flush(&self) {}
}

pub fn init_logger() {
    log::set_logger(&LOGGER).expect("Logger already set");
    log::set_max_level(log::LevelFilter::Trace);
}
pub fn enable_rendering() {
    LOGGER.enable_rendering();
}
