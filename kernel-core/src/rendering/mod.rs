use bootloader_api::info::{FrameBuffer, FrameBufferInfo, PixelFormat};
use core::{fmt, ptr};
use embedded_graphics::{
    Pixel,
    draw_target::DrawTarget,
    geometry::{Dimensions, OriginDimensions, Size},
    pixelcolor::{PixelColor, raw::RawU24},
    primitives::Rectangle,
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
    /// intensity between 0 and 255, 0 is black
    pub fn with_intensity(&self, intensity: u8) -> Color {
        Color {
            red: ((self.red as u16 * intensity as u16) / 255) as u8,
            green: ((self.green as u16 * intensity as u16) / 255) as u8,
            blue: ((self.blue as u16 * intensity as u16) / 255) as u8,
        }
    }
    /// intensity between 0.0 and 1.0, 0.0 is black
    pub fn with_intensity_f(&self, intensity: f32) -> Color {
        Color {
            red: (self.red as f32 * intensity) as u8,
            green: (self.green as f32 * intensity) as u8,
            blue: (self.blue as f32 * intensity) as u8,
        }
    }
    pub fn to_gray(&self) -> u8 {
        self.red / 3 + self.green / 3 + self.blue / 3
    }
    pub fn parse_hex(hex: &str) -> Option<Color> {
        let hex = hex.strip_prefix('#')?;
        let num = u32::from_str_radix(hex, 16).ok()?;
        // RGB format in bits: 00000000 RRRRRRRR GGGGGGGG BBBBBBBB
        Some(Color {
            red: (num >> 16) as u8,
            green: (num >> 8) as u8,
            blue: num as u8,
        })
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

    fn build_48_byte_pattern(&self, format: PixelFormat, bpp: usize) -> [u8; 48] {
        let mut pattern = [0u8; 48];
        for chunk in pattern.chunks_exact_mut(bpp) {
            self.write_to(chunk, format);
        }
        pattern
    }

    pub fn fg(&self) -> Fg<'_> {
        Fg(self)
    }
}

pub struct Fg<'a>(&'a Color);
impl fmt::Display for Fg<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let c = self.0;
        // custom control protocol
        // format:
        // ESC ~ <COMMAND> BEL
        //
        // set foreground format
        // ESC ~ FG#RRGGBB BEL
        write!(f, "\x1B~FG#{:02X}{:02X}{:02X}\x07", c.red, c.green, c.blue)
    }
}

impl PixelColor for Color {
    type Raw = RawU24;
}

unsafe fn fill_slice_48(dst: *mut u8, len: usize, pattern: &[u8; 48]) {
    if len == 0 {
        return;
    }

    let num_chunks = len / 48;
    let remainder = len % 48;

    for chunk in 0..num_chunks {
        unsafe { ptr::copy_nonoverlapping(pattern.as_ptr(), dst.add(chunk * 48), 48) };
    }
    unsafe { ptr::copy_nonoverlapping(pattern.as_ptr(), dst.add(num_chunks * 48), remainder) };
}

pub struct Renderer<'f> {
    framebuffer: &'f mut [u8],
    pub info: FrameBufferInfo,
}

impl<'f> Renderer<'f> {
    pub fn from_framebuffer(framebuffer: &'f mut FrameBuffer) -> Self {
        Renderer {
            info: framebuffer.info(),
            framebuffer: framebuffer.buffer_mut(),
        }
    }

