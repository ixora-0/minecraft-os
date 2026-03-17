use crate::rendering::{EXPECT_MSG_FRAMEBUFFER_NOT_INITIALIZED, GLOBAL_RENDERER};
use core::fmt;
use embedded_graphics::primitives::Rectangle;
use kernel_core::rendering::Color;

pub struct TextBox {
    inner: kernel_core::rendering::TextBox,
}

impl TextBox {
    pub fn new(bounding_box: Rectangle) -> Self {
        Self {
            inner: kernel_core::rendering::TextBox::new(bounding_box),
        }
    }
    pub fn get_foreground_color(&mut self) -> Color {
        self.inner.get_foreground_color()
    }
    pub fn set_foreground_color(&mut self, color: Color) {
        self.inner.set_foreground_color(color);
    }
}

impl fmt::Write for TextBox {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut renderer_guard = GLOBAL_RENDERER.lock();
        let mut renderer = renderer_guard
            .get_mut()
            .expect(EXPECT_MSG_FRAMEBUFFER_NOT_INITIALIZED);
        for c in s.chars() {
            self.inner.write_char(c, &mut renderer);
        }
        Ok(())
    }
}
