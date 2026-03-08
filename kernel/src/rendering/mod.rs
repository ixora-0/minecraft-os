use core::fmt;

use bootloader_api::info::{FrameBuffer, FrameBufferInfo, PixelFormat};
use embedded_graphics::{
    Pixel,
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Size},
    pixelcolor::{PixelColor, raw::RawU24},
};
use spin::{Mutex, Once};

pub mod text_box;

pub static FRAMEBUFFER: Mutex<Once<&'static mut [u8]>> = Mutex::new(Once::new());
pub static FRAMEBUFFER_INFO: Once<FrameBufferInfo> = Once::new();
const EXPECT_MSG_FRAMEBUFFER_NOT_INITIALIZED: &str =
    "Framebuffer not initialized. Probably haven't run init_framebuffer()";

pub fn init_framebuffer(framebuffer: &'static mut FrameBuffer) {
    let frame_buffer_info = framebuffer.info().clone();
    FRAMEBUFFER_INFO.call_once(|| frame_buffer_info);

    // get the framebuffer's mutable raw byte slice
    let raw_framebuffer = framebuffer.buffer_mut();
    FRAMEBUFFER.lock().call_once(|| raw_framebuffer);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}
impl Color {
    pub const BLACK: Color = Color {
        red: 0,
        green: 0,
        blue: 0,
    };
    pub const WHITE: Color = Color {
        red: 255,
        green: 255,
        blue: 255,
    };
    pub fn with_intensity(&self, intensity: u8) -> Color {
        Color {
            red: ((self.red as u16 * intensity as u16) / 255) as u8,
            green: ((self.green as u16 * intensity as u16) / 255) as u8,
            blue: ((self.blue as u16 * intensity as u16) / 255) as u8,
        }
    }
}
impl PixelColor for Color {
    type Raw = RawU24;
}

pub struct Renderer {}

impl Renderer {
    pub fn new() -> Renderer {
        Renderer {}
    }

    fn render_pixel(&self, framebuffer: &mut [u8], x: usize, y: usize, color: Color) {
        let info = FRAMEBUFFER_INFO
            .get()
            .expect(EXPECT_MSG_FRAMEBUFFER_NOT_INITIALIZED);
        // ignore any out of bounds pixels
        let (width, height) = { (info.width, info.height) };
        if !(0..width).contains(&x) || !(0..height).contains(&y) {
            return;
        }

        // calculate offset to first byte of pixel
        let byte_offset = {
            // use stride to calculate pixel offset of target line
            let line_offset = y * info.stride;
            // add x position to get the absolute pixel offset in buffer
            let pixel_offset = line_offset + x;
            // convert to byte offset
            pixel_offset * info.bytes_per_pixel
        };

        let pixel_buffer = &mut framebuffer[byte_offset..];
        match info.pixel_format {
            PixelFormat::Rgb => {
                pixel_buffer[0] = color.red;
                pixel_buffer[1] = color.green;
                pixel_buffer[2] = color.blue;
            }
            PixelFormat::Bgr => {
                pixel_buffer[0] = color.blue;
                pixel_buffer[1] = color.green;
                pixel_buffer[2] = color.red;
            }
            PixelFormat::U8 => {
                // use a simple average-based grayscale transform
                let gray = color.red / 3 + color.green / 3 + color.blue / 3;
                pixel_buffer[0] = gray;
            }
            other => panic!("unknown pixel format {other:?}"),
        }
    }
}

impl DrawTarget for Renderer {
    type Color = Color;
    /// Drawing operations can never fail.
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        let mut fb_once = FRAMEBUFFER.lock();
        let framebuffer = fb_once
            .get_mut()
            .expect(EXPECT_MSG_FRAMEBUFFER_NOT_INITIALIZED);
        for Pixel(coordinates, color) in pixels.into_iter() {
            let (x, y) = {
                let c: (i32, i32) = coordinates.into();
                (c.0 as usize, c.1 as usize)
            };
            self.render_pixel(framebuffer, x, y, color);
        }

        Ok(())
    }
}

impl OriginDimensions for Renderer {
    fn size(&self) -> Size {
        let info = FRAMEBUFFER_INFO
            .get()
            .expect(EXPECT_MSG_FRAMEBUFFER_NOT_INITIALIZED);

        Size::new(info.width as u32, info.height as u32)
    }
}
