use crate::game::{Face, World};

use super::Triangle;
use glam::{ISizeVec3, Mat4, USizeVec3, Vec2, Vec3, Vec4Swizzles};
const NEAR: f32 = 0.1;

pub struct Triangle2D {
    pub v0: Vec2,
    pub v1: Vec2,
    pub v2: Vec2,
    /// normal of the triangle before projection
    /// used for lighting calculations
    pub normal: Vec3,
}

/// Traverses voxels along a ray, iterator returning the grid coordinate and face of each voxel
/// Uses Amanatides-Woo algorithm
enum VoxelTraverser {
    Empty,
    Uninit {
        world_dimensions: USizeVec3,
        position: Vec3,
        forward: Vec3,
    },
    Active {
        current_block: USizeVec3,
        world_dimensions: USizeVec3,
        step: ISizeVec3,
        delta: Vec3,
        t: Vec3,
    },
}
impl VoxelTraverser {
    pub fn new(world_dimensions: USizeVec3, position: Vec3, forward: Vec3) -> Self {
        Self::Uninit {
            world_dimensions,
            position,
            forward,
        }
    }
}
impl Iterator for VoxelTraverser {
    type Item = (USizeVec3, Face);

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            VoxelTraverser::Empty => None,
            VoxelTraverser::Uninit {
                world_dimensions,
                position,
                forward,
            } => {
                let forward_inverse =
                    forward.map(|x| if x == 0.0 { f32::INFINITY } else { x.recip() });
                let current_block = {
                    let mut tmin = 0f32;
                    let mut tmax = f32::INFINITY;
                    for i in 0..3 {
                        let t1 = -position[i] * forward_inverse[i];
                        let t2 = (world_dimensions[i] as f32 - position[i]) * forward_inverse[i];
                        tmin = tmin.max(t1.min(t2));
                        tmax = tmax.min(t1.max(t2));
                    }
                    if tmax < tmin {
                        *self = Self::Empty;
                        return None;
                    }
                    let point = *position + *forward * tmin;
                    point.as_usizevec3()
                };

                let step = forward.map(|x| x.signum()).as_isizevec3();
                let delta = forward_inverse.abs();
                let t = {
                    let select = forward.map(|x| 0.5 + 0.5 * x.signum());
                    let planes = current_block.as_vec3() + select;
                    (planes - *position) * forward_inverse
                };
                *self = Self::Active {
                    current_block,
                    world_dimensions: *world_dimensions,
                    step,
                    delta,
                    t,
                };
                Some((current_block, Face::BACK))
            }
            VoxelTraverser::Active {
                current_block,
                world_dimensions,
                step,
                delta,
                t,
            } => {
                let face = if t.x < t.y {
                    if t.x < t.z {
                        current_block.x = current_block.x.saturating_add_signed(step.x);
                        t.x += delta.x;
                        // +X is right face, step.x < 0 means coming from +X, so right
                        if step.x < 0 { Face::RIGHT } else { Face::LEFT }
                    } else {
                        current_block.z = current_block.z.saturating_add_signed(step.z);
                        t.z += delta.z;
                        if step.z < 0 { Face::FRONT } else { Face::BACK }
                    }
                } else {
                    if t.y < t.z {
                        current_block.y = current_block.y.saturating_add_signed(step.y);
                        t.y += delta.y;
                        if step.y < 0 { Face::TOP } else { Face::BOTTOM }
                    } else {
                        current_block.z = current_block.z.saturating_add_signed(step.z);
                        t.z += delta.z;
                        if step.z < 0 { Face::FRONT } else { Face::BACK }
                    }
                };
                if current_block.x >= world_dimensions.x
                    || current_block.y >= world_dimensions.y
                    || current_block.z >= world_dimensions.z
                {
                    *self = VoxelTraverser::Empty;
                    return None;
                }
                Some((*current_block, face))
            }
        }
    }
}

pub struct Camera {
    pub position: Vec3,
    /// left-right rotation in radians, relative to +Z, positive rotating towards -X (the right)
    pub yaw: f32,
    /// up-down rotation in radians, relative to the XZ plane, positive rotating towards +Y (upwards)
    pub pitch: f32,
    /// vertical field of view in radians
    pub v_fov: f32,
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            v_fov: 70.0,
        }
    }
}

impl Camera {
    pub fn set_position(&mut self, x: f32, y: f32, z: f32) {
        self.position = Vec3::new(x, y, z);
    }

