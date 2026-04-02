use bootloader_api::info::PixelFormat;
use core::fmt;
use kernel_core::rendering::Color;
use kernel_core::rendering::Rectangle;
use kernel_core::rendering::Renderer;

pub struct TextBox {
    inner: kernel_core::rendering::TextBox,
}

impl TextBox {
    pub fn new(bounding_box: Rectangle, pixel_format: PixelFormat, bytes_per_pixel: usize) -> Self {
        Self {
            inner: kernel_core::rendering::TextBox::new(
                bounding_box,
                pixel_format,
                bytes_per_pixel,
            ),
        }
    }
    pub fn render(&mut self) {
        self.inner.render();
    }
    pub fn flush(&mut self, renderer: &mut Renderer) {
        self.inner.flush(renderer);
    }
    pub fn get_current_text_color(&mut self) -> Color {
        self.inner.get_current_text_color()
    }
}

impl fmt::Write for TextBox {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        // only pushing bytes to text box, not rendering
        for b in s.bytes() {
            self.inner.push_byte(b);
        }
        Ok(())
    }
}
