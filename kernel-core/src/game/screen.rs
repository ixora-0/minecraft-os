use alloc::{vec, vec::Vec};
use bootloader_api::info::{FrameBufferInfo, PixelFormat};
use core::ptr;
use glam::{IVec2, USizeVec2, USizeVec3, Vec3};
use spin::Lazy;

use crate::{
    game::{Triangle, camera::Camera},
    rendering::{Color, Rectangle, Renderer, renderer::Renderer3d},
};

const VOID_COLOR: Lazy<Color> = Lazy::new(|| Color::parse_hex("#82CAFF").unwrap());
const LIGHT_DIRECTION: Lazy<Vec3> = Lazy::new(|| Vec3::new(-1.0, -1.0, 0.2).normalize());
const CROSSHAIR_COLOR: Color = Color::BLACK;
const CROSSHAIR_LEN: u32 = 5;
const CROSSHAIR_THICKNESS: f32 = 1.0;

pub struct Screen {
    /// bounding box within the global framebuffer
    pub bounding_box: Rectangle,
    /// temporary buffer used for rendering, can be flushed to the global framebuffer
    buffer: Vec<u8>,
    depth_buffer: Vec<f32>,
    info: FrameBufferInfo,
}

impl Screen {
    pub fn new(
        x: i32,
        y: i32,
        width: usize,
        height: usize,
        pixel_format: PixelFormat,
        bytes_per_pixel: usize,
    ) -> Self {
        let byte_len = width * height * bytes_per_pixel;
        let info = FrameBufferInfo {
            byte_len,
            width,
            height,
            pixel_format,
            bytes_per_pixel,
            stride: width,
        };
        Self {
            bounding_box: Rectangle {
                top_left: IVec2::new(x, y),
                size: USizeVec2::new(width, height),
            },
            buffer: vec![0u8; byte_len],
            depth_buffer: vec![0.0f32; width * height],
            info,
        }
    }

