use conquer_once::spin::OnceCell;
use core::fmt::Write;
use spin::Mutex;

use crate::rendering::TextBox;
use crate::serial_println;
use embedded_graphics::geometry::{Point, Size};
use embedded_graphics::primitives::Rectangle;

pub static LOGGER: OnceCell<TextBoxLogger> = OnceCell::uninit();

pub struct TextBoxLogger {
    text_box: Mutex<TextBox>,
}

impl TextBoxLogger {
    fn new() -> Self {
        let text_box = TextBox::new(Rectangle {
            top_left: Point::new(100, 100),
            size: Size::new(400, 300),
        });
        TextBoxLogger {
            text_box: Mutex::new(text_box),
        }
    }
}

impl log::Log for TextBoxLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        serial_println!("{:5}: {}", record.level(), record.args());
        let mut text_box = self.text_box.lock();
        writeln!(text_box, "{:5}: {}", record.level(), record.args()).unwrap();
    }

    fn flush(&self) {}
}

pub fn init_logger() {
    let logger = LOGGER.get_or_init(move || TextBoxLogger::new());
    log::set_logger(logger).expect("Logger already set");
    log::set_max_level(log::LevelFilter::Trace);
}