    /// Returns the unit vector of the camera's direction.
    /// At yaw=0 and pitch=0, the forward direction is (0, 0, 1), which is +Z.
    pub fn forward(&self) -> Vec3 {
        Vec3::new(
            -libm::cosf(self.pitch) * libm::sinf(self.yaw),
            libm::sinf(self.pitch),
            libm::cosf(self.pitch) * libm::cosf(self.yaw),
        )
    }

    pub fn view_projection_matrix(&self, width: f32, height: f32) -> Mat4 {
        let target = self.position + self.forward();
        let view = Mat4::look_at_rh(self.position, target, Vec3::Y);
        let proj = Mat4::perspective_infinite_reverse_rh(self.v_fov, width / height, NEAR);
        proj * view
    }

    /// Returns None if the vertex is behind the camera or fully outside clip space.
    /// NOTE: this means triangles partially off-screen get culled entirely. Good enough for now.
    fn project_vertex(&self, vpm: &Mat4, v: Vec3, width: f32, height: f32) -> Option<Vec2> {
        let clip = vpm * v.extend(1.0); // extend to homogeneous coords

        // behind the camera
        if clip.w <= 0.0 {
            return None;
        }

        // convert to NDC, range (-1, 1) on each axis
        let ndc = clip.xyz() / clip.w;

        // outside clip space
        if ndc.x < -1.0 || ndc.x > 1.0 || ndc.y < -1.0 || ndc.y > 1.0 || ndc.z < -1.0 || ndc.z > 1.0
        {
            return None;
        }

        // transform to viewport pixel coords
        // y is flipped because NDC +y is up, screen +y is down
        let px = (ndc.x + 1.0) / 2.0 * width;
        let py = (1.0 - ndc.y) / 2.0 * height;

        Some(Vec2::new(px, py))
    }

    pub fn project_triangle(
        &self,
        vpm: &Mat4,
        tri: &Triangle,
        width: f32,
        height: f32,
    ) -> Option<Triangle2D> {
        // backface culling
        // if triangle is facing away from camera, don't render
        let to_camera = self.position - tri.v0;
        if to_camera.dot(tri.normal) <= 0.0 {
            return None;
        }

        Some(Triangle2D {
            v0: self.project_vertex(vpm, tri.v0, width, height)?,
            v1: self.project_vertex(vpm, tri.v1, width, height)?,
            v2: self.project_vertex(vpm, tri.v2, width, height)?,
            normal: tri.normal,
        })
    }

