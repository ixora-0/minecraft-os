use core::{ops::Range, ptr};

use bootloader_api::info::{FrameBuffer, FrameBufferInfo};
use glam::{IVec2, USizeVec2, Vec3};

use crate::game::Triangle;

use super::Color;

pub struct Pixel {
    pub coord: IVec2,
    pub color: Color,
}
pub struct Rectangle {
    pub top_left: IVec2,
    pub size: USizeVec2,
}
impl Rectangle {
    pub fn bottom_right(&self) -> IVec2 {
        self.top_left + self.size.as_ivec2()
    }
    pub fn intersection(&self, other: &Self) -> Option<Self> {
        let intersection_top_left = self.top_left.max(other.top_left);
        let intersection_bottom_right = self.bottom_right().min(other.bottom_right());

        if intersection_top_left.x < intersection_bottom_right.x
            && intersection_top_left.y < intersection_bottom_right.y
        {
            Some(Self {
                top_left: intersection_top_left,
                size: (intersection_bottom_right - intersection_top_left).as_usizevec2(),
            })
        } else {
            None
        }
    }
    pub fn rows(&self) -> Range<i32> {
        self.top_left.y..self.bottom_right().y
    }
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
    pub fn bounding_box(&self) -> Rectangle {
        Rectangle {
            top_left: IVec2::ZERO,
            size: USizeVec2::new(self.info.width, self.info.height),
        }
    }

    fn render_pixel(&mut self, pixel: Pixel) {
        let (width, height) = (self.info.width, self.info.height);
        let x = pixel.coord.x as usize;
        let y = pixel.coord.y as usize;
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
        pixel.color.write_to(chunk, self.info.pixel_format);
    }

    pub fn draw_iter<I>(&mut self, pixels: I)
    where
        I: IntoIterator<Item = Pixel>,
    {
        for pixel in pixels.into_iter() {
            self.render_pixel(pixel);
        }
    }

    pub fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I)
    where
        I: IntoIterator<Item = Color>,
    {
        let mut colors = colors.into_iter();

        let Some(area) = area.intersection(&self.bounding_box()) else {
            return;
        };

        let bpp = self.info.bytes_per_pixel;
        let width = area.size.x;
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
    }

    pub fn fill_solid(&mut self, area: &Rectangle, color: Color) {
        let Some(area) = area.intersection(&self.bounding_box()) else {
            return;
        };

        let bpp = self.info.bytes_per_pixel;
        let width = area.size.x;
        let row_bytes = width * bpp;
        if row_bytes == 0 {
            return;
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
    }

    pub fn clear(&mut self, color: Color) {
        debug_assert!(self.framebuffer.len() % self.info.bytes_per_pixel == 0);
        let len = self.framebuffer.len();
        if len == 0 {
            return;
        }
        let pattern =
            color.build_48_byte_pattern(self.info.pixel_format, self.info.bytes_per_pixel);
        unsafe {
            fill_slice_48(self.framebuffer.as_mut_ptr(), len, &pattern);
        }
    }

    pub fn draw_line(&mut self, start: IVec2, end: IVec2, color: Color, thickness: f32) {
        let _ = start;
        let _ = end;
        let _ = color;
        let _ = thickness;
        todo!()
    }
}

pub struct Renderer3d<'f> {
    buffer: &'f mut [u8],
    depth_buffer: &'f mut [f32],
    pub info: FrameBufferInfo,
}
impl<'f> Renderer3d<'f> {
    pub fn new(buffer: &'f mut [u8], depth_buffer: &'f mut [f32], info: FrameBufferInfo) -> Self {
        Self {
            buffer,
            depth_buffer,
            info,
        }
    }
    pub fn draw_line(&mut self, start: Vec3, end: Vec3, color: Color, thickness: f32) {
        if thickness <= 1.5 {
            self.fill_line_thin(start, end, color);
        } else {
            self.fill_line_thick(start, end, color, thickness);
        }
    }

    fn fill_line_thin(&mut self, start: Vec3, end: Vec3, color: Color) {
        let _ = start;
        let _ = end;
        let _ = color;
        todo!()
    }

    fn fill_line_thick(&mut self, start: Vec3, end: Vec3, color: Color, thickness: f32) {
        let _ = start;
        let _ = end;
        let _ = color;
        let _ = thickness;
        todo!()
    }

    pub fn fill_triangle(&mut self, triangle: &Triangle, color: Color) {
        let _ = triangle;
        let _ = color;
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use bootloader_api::info::PixelFormat;

    use super::*;

    #[test]
    fn clear_rgb() {
        use bootloader_api::info::{FrameBuffer, FrameBufferInfo, PixelFormat};
        const INFO: FrameBufferInfo = FrameBufferInfo {
            byte_len: 16 * 16 * 3,
            width: 16,
            height: 16,
            pixel_format: PixelFormat::Rgb,
            bytes_per_pixel: 3,
            stride: 16,
        };

        let mut buffer = [0u8; INFO.byte_len];
        let buffer_addr = buffer.as_mut_ptr() as u64;
        let mut fb = unsafe { FrameBuffer::new(buffer_addr, INFO) };

        let mut renderer = Renderer::from_framebuffer(&mut fb);
        renderer.clear(Color::WHITE);
        assert!(buffer.iter().all(|&b| b == 255));

        renderer.clear(Color::RED);
        for chunk in buffer.chunks(3) {
            assert_eq!(chunk, &[0xFF, 0x00, 0x00]);
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
