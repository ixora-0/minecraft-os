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

    /// Fills a ~1px thick line segment with the given color using bresenham's line algorithm.
    fn fill_line_thin(&mut self, a: Vec3, b: Vec3, color: Color) {
        let (w, h) = (self.info.width, self.info.height);

        // precompute color bytes once, avoid format match in inner loop
        let bpp = self.info.bytes_per_pixel;
        let mut color_bytes = [0u8; 8];
        color.write_to(&mut color_bytes, self.info.pixel_format);
        let color_bytes = &color_bytes[..bpp];

        let mut x = a.x as i32;
        let mut y = a.y as i32;
        let end_x = b.x as i32;
        let end_y = b.y as i32;

        let dx = (end_x - x).abs();
        let dy = (end_y - y).abs();
        let sx = if x < end_x { 1i32 } else { -1i32 };
        let sy = if y < end_y { 1i32 } else { -1i32 };
        let mut err = dx - dy;

        // bresenham takes exactly max(dx, dy) steps
        let steps = dx.max(dy);
        let dz = if steps > 0 {
            (b.z - a.z) / steps as f32
        } else {
            0.0
        };
        let mut z = a.z;

        // HACK: add a small z bias to line segments to avoid z fighting when drawing block outlines
        // ideally we would change the face texture instead of drawing line in 3d space,
        // but this is good enough for now
        const LINE_Z_BIAS: f32 = 0.0001;
        loop {
            if x >= 0 && (x as usize) < w && y >= 0 && (y as usize) < h {
                let (xi, yi) = (x as usize, y as usize);
                let depth_offset = yi * w + xi;
                if z + LINE_Z_BIAS > self.depth_buffer[depth_offset] {
                    self.depth_buffer[depth_offset] = z;
                    let off = (yi * self.info.stride + xi) * bpp;
                    self.buffer[off..off + bpp].copy_from_slice(color_bytes);
                }
            }

            if x == end_x && y == end_y {
                break;
            }

            let e2 = 2 * err;
            if e2 > -dy {
                err -= dy;
                x += sx;
            }
            if e2 < dx {
                err += dx;
                y += sy;
            }
            z += dz;
        }
    }

    fn fill_line_thick(&mut self, start: Vec3, end: Vec3, color: Color, thickness: f32) {
        let _ = (start, end, color, thickness);
        todo!("Drawing thick lines in 3d space is not implemented for now")
    }

    /// Fills a triangle with the given color within 3d space.
    /// assumes vertices of input triangle are within bounds, with clockwise winding.
    pub fn fill_triangle(&mut self, triangle: &Triangle, color: Color) {
        let (w, h) = (self.info.width, self.info.height);
        let (a, b, c) = (triangle.v0, triangle.v1, triangle.v2);

        // signed area in screen space (CW = positive).
        // "cross product" of AB and AC
        // actually 2x the area, but cancelled when calculating barycentric coords
        let area = (b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y);
        // if triangle taken from project_triangle, it should alreaedy be backface culled
        // double checking here for safety
        if area <= 0.0 {
            return;
        }
        let inv_area = 1.0 / area;

        // assuming vertices of input triangle is within bounds
        let y_min = a.y.min(b.y).min(c.y) as usize;
        let y_max = libm::ceilf(a.y.max(b.y).max(c.y)).min(h as f32) as usize;

        // depth gradient along x for incremental update.
        // dz/dPx = [dwA/dPx * Az + dwB/dPx * Bz + dwC/dPx * Cz] / area
        // where:
        // Px = x coordinate of pixel being drawn
        // wA / area = first component of P's barycentric coords within triangle ABC
        // wA = "cross product" of the BC and BP
        // dwA/dPx = (By-Cy)/area is the change in first component of barycentric coords when Px change by 1
        let dz_dx = ((b.y - c.y) * a.z + (c.y - a.y) * b.z + (a.y - b.y) * c.z) * inv_area;

        // precompute color bytes once
        // to avoids branching on pixel format in inner loop.
        let bpp = self.info.bytes_per_pixel;
        let mut color_bytes = [0u8; 8];
        color.write_to(&mut color_bytes, self.info.pixel_format);
        let color_bytes = &color_bytes[..bpp];

        for y in y_min..y_max {
            let py = y as f32 + 0.5;
            // for edge IJ, the x crossing of this scanline is
            // x_cross = ix + (jx-ix)*(py-iy)/(jy-iy)
            // and for clockwise winding
            // dy = jy-iy > 0 means right boundary
            // dy < 0 = left boundary
            let mut x_left = 0.0f32;
            let mut x_right = w as f32;
            macro_rules! apply_bound_edge {
                ($xi:expr, $yi:expr, $xj:expr, $yj:expr) => {{
                    let dy = $yj - $yi;
                    if dy.abs() > 1e-6 {
                        let x = $xi + ($xj - $xi) * (py - $yi) / dy;
                        if dy > 0.0 {
                            x_right = x_right.min(x);
                        } else {
                            x_left = x_left.max(x);
                        }
                    }
                }};
            }
            apply_bound_edge!(b.x, b.y, c.x, c.y);
            apply_bound_edge!(c.x, c.y, a.x, a.y);
            apply_bound_edge!(a.x, a.y, b.x, b.y);

            // convert to pixel indices
            // pixel x samples at (x + 0.5),
            // so we want xi where (xi + 0.5) is inside [x_left, x_right].
            let xi_left = libm::ceilf(x_left - 0.5).max(0.0);
            let xi_right = libm::floorf(x_right - 0.5).min(w as f32 - 1.0);
            if xi_right < xi_left {
                continue;
            }
            let xi_left = xi_left as usize;
            let xi_right = xi_right as usize;

            // compute depth at xi_left pixel center, then step by dz_dx.
            let px_start = xi_left as f32 + 0.5;
            let wa = (c.x - b.x) * (py - b.y) - (c.y - b.y) * (px_start - b.x);
            let wb = (a.x - c.x) * (py - c.y) - (a.y - c.y) * (px_start - c.x);
            let wc = (b.x - a.x) * (py - a.y) - (b.y - a.y) * (px_start - a.x);
            let mut z = (wa * a.z + wb * b.z + wc * c.z) * inv_area;

            let depth_row_offset = &mut self.depth_buffer[y * w..(y + 1) * w];
            let buffer_row_offset = y * self.info.stride * bpp;

            for x in xi_left..=xi_right {
                // depth buffer uses reverse z (far to near)
                if z > depth_row_offset[x] {
                    depth_row_offset[x] = z;
                    let off = buffer_row_offset + x * bpp;
                    self.buffer[off..off + bpp].copy_from_slice(color_bytes);
                }
                z += dz_dx;
            }
        }
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