    /// Returns the world coordinates and face of the solid block that the camera is looking at, if any.
    pub fn looking_at_solid_block(&self, world: &World) -> Option<(USizeVec3, Face)> {
        let world_dimensions = USizeVec3::new(world.len(), world[0].len(), world[0][0].len());
        let traverser = VoxelTraverser::new(world_dimensions, self.position, self.forward());
        for (pos, face) in traverser {
            if world[pos.x][pos.y][pos.z] {
                return Some((pos, face));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec4;

    #[test]
    fn forward_vectors() {
        let mut camera = Camera::default();
        let forward = camera.forward();
        assert_abs_diff_eq!(forward.x, 0.0);
        assert_abs_diff_eq!(forward.y, 0.0);
        assert_abs_diff_eq!(forward.z, 1.0);

        const PI: f32 = 3.14159265358979323846264338327950288;

        camera.yaw += PI / 2.0; // should turn right to -X
        let forward = camera.forward();
        assert_abs_diff_eq!(forward.x, -1.0);
        assert_abs_diff_eq!(forward.y, 0.0);
        assert_abs_diff_eq!(forward.z, 0.0);
        camera.yaw += PI / 2.0; // should turn right to -Z
        let forward = camera.forward();
        assert_abs_diff_eq!(forward.x, 0.0);
        assert_abs_diff_eq!(forward.y, 0.0);
        assert_abs_diff_eq!(forward.z, -1.0);
        camera.yaw += PI / 2.0; // should turn right to +X
        let forward = camera.forward();
        assert_abs_diff_eq!(forward.x, 1.0);
        assert_abs_diff_eq!(forward.y, 0.0);
        assert_abs_diff_eq!(forward.z, 0.0);

        camera.pitch += PI / 2.0;
        let forward = camera.forward();
        assert_abs_diff_eq!(forward.x, 0.0);
        assert_abs_diff_eq!(forward.y, 1.0);
        assert_abs_diff_eq!(forward.z, 0.0);
    }

    #[test]
    fn projection_center_screen() {
        let (width, height) = (800.0, 600.0);
        let camera = Camera::default();
        let vpm = camera.view_projection_matrix(width, height);

        let vertex = Vec3::new(0.0, 0.0, 10.0);
        let result = camera.project_vertex(&vpm, vertex, width, height);

        assert!(result.is_some(), "Vertex in front should not be culled");
        let projected = result.unwrap();
        // should be exactly in the middle of the screen
        assert_abs_diff_eq!(projected.x, width / 2.0);
        assert_abs_diff_eq!(projected.y, height / 2.0);
    }

    #[test]
    fn view_projection_matrix() {
        let (width, height) = (800.0, 600.0);
        let camera = Camera::default();
        let vp = camera.view_projection_matrix(width, height);

        let clip_front = vp * Vec4::new(0.0, 0.0, 10.0, 1.0);
        // W should represent distance from camera
        assert_abs_diff_eq!(clip_front.w, 10.0);

        let clip_behind = vp * Vec4::new(0.0, 0.0, -10.0, 1.0);
        assert!(
            clip_behind.w < 0.0,
            "W must be negative for behind-camera geometry"
        );
    }

    #[test]
    fn test_behind_camera_culling() {
        let (width, height) = (800.0, 600.0);
        let camera = Camera::default();
        let vpm = camera.view_projection_matrix(width, height);

        let vertex_behind = Vec3::new(0.0, 0.0, -5.0);
        let res = camera.project_vertex(&vpm, vertex_behind, width, height);
        assert!(res.is_none(), "Vertices behind camera should be culled");
    }

    #[test]
    fn reverse_depth_mapping() {
        let (width, height) = (800.0, 600.0);
        let camera = Camera::default();
        let vpm = camera.view_projection_matrix(width, height);

        let v_near = Vec3::new(0.0, 0.0, NEAR);
        let clip_near = vpm * v_near.extend(1.0);
        let ndc_near = clip_near.xyz() / clip_near.w;
        assert_abs_diff_eq!(ndc_near.z, 1.0);

        let v_far = Vec3::new(0.0, 0.0, 100000.0);
        let clip_far = vpm * v_far.extend(1.0);
        let ndc_far = clip_far.xyz() / clip_far.w;
        assert!(
            ndc_far.z > 0.0 && ndc_far.z < 0.01,
            "Far plane should map close to 0.0"
        );
    }

    #[test]
    fn backface_culling() {
        // Camera at origin looking along +Z
        let camera = Camera::default();
        let (width, height) = (800.0, 600.0);
        let vpm = camera.view_projection_matrix(width, height);

        // triangle facing camera (normal pointing toward camera, which is -Z)
        // vertices arranged clockwise when viewed from camera
        let tri_facing = Triangle::new(
            Vec3::new(-1.0, -1.0, 5.0),
            Vec3::new(1.0, -1.0, 5.0),
            Vec3::new(0.0, 1.0, 5.0),
        );
        // should render
        let result_facing = camera.project_triangle(&vpm, &tri_facing, width, height);
        assert!(
            result_facing.is_some(),
            "Front-facing triangle should render"
        );
    }

    #[test]
    fn looking_at_solid_block_hit() {
        let mut camera = Camera::default();
        camera.set_position(1.5, 1.5, -0.5);

        let mut world: World = [[[false; 4]; 4]; 4];
        world[1][1][0] = true;

        let result = camera.looking_at_solid_block(&world);
        assert!(result.is_some(), "Should hit solid block");

        let (pos, face) = result.unwrap();
        assert_eq!(pos.x, 1);
        assert_eq!(pos.y, 1);
        assert_eq!(pos.z, 0);
        assert_eq!(face, Face::BACK);
    }

    #[test]
    fn looking_at_solid_block_miss() {
        let camera = Camera::default();

        let world: World = [[[false; 4]; 4]; 4];

        let result = camera.looking_at_solid_block(&world);
        assert!(result.is_none(), "Should not hit any block");
    }

    #[test]
    fn looking_at_solid_block_closest() {
        let mut camera = Camera::default();
        camera.set_position(1.5, 1.5, -0.5);

        let mut world: World = [[[false; 4]; 4]; 4];
        world[1][1][0] = true;
        world[2][2][1] = true;

        let result = camera.looking_at_solid_block(&world);
        assert!(result.is_some(), "Should hit solid block");

        let (pos, _) = result.unwrap();
        assert_eq!(pos.x, 1);
        assert_eq!(pos.y, 1);
        assert_eq!(pos.z, 0, "Should return closest block, not further one");
    }
}
