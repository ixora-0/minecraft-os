use bootloader_api::info::{FrameBuffer, FrameBufferInfo, PixelFormat};
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{Dimensions, OriginDimensions, Size},
    pixelcolor::{raw::RawU24, PixelColor},
    primitives::Rectangle,
    Pixel,
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
    pub const RED: Color = Color {
        red: 255,
        green: 0,
        blue: 0,
    };
    pub const YELLOW: Color = Color {
        red: 255,
        green: 255,
        blue: 0,
    };
    pub const GREEN: Color = Color {
        red: 0,
        green: 255,
        blue: 0,
    };
    pub const CYAN: Color = Color {
        red: 0,
        green: 255,
        blue: 255,
    };
    pub const LIGHT_GRAY: Color = Color {
        red: 180,
        green: 180,
        blue: 180,
    };
    pub const DARK_GRAY: Color = Color {
        red: 120,
        green: 120,
        blue: 120,
    };
    pub fn with_intensity(&self, intensity: u8) -> Color {
        Color {
            red: ((self.red as u16 * intensity as u16) / 255) as u8,
            green: ((self.green as u16 * intensity as u16) / 255) as u8,
            blue: ((self.blue as u16 * intensity as u16) / 255) as u8,
        }
    }
    pub fn to_gray(&self) -> u8 {
        self.red / 3 + self.green / 3 + self.blue / 3
    }
    pub fn write_to(&self, chunk: &mut [u8], format: PixelFormat) {
        match format {
            PixelFormat::Rgb => {
                chunk[0] = self.red;
                chunk[1] = self.green;
                chunk[2] = self.blue;
            }
            PixelFormat::Bgr => {
                chunk[0] = self.blue;
                chunk[1] = self.green;
                chunk[2] = self.red;
            }
            PixelFormat::U8 => {
                chunk[0] = self.to_gray();
            }
            other => panic!("unknown pixel format {other:?}"),
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
        let (width, height) = (self.info.width, self.info.height);
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

        let chunk = &mut self.framebuffer[byte_offset..];
        color.write_to(chunk, self.info.pixel_format);
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

    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        let mut colors = colors.into_iter();

        let area = area.intersection(&self.bounding_box());
        if area.bottom_right().is_none() {
            return Ok(());
        };

        let bpp = self.info.bytes_per_pixel;
        let width = area.size.width as usize;
        let format = self.info.pixel_format;

        for y in area.rows() {
            let y = y as usize;
            let row_start = {
                // use stride to calculate pixel offset of target line
                let line_offset = y * self.info.stride;
                // add x position to get the absolute pixel offset in buffer
                let pixel_offset = line_offset + area.top_left.x as usize;
                // convert to byte offset
                pixel_offset * self.info.bytes_per_pixel
            };

            let row = &mut self.framebuffer[row_start..row_start + width * bpp];

            for chunk in row.chunks_mut(bpp) {
                if let Some(color) = colors.next() {
                    color.write_to(chunk, format);
                }
            }
        }

        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        debug_assert!(self.framebuffer.len() % self.info.bytes_per_pixel == 0);
        for chunk in self.framebuffer.chunks_mut(self.info.bytes_per_pixel) {
            color.write_to(chunk, self.info.pixel_format);
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
