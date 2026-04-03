use glam::{IVec2, USizeVec2, USizeVec3, Vec2, Vec3};
use spin::Lazy;

use crate::{
    game::{Triangle, camera::Camera},
    rendering::{Color, Frame, Rectangle},
};

const VOID_COLOR: Lazy<Color> = Lazy::new(|| Color::parse_hex("#82CAFF").unwrap());
const LIGHT_DIRECTION: Lazy<Vec3> = Lazy::new(|| Vec3::new(-1.0, -1.0, 0.2).normalize());
const CROSSHAIR_COLOR: Color = Color::BLACK;
const CROSSHAIR_LEN: f32 = 5.0;
const CROSSHAIR_THICKNESS: f32 = 2.0;

pub struct Screen {
    /// bounding box within the global framebuffer
    pub bounding_box: Rectangle,
}

impl Screen {
    pub fn new(x: i32, y: i32, width: usize, height: usize) -> Self {
        Self {
            bounding_box: Rectangle {
                top_left: IVec2::new(x, y),
                size: USizeVec2::new(width, height),
            },
        }
    }

    pub fn render(&self, frame: &mut Frame, camera: &Camera, mesh: &[Triangle]) {
        let (wf, hf) = (
            self.bounding_box.size.x as f32,
            self.bounding_box.size.y as f32,
        );
        let vpm = camera.view_projection_matrix(wf, hf);
        let projected_mesh = mesh
            .iter()
            .filter_map(|tri| camera.project_triangle(&vpm, tri, wf, hf));

        {
            let mut renderer = frame.renderer2d();
            renderer.fill_solid(&self.bounding_box, *VOID_COLOR);
        }

        // draw mesh
        let mut renderer = frame.renderer3d();
        let offset = Vec3::new(
            self.bounding_box.top_left.x as f32,
            self.bounding_box.top_left.y as f32,
            0.0,
        );
        for triangle in projected_mesh {
            let mut triangle = triangle;
            triangle.v0 += offset;
            triangle.v1 += offset;
            triangle.v2 += offset;
            let light = -triangle.normal.dot(*LIGHT_DIRECTION); // -1 to 1
            const MIN_LIGHT: f32 = 0.1;
            let light = MIN_LIGHT + (1.0 - MIN_LIGHT) * ((light + 1.0) / 2.0); // min to 1
            let color = Color::WHITE.with_intensity_f(light);

            renderer.fill_triangle(&triangle, color, Some(&self.bounding_box));
            // t.into_styled(PrimitiveStyle::with_stroke(Color::RED, 1))
            //     .draw(&mut renderer);
        }
    }

    pub fn draw_block_outline(
        &self,
        frame: &mut Frame,
        camera: &Camera,
        block: USizeVec3,
        color: Color,
    ) {
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

        let mut renderer = frame.renderer3d();
        let offset = Vec3::new(
            self.bounding_box.top_left.x as f32,
            self.bounding_box.top_left.y as f32,
            0.0,
        );
        for (a, b) in EDGES {
            if let (Some(p0), Some(p1)) = (projected[a], projected[b]) {
                renderer.draw_line(
                    p0 + offset,
                    p1 + offset,
                    color,
                    1.0,
                    Some(&self.bounding_box),
                );
            }
        }
    }

    pub fn draw_crosshair(&self, frame: &mut Frame) {
        let top_left = self.bounding_box.top_left.as_vec2();
        let center = top_left + self.bounding_box.size.as_vec2() * 0.5;

        let mut renderer = frame.renderer2d();
        renderer.draw_line(
            Vec2::new(center.x - CROSSHAIR_LEN, center.y),
            Vec2::new(center.x + CROSSHAIR_LEN, center.y),
            CROSSHAIR_COLOR,
            CROSSHAIR_THICKNESS,
        );
        renderer.draw_line(
            Vec2::new(center.x, center.y - CROSSHAIR_LEN),
            Vec2::new(center.x, center.y + CROSSHAIR_LEN),
            CROSSHAIR_COLOR,
            CROSSHAIR_THICKNESS,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use bootloader_api::info::{FrameBufferInfo, PixelFormat};

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

    fn pixel_index(info: &FrameBufferInfo, x: usize, y: usize) -> usize {
        ((y * info.stride) + x) * info.bytes_per_pixel
    }

    #[test]
    fn render_fills_viewport_area() {
        let info = make_framebuffer_info(8, 8, 4);
        let mut color = vec![0u8; info.byte_len];
        let mut depth = vec![1.0f32; info.width * info.height];
        let screen = Screen::new(2, 2, 4, 4);
        {
            let mut frame = Frame::new(color.as_mut_slice(), depth.as_mut_slice(), info);
            frame.clear_depth();
            screen.render(&mut frame, &Camera::default(), &[]);
        }

        for y in 0..info.height {
            for x in 0..info.width {
                let idx = pixel_index(&info, x, y);
                if (2..6).contains(&x) && (2..6).contains(&y) {
                    assert_eq!(color[idx], (*VOID_COLOR).red);
                } else {
                    assert_eq!(color[idx], 0);
                }
            }
        }
    }

    #[test]
    fn crosshair_respects_screen_offset() {
        let info = make_framebuffer_info(10, 10, 4);
        let mut color = vec![0u8; info.byte_len];
        let mut depth = vec![0.0f32; info.width * info.height];
        let screen = Screen::new(3, 3, 4, 4);
        {
            let mut frame = Frame::new(color.as_mut_slice(), depth.as_mut_slice(), info);
            screen.draw_crosshair(&mut frame);
        }

        let center_x = screen.bounding_box.top_left.x + (screen.bounding_box.size.x as i32 / 2);
        let center_y = screen.bounding_box.top_left.y + (screen.bounding_box.size.y as i32 / 2);
        let idx = pixel_index(&info, center_x as usize, center_y as usize);
        assert_eq!(color[idx], CROSSHAIR_COLOR.red);
    }
}
