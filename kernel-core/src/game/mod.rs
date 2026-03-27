use glam::Vec3;
pub mod world;

pub struct Triangle {
    v0: Vec3,
    v1: Vec3,
    v2: Vec3,
    normal: Vec3,
}
