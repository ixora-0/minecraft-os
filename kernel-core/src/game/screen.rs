use alloc::vec::Vec;
use embedded_graphics::{
    Drawable,
    prelude::{DrawTarget, Point, Primitive, Size},
    primitives::{PrimitiveStyle, Rectangle, Triangle as egTriangle},
};
use spin::Lazy;

use crate::{
    game::{Triangle, camera::Camera},
    rendering::{Color, Renderer},
};

const VOID_COLOR: Lazy<Color> = Lazy::new(|| Color::parse_hex("#82CAFF").unwrap());

pub struct Screen {
    pub bounding_box: Rectangle,
}
impl Screen {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            bounding_box: Rectangle::new(Point::new(x, y), Size::new(width, height)),
        }
    }
    pub fn render(&self, camera: &Camera, mesh: &Vec<Triangle>, renderer: &mut Renderer) {
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
            egTriangle::new(
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
            )
            .into_styled(PrimitiveStyle::with_stroke(Color::WHITE, 1))
            .draw(renderer);
        }
    }
}
