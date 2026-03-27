use glam::Vec3;
pub mod camera;
pub mod world;

pub use camera::Camera;

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
