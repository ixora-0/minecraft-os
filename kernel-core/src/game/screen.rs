use alloc::vec::Vec;
use embedded_graphics::{
    Drawable,
    prelude::{DrawTarget, Point, Primitive, Size},
    primitives::{PrimitiveStyle, Rectangle, Triangle as egTriangle},
};
use glam::Vec3;
use spin::Lazy;

use crate::{
    game::{Triangle, camera::Camera},
    rendering::{Color, Renderer},
};

const VOID_COLOR: Lazy<Color> = Lazy::new(|| Color::parse_hex("#82CAFF").unwrap());
const LIGHT_DIRECTION: Lazy<Vec3> = Lazy::new(|| Vec3::new(-1.0, -1.0, 0.2).normalize());

pub struct Screen {
    pub bounding_box: Rectangle,
}
impl Screen {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            bounding_box: Rectangle::new(Point::new(x, y), Size::new(width, height)),
        }
    }

    /// need mutable reference of mesh to sort by depth relative to the camera
    pub fn render(&self, camera: &Camera, mesh: &mut Vec<Triangle>, renderer: &mut Renderer) {
        // painter's algorithm
        mesh.sort_unstable_by(|t1, t2| {
            let d1 = (t1.centroid() - camera.position).length_squared();
            let d2 = (t2.centroid() - camera.position).length_squared();
            d2.partial_cmp(&d1).unwrap() // far to near
        });
        // clear screen
        renderer.fill_solid(&self.bounding_box, *VOID_COLOR);

        let (wf, hf) = (
            self.bounding_box.size.width as f32,
            self.bounding_box.size.height as f32,
        );
        let vpm = camera.view_projection_matrix(wf, hf);
        let projected_mesh = mesh
            .iter()
            .filter_map(|tri| camera.project_triangle(&vpm, tri, wf, hf));
        for tri in projected_mesh {
            let light = -tri.normal.dot(*LIGHT_DIRECTION); // -1 to 1
            const MIN_LIGHT: f32 = 0.1;
            let light = MIN_LIGHT + (1.0 - MIN_LIGHT) * ((light + 1.0) / 2.0); // min to 1
            let color = Color::WHITE.with_intensity_f(light);

            let t = egTriangle::new(
                Point::new(
                    tri.v0.x as i32 + self.bounding_box.top_left.x,
                    tri.v0.y as i32 + self.bounding_box.top_left.y,
                ),
                Point::new(
                    tri.v1.x as i32 + self.bounding_box.top_left.x,
                    tri.v1.y as i32 + self.bounding_box.top_left.y,
                ),
                Point::new(
                    tri.v2.x as i32 + self.bounding_box.top_left.x,
                    tri.v2.y as i32 + self.bounding_box.top_left.y,
                ),
            );
            t.into_styled(PrimitiveStyle::with_fill(color))
                .draw(renderer);
            // t.into_styled(PrimitiveStyle::with_stroke(Color::RED, 1))
            //     .draw(renderer);
        }
    }
}
