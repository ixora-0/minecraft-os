use glam::{ISizeVec3, Vec3};
pub mod camera;
pub mod screen;
pub mod world;

pub use camera::Camera;
pub use screen::Screen;
pub use world::World;

pub struct Triangle {
    v0: Vec3,
    v1: Vec3,
    v2: Vec3,
    normal: Vec3,
}
impl Triangle {
    /// order of vertices should be clockwise when normal is pointing at us
    pub fn new(v0: Vec3, v1: Vec3, v2: Vec3) -> Self {
        let normal = {
            let edge1 = v1 - v0;
            let edge2 = v2 - v0;
            edge2.cross(edge1).normalize()
        };
        Self { v0, v1, v2, normal }
    }
}
impl Triangle {
    pub fn centroid(&self) -> Vec3 {
        (self.v0 + self.v1 + self.v2) / 3.0
    }
}

#[derive(PartialEq, Debug)]
pub enum Face {
    FRONT,
    BACK,
    TOP,
    BOTTOM,
    LEFT,
    RIGHT,
}

impl Face {
    pub fn get_offset(&self) -> ISizeVec3 {
        match self {
            Face::FRONT => ISizeVec3::new(0, 0, 1),
            Face::BACK => ISizeVec3::new(0, 0, -1),
            Face::TOP => ISizeVec3::new(0, 1, 0),
            Face::BOTTOM => ISizeVec3::new(0, -1, 0),
            Face::RIGHT => ISizeVec3::new(1, 0, 0),
            Face::LEFT => ISizeVec3::new(-1, 0, 0),
        }
    }
}
