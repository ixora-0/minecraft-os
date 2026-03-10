use bootloader_api::info::{FrameBuffer, FrameBufferInfo, PixelFormat};
use embedded_graphics::{
    Pixel,
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Size},
    pixelcolor::{PixelColor, raw::RawU24},
};

pub mod text_box;
pub use text_box::{TextBox, TextBoxConfig};

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

pub struct Renderer<'f> {
    framebuffer: &'f mut [u8],
    info: FrameBufferInfo,
}

impl<'f> Renderer<'f> {
    pub fn new(framebuffer: &'f mut FrameBuffer) -> Self {
        Renderer {
            info: framebuffer.info(),
            framebuffer: framebuffer.buffer_mut(),
        }
    }

    fn render_pixel(&mut self, x: usize, y: usize, color: Color) {
        // ignore any out of bounds pixels
        let (width, height) = { (self.info.width, self.info.height) };
        if !(0..width).contains(&x) || !(0..height).contains(&y) {
            return;
        }

        // calculate offset to first byte of pixel
        let byte_offset = {
            // use stride to calculate pixel offset of target line
            let line_offset = y * self.info.stride;
            // add x position to get the absolute pixel offset in buffer
            let pixel_offset = line_offset + x;
            // convert to byte offset
            pixel_offset * self.info.bytes_per_pixel
        };

        let pixel_buffer = &mut self.framebuffer[byte_offset..];
        match self.info.pixel_format {
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

impl<'f> DrawTarget for Renderer<'f> {
    type Color = Color;
    /// Drawing operations can never fail.
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coordinates, color) in pixels.into_iter() {
            let (x, y) = {
                let c: (i32, i32) = coordinates.into();
                (c.0 as usize, c.1 as usize)
            };
            self.render_pixel(x, y, color);
        }

        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        debug_assert!(self.framebuffer.len() % self.info.bytes_per_pixel == 0);
        match self.info.pixel_format {
            PixelFormat::Rgb => {
                for chunk in self.framebuffer.chunks_mut(self.info.bytes_per_pixel) {
                    chunk[0] = color.red;
                    chunk[1] = color.green;
                    chunk[2] = color.blue;
                }
            }
            PixelFormat::Bgr => {
                for chunk in self.framebuffer.chunks_mut(self.info.bytes_per_pixel) {
                    chunk[0] = color.blue;
                    chunk[1] = color.green;
                    chunk[2] = color.red;
                }
            }
            PixelFormat::U8 => {
                let gray = color.red / 3 + color.green / 3 + color.blue / 3;
                self.framebuffer.fill(gray);
            }
            other => panic!("unknown pixel format {other:?}"),
        }

        Ok(())
    }
}

impl<'f> OriginDimensions for Renderer<'f> {
    fn size(&self) -> Size {
        Size::new(self.info.width as u32, self.info.height as u32)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_color_with_intensity() {
        // tests for overflow
        let color = Color::WHITE;
        let color_with_intensity = color.with_intensity(255);
        assert_eq!(color_with_intensity.red, 255);
        assert_eq!(color_with_intensity.green, 255);
        assert_eq!(color_with_intensity.blue, 255);
    }

    #[test]
    fn test_clear_rgb() {
        use bootloader_api::info::{FrameBuffer, FrameBufferInfo, PixelFormat};
        const INFO: FrameBufferInfo = FrameBufferInfo {
            byte_len: 16 * 16 * 3,
            width: 16,
            height: 16,
            pixel_format: PixelFormat::Rgb,
            bytes_per_pixel: 3,
            stride: 16 * 3,
        };

        let mut buffer = [0u8; INFO.byte_len];
        let buffer_addr = buffer.as_mut_ptr() as u64;
        let mut fb = unsafe { FrameBuffer::new(buffer_addr, INFO) };

        let mut renderer = Renderer::new(&mut fb);
        renderer.clear(Color::WHITE).unwrap();

        assert!(buffer.iter().all(|&b| b == 255));
    }
}