    fn as_2d_draw_target(&mut self) -> Renderer<'_> {
        Renderer::new(self.buffer.as_mut_slice(), self.info)
    }
    fn as_3d_draw_target(&mut self) -> Renderer3d<'_> {
        Renderer3d::new(
            self.buffer.as_mut_slice(),
            self.depth_buffer.as_mut_slice(),
            self.info,
        )
    }

    /// need mutable reference of mesh to sort by depth relative to the camera (painter's algorithm)
    pub fn render(&mut self, camera: &Camera, mesh: &Vec<Triangle>) {
        let (wf, hf) = (
            self.bounding_box.size.x as f32,
            self.bounding_box.size.y as f32,
        );
        let vpm = camera.view_projection_matrix(wf, hf);
        let projected_mesh = mesh
            .iter()
            .filter_map(|tri| camera.project_triangle(&vpm, tri, wf, hf));

        // clear screen
        self.as_2d_draw_target().clear(*VOID_COLOR);
        self.depth_buffer.fill(0.0);

        // draw mesh
        let mut renderer = self.as_3d_draw_target();
        for triangle in projected_mesh {
            let light = -triangle.normal.dot(*LIGHT_DIRECTION); // -1 to 1
            const MIN_LIGHT: f32 = 0.1;
            let light = MIN_LIGHT + (1.0 - MIN_LIGHT) * ((light + 1.0) / 2.0); // min to 1
            let color = Color::WHITE.with_intensity_f(light);

            renderer.fill_triangle(&triangle, color);
            // t.into_styled(PrimitiveStyle::with_stroke(Color::RED, 1))
            //     .draw(&mut renderer);
        }
    }

    pub fn draw_block_outline(&mut self, camera: &Camera, block: USizeVec3, color: Color) {
        let (wf, hf) = (
            self.bounding_box.size.x as f32,
            self.bounding_box.size.y as f32,
        );
        let vpm = camera.view_projection_matrix(wf, hf);

        let (x, y, z) = (block.x as f32, block.y as f32, block.z as f32);
        let corners = [
            Vec3::new(x, y, z),
            Vec3::new(x + 1.0, y, z),
            Vec3::new(x + 1.0, y, z + 1.0),
            Vec3::new(x, y, z + 1.0),
            Vec3::new(x, y + 1.0, z),
            Vec3::new(x + 1.0, y + 1.0, z),
            Vec3::new(x + 1.0, y + 1.0, z + 1.0),
            Vec3::new(x, y + 1.0, z + 1.0),
        ];
        const EDGES: [(usize, usize); 12] = [
            // a
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 0),
            // b
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 4),
            // c
            (0, 4),
            (1, 5),
            (2, 6),
            (3, 7),
        ];

        let projected: [Option<Vec3>; 8] =
            core::array::from_fn(|i| camera.project_vertex(&vpm, corners[i], wf, hf));

        let mut renderer = self.as_3d_draw_target();
        for (a, b) in EDGES {
            if let (Some(p0), Some(p1)) = (projected[a], projected[b]) {
                renderer.draw_line(p0, p1, color, 1.0);
            }
        }
    }

    pub fn draw_crosshair(&mut self) {
        let wf = self.bounding_box.size.x as f32;
        let hf = self.bounding_box.size.y as f32;
        let center = IVec2::new((wf / 2.0) as i32, (hf / 2.0) as i32);

        let mut renderer = self.as_2d_draw_target();
        renderer.draw_line(
            IVec2::new(center.x - CROSSHAIR_LEN as i32, center.y),
            IVec2::new(center.x + CROSSHAIR_LEN as i32, center.y),
            CROSSHAIR_COLOR,
            CROSSHAIR_THICKNESS,
        );
        renderer.draw_line(
            IVec2::new(center.x, center.y - CROSSHAIR_LEN as i32),
            IVec2::new(center.x, center.y + CROSSHAIR_LEN as i32),
            CROSSHAIR_COLOR,
            CROSSHAIR_THICKNESS,
        );
    }

    /// Copies the screen's temporary buffer into the global renderer.
    ///
    /// Panics if pixel formats don't match.
    /// Caller should ensure the screen's pixel format matches the global renderer's.
    pub fn flush(&mut self, global_renderer: &mut Renderer) {
        assert_eq!(
            self.info.pixel_format, global_renderer.info.pixel_format,
            "Pixel format mismatch: screen uses {:?}, global uses {:?}.",
            self.info.pixel_format, global_renderer.info.pixel_format
        );
        let bpp = self.info.bytes_per_pixel;
        assert_eq!(
            bpp, global_renderer.info.bytes_per_pixel,
            "bytes_per_pixel mismatch"
        );

        let Some(area) = global_renderer
            .bounding_box()
            .intersection(&self.bounding_box)
        else {
            return;
        };
        let dst_start_x = area.top_left.x as usize;
        let dst_start_y = area.top_left.y as usize;
        let copy_width = area.size.x;
        let copy_height = area.size.y;

        let src_start_x = (-self.bounding_box.top_left.x).max(0) as usize;
        let src_start_y = (-self.bounding_box.top_left.y).max(0) as usize;

        // bytes per row to copy
        let copy_bytes = (copy_width * bpp) as usize;

        let src_ptr = self.buffer.as_ptr();
        let dst_ptr = global_renderer.buffer_mut().as_mut_ptr();
        for y in 0..copy_height {
            let src_offset = ((src_start_y + y) * self.info.stride + src_start_x) * bpp;
            let dst_offset = ((dst_start_y + y) * global_renderer.info.stride + dst_start_x) * bpp;

            unsafe {
                ptr::copy_nonoverlapping(
                    src_ptr.add(src_offset),
                    dst_ptr.add(dst_offset),
                    copy_bytes,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rendering::Renderer;

    fn make_framebuffer_info(width: usize, height: usize, bpp: usize) -> FrameBufferInfo {
        FrameBufferInfo {
            byte_len: width * height * bpp,
            width,
            height,
            pixel_format: PixelFormat::Rgb,
            bytes_per_pixel: bpp,
            stride: width,
        }
    }

    #[test]
    fn flush_copies_data_correctly() {
        let bpp = 4;
        let screen_width = 4;
        let screen_height = 4;

        let mut screen = Screen::new(0, 0, screen_width, screen_height, PixelFormat::Rgb, bpp);
        screen.buffer.fill(0xFF);

        let mut global_buffer = vec![0u8; 16 * 16 * 4];
        let mut global_renderer =
            Renderer::new(&mut global_buffer, make_framebuffer_info(16, 16, bpp));

        screen.flush(&mut global_renderer);

        for y in 0..4 {
            for x in 0..4 {
                let offset = (y * 16 + x) * 4;
                assert_eq!(global_buffer[offset], 0xFF);
            }
        }
    }

    #[test]
    #[should_panic(expected = "Pixel format mismatch")]
    fn flush_panics_on_pixel_format_mismatch() {
        let bpp = 4;
        let mut screen = Screen::new(0, 0, 4, 4, PixelFormat::Rgb, bpp);

        let mut global_buffer = vec![0u8; 16 * 16 * 4];
        let mut wrong_format_info = make_framebuffer_info(16, 16, bpp);
        wrong_format_info.pixel_format = PixelFormat::Bgr;
        let mut global_renderer = Renderer::new(&mut global_buffer, wrong_format_info);

        screen.flush(&mut global_renderer);
    }

    #[test]
    #[should_panic(expected = "bytes_per_pixel mismatch")]
    fn flush_panics_on_bpp_mismatch() {
        let mut screen = Screen::new(0, 0, 4, 4, PixelFormat::Rgb, 4);

        let mut global_buffer = vec![0u8; 16 * 16 * 3];
        let info = FrameBufferInfo {
            byte_len: 16 * 16 * 3,
            width: 16,
            height: 16,
            pixel_format: PixelFormat::Rgb,
            bytes_per_pixel: 3,
            stride: 16 * 3,
        };
        let mut global_renderer = Renderer::new(&mut global_buffer, info);

        screen.flush(&mut global_renderer);
    }

    #[test]
    fn flush_no_intersection_returns_early() {
        let bpp = 4;
        let mut screen = Screen::new(100, 100, 4, 4, PixelFormat::Rgb, bpp);
        screen.buffer.fill(0xFF);

        let mut global_buffer = vec![0u8; 16 * 16 * 4];
        let mut global_renderer =
            Renderer::new(&mut global_buffer, make_framebuffer_info(16, 16, bpp));

        screen.flush(&mut global_renderer);
        assert!(global_buffer.iter().all(|&b| b == 0));
    }

    #[test]
    fn flush_partial_overlap() {
        let bpp = 4;
        let mut screen = Screen::new(2, 2, 4, 4, PixelFormat::Rgb, bpp);
        screen.buffer.fill(0xAA);

        let mut global_buffer = vec![0u8; 16 * 16 * 4];
        let mut global_renderer =
            Renderer::new(&mut global_buffer, make_framebuffer_info(16, 16, bpp));

        screen.flush(&mut global_renderer);

        for y in 2..6 {
            for x in 2..6 {
                let offset = (y * 16 + x) * 4;
                assert_eq!(global_buffer[offset], 0xAA);
            }
        }
        assert_eq!(global_buffer[0], 0);
    }
}