    pub fn new(framebuffer: &'f mut [u8], info: FrameBufferInfo) -> Self {
        Renderer { framebuffer, info }
    }
    pub fn buffer_mut(&mut self) -> &mut [u8] {
        self.framebuffer
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

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        let area = area.intersection(&self.bounding_box());
        if area.bottom_right().is_none() {
            return Ok(());
        }

        let bpp = self.info.bytes_per_pixel;
        let width = area.size.width as usize;
        let row_bytes = width * bpp;
        if row_bytes == 0 {
            return Ok(());
        }

        let format = self.info.pixel_format;
        let pattern = color.build_48_byte_pattern(format, self.info.bytes_per_pixel);

        for y in area.rows() {
            let y = y as usize;
            let row_start = {
                let line_offset = y * self.info.stride;
                let pixel_offset = line_offset + area.top_left.x as usize;
                pixel_offset * self.info.bytes_per_pixel
            };
            unsafe {
                fill_slice_48(
                    self.framebuffer.as_mut_ptr().add(row_start),
                    row_bytes,
                    &pattern,
                );
            }
        }

        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        debug_assert!(self.framebuffer.len() % self.info.bytes_per_pixel == 0);
        let len = self.framebuffer.len();
        if len == 0 {
            return Ok(());
        }
        let pattern =
            color.build_48_byte_pattern(self.info.pixel_format, self.info.bytes_per_pixel);
        unsafe {
            fill_slice_48(self.framebuffer.as_mut_ptr(), len, &pattern);
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
    fn color_with_intensity() {
        // tests for overflow
        let color = Color::WHITE;
        let color_with_intensity = color.with_intensity(255);
        assert_eq!(color_with_intensity.red, 255);
        assert_eq!(color_with_intensity.green, 255);
        assert_eq!(color_with_intensity.blue, 255);

        let color_with_intensity_f = color.with_intensity_f(1.0);
        assert_eq!(color_with_intensity_f.red, 255);
        assert_eq!(color_with_intensity_f.green, 255);
        assert_eq!(color_with_intensity_f.blue, 255);

        let color_with_intensity_f_half = color.with_intensity_f(0.5);
        assert_eq!(color_with_intensity_f_half.red, 127);
        assert_eq!(color_with_intensity_f_half.green, 127);
        assert_eq!(color_with_intensity_f_half.blue, 127);
    }

    #[test]
    fn clear_rgb() {
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

        let mut renderer = Renderer::from_framebuffer(&mut fb);
        renderer.clear(Color::WHITE).unwrap();
        assert!(buffer.iter().all(|&b| b == 255));

        renderer.clear(Color::RED).unwrap();
        for chunk in buffer.chunks(3) {
            assert_eq!(chunk, &[0xFF, 0x00, 0x00]);
        }
    }

    #[test]
    fn build_48_byte_pattern_rgb() {
        let color = Color {
            red: 0x12,
            green: 0x34,
            blue: 0x56,
        };
        let pattern = color.build_48_byte_pattern(PixelFormat::Rgb, 4);
        for chunk in pattern.chunks(4) {
            assert_eq!(chunk, &[0x12, 0x34, 0x56, 0x00]);
        }
    }

    #[test]
    fn fill_slice_48_exact_48_bytes() {
        let pattern = [0xAAu8; 48];
        let mut buffer = [0u8; 48];
        unsafe {
            fill_slice_48(buffer.as_mut_ptr(), 48, &pattern);
        }
        assert!(buffer.iter().all(|&b| b == 0xAA));
    }

    #[test]
    fn fill_slice_48_multiple_of_48() {
        let pattern = [0xBBu8; 48];
        let mut buffer = [0u8; 144];
        unsafe {
            fill_slice_48(buffer.as_mut_ptr(), 144, &pattern);
        }
        assert!(buffer.iter().all(|&b| b == 0xBB));
    }

    #[test]
    fn fill_slice_48_remainder() {
        let pattern = [0xCCu8; 48];
        let mut buffer = [0u8; 50];
        unsafe {
            fill_slice_48(buffer.as_mut_ptr(), 50, &pattern);
        }
        assert!(buffer.iter().all(|&b| b == 0xCC));
    }

    #[test]
    fn fill_slice_48_zero_len() {
        let pattern = [0xDDu8; 48];
        let mut buffer = [0u8; 48];
        buffer[0] = 0x11;
        unsafe {
            fill_slice_48(buffer.as_mut_ptr(), 0, &pattern);
        }
        assert_eq!(buffer[0], 0x11);
    }

    #[test]
    fn fill_slice_48_pattern_repeats() {
        let color = Color {
            red: 1,
            green: 2,
            blue: 3,
        };
        let pattern = color.build_48_byte_pattern(PixelFormat::Rgb, 4);
        let mut buffer = [0u8; 67 * 4];

        unsafe {
            fill_slice_48(buffer.as_mut_ptr(), 67 * 4, &pattern);
        }

        for chunk in buffer.chunks(4) {
            assert_eq!(chunk, &[1, 2, 3, 0]);
        }
    }
}
