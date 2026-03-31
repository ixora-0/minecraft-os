use core::{f32, ops::Neg};

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
        max_distance: f32,
    },
    Active {
        current_block: USizeVec3,
        world_dimensions: USizeVec3,
        step: ISizeVec3,
        delta: Vec3,
        t: Vec3,
        /// distance allowed to travel during the active phases
        /// isn't changed during iteration while in Active
        remaining_distance: f32,
    },
}
impl VoxelTraverser {
    /// set max distance to infinity if don't want to limit
    pub fn new(
        world_dimensions: USizeVec3,
        position: Vec3,
        forward: Vec3,
        max_distance: f32,
    ) -> Self {
        Self::Uninit {
            world_dimensions,
            position,
            forward,
            max_distance,
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
                max_distance,
            } => {
                // get the first voxel intersection by slab method
                let forward_inverse = forward.recip();
                let forward_sign = forward_inverse.signum();

                let (axis, tclose, tfar) = {
                    let t1 = position.neg() * forward_inverse;
                    let t2 = (world_dimensions.as_vec3() - *position) * forward_inverse;
                    let tclose_v = t1.min(t2);
                    let tfar_v = t1.max(t2);
                    let axis = tclose_v.max_position();
                    let tclose = tclose_v[axis].clamp(0.0, f32::INFINITY);
                    let tfar = tfar_v.min_element();
                    (axis, tclose, tfar)
                };
                if tclose > tfar {
                    *self = Self::Empty;
                    return None;
                }

                let remaining_distance = *max_distance - tclose;
                if remaining_distance < 0.0 {
                    *self = Self::Empty;
                    return None;
                }

                *position += *forward * tclose;
                let current_block = position
                    .floor()
                    .clamp(Vec3::ZERO, (*world_dimensions - 1).as_vec3())
                    .as_usizevec3();
                if current_block.x >= world_dimensions.x
                    || current_block.y >= world_dimensions.y
                    || current_block.z >= world_dimensions.z
                {
                    *self = Self::Empty;
                    return None;
                }

                let step = forward_sign.as_isizevec3();
                let delta_t = forward_inverse.abs();
                // t[i] * forward[i] = i coordinate of next i plane, i = x/y/z
                let t = {
                    let select = 0.5 + 0.5 * forward_sign;
                    let planes = current_block.as_vec3() + select;
                    (planes - *position) * forward_inverse
                };
                *self = Self::Active {
                    current_block,
                    world_dimensions: *world_dimensions,
                    step,
                    delta: delta_t,
                    t,
                    remaining_distance,
                };

                let face = match axis {
                    0 => {
                        if forward_sign.x.is_sign_negative() {
                            Face::RIGHT
                        } else {
                            Face::LEFT
                        }
                    }
                    1 => {
                        if forward_sign.y.is_sign_negative() {
                            Face::TOP
                        } else {
                            Face::BOTTOM
                        }
                    }
                    2 => {
                        if forward_sign.z.is_sign_negative() {
                            Face::FRONT
                        } else {
                            Face::BACK
                        }
                    }
                    _ => unreachable!(),
                };
                Some((current_block, face))
            }
            VoxelTraverser::Active {
                current_block,
                world_dimensions,
                step,
                delta,
                t,
                remaining_distance,
            } => {
                let axis = t.min_position();
                // using indexing here is slow, but much more readable
                let distance_traveled = t[axis];
                current_block[axis] = current_block[axis].wrapping_add_signed(step[axis]);
                if distance_traveled > *remaining_distance
                    || current_block.x >= world_dimensions.x
                    || current_block.y >= world_dimensions.y
                    || current_block.z >= world_dimensions.z
                {
                    *self = VoxelTraverser::Empty;
                    return None;
                }

                t[axis] += delta[axis];
                let face = match axis {
                    0 => {
                        // +X is right face, step.x < 0 means coming from +X, so looking at right
                        if step.x < 0 { Face::RIGHT } else { Face::LEFT }
                    }
                    1 => {
                        // +Y is top face, step.Y < 0 means coming from +Y, so looking at top
                        if step.y < 0 { Face::TOP } else { Face::BOTTOM }
                    }
                    2 => {
                        // +Z is front face, step.Z < 0 means coming from +Z, so looking at front
                        if step.z < 0 { Face::FRONT } else { Face::BACK }
                    }
                    _ => unreachable!(),
                };

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
    pub fn project_vertex(&self, vpm: &Mat4, v: Vec3, width: f32, height: f32) -> Option<Vec2> {
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
    /// Set max_distance to f32::INFINITY for infinite distance.
    pub fn looking_at_solid_block(
        &self,
        world: &World,
        max_distance: f32,
    ) -> Option<(USizeVec3, Face)> {
        let world_dimensions = USizeVec3::new(world.len(), world[0].len(), world[0][0].len());
        let traverser = VoxelTraverser::new(
            world_dimensions,
            self.position,
            self.forward(),
            max_distance,
        );
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
    use core::f32;

    use crate::game::world::empty_world;

    use super::*;
    use glam::Vec4;
    const PI: f32 = core::f32::consts::PI;

    #[test]
    fn forward_vectors() {
        let mut camera = Camera::default();
        let forward = camera.forward();
        assert_abs_diff_eq!(forward.x, 0.0);
        assert_abs_diff_eq!(forward.y, 0.0);
        assert_abs_diff_eq!(forward.z, 1.0);

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

        let mut world = empty_world();
        world[1][1][0] = true;

        let result = camera.looking_at_solid_block(&world, f32::INFINITY);
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

        let world = empty_world();

        let result = camera.looking_at_solid_block(&world, f32::INFINITY);
        assert!(result.is_none(), "Should not hit any block");
    }

    #[test]
    fn looking_at_solid_block_max_distance_out_of_reach() {
        let mut camera = Camera::default();
        camera.set_position(1.5, 1.5, -0.5);

        let mut world = empty_world();
        world[1][1][1] = true;

        let result = camera.looking_at_solid_block(&world, 1.4);
        assert!(
            result.is_none(),
            "Block at z=1 should be out of reach with max_distance=1.4"
        );
    }

    #[test]
    fn looking_at_solid_block_max_distance_just_within_reach() {
        let mut camera = Camera::default();
        camera.set_position(1.5, 1.5, -0.5);

        let mut world = empty_world();
        world[1][1][1] = true;

        let result = camera.looking_at_solid_block(&world, 1.51);
        assert!(
            result.is_some(),
            "Block at z=1 should be within reach with max_distance=1.5"
        );

        let (pos, face) = result.unwrap();
        assert_eq!(pos.x, 1);
        assert_eq!(pos.y, 1);
        assert_eq!(pos.z, 1);
        assert_eq!(face, Face::BACK);
    }

    #[test]
    fn looking_at_solid_block_closest() {
        let mut camera = Camera::default();
        camera.set_position(1.5, 1.5, -0.5);

        let mut world = empty_world();
        world[1][1][0] = true;
        world[2][2][1] = true;

        let result = camera.looking_at_solid_block(&world, f32::INFINITY);
        assert!(result.is_some(), "Should hit solid block");

        let (pos, face) = result.unwrap();
        assert_eq!(pos.x, 1);
        assert_eq!(pos.y, 1);
        assert_eq!(pos.z, 0, "Should return closest block, not further one");
        assert_eq!(face, Face::BACK);
    }

    #[test]
    fn ray_exits_world_returns_none() {
        let mut camera = Camera::default();
        camera.set_position(0.5, 0.5, 5.0);

        let world = empty_world();

        let result = camera.looking_at_solid_block(&world, f32::INFINITY);
        assert!(
            result.is_none(),
            "Should not hit any block - looking away from world"
        );
    }

    #[test]
    fn looking_from_world_boundary() {
        let mut camera = Camera::default();
        camera.set_position(0.5, 0.5, 4.0);

        let world = empty_world();

        let result = camera.looking_at_solid_block(&world, f32::INFINITY);
        assert!(result.is_none(), "Camera at boundary shouldn't count");
    }

    #[test]
    fn looking_with_neg_x_dir() {
        let mut camera = Camera::default();
        camera.set_position(5.0, 1.5, 1.5);
        camera.yaw = PI / 2.0; // -X

        let forward = camera.forward();
        let mut world = empty_world();
        world[3][1][1] = true;

        let result = camera.looking_at_solid_block(&world, f32::INFINITY);
        assert!(
            result.is_some(),
            "forward=({},{},{})",
            forward.x,
            forward.y,
            forward.z
        );

        let (pos, face) = result.unwrap();
        assert_eq!(pos.x, 3);
        assert_eq!(pos.y, 1);
        assert_eq!(pos.z, 1);
        assert_eq!(face, Face::RIGHT);
    }

    #[test]
    fn looking_at_air_not_nearby_solid() {
        let mut camera = Camera::default();
        camera.set_position(1.5, 1.5, 4.5);
        camera.yaw = PI; // -Z

        let mut world = empty_world();
        world[1][0][3] = true;

        let result = camera.looking_at_solid_block(&world, f32::INFINITY);
        assert!(
            result.is_none(),
            "Should not hit block, ray passes through air at y=1"
        );
    }

    #[test]
    fn camera_inside_solid_block() {
        let mut camera = Camera::default();
        camera.set_position(0.5, 0.5, 0.5);
        let mut world = empty_world();
        world[0][0][0] = true;
        let result = camera.looking_at_solid_block(&world, f32::INFINITY);
        assert!(result.is_some(), "Should hit block camera is inside");
        let (pos, _) = result.unwrap();
        assert_eq!(pos.x, 0);
        assert_eq!(pos.y, 0);
        assert_eq!(pos.z, 0);
    }

    #[test]
    fn looking_corner_to_corner() {
        let mut camera = Camera::default();
        camera.set_position(0.0, 0.0, 0.0);
        camera.yaw = -PI / 4.0;
        camera.pitch = libm::atanf(1.0 / libm::sqrtf(2.0));
        let mut world = empty_world();
        world[3][3][3] = true;
        let result = camera.looking_at_solid_block(&world, f32::INFINITY);
        assert!(result.is_some(), "Should hit the other corner");
        let (pos, _) = result.unwrap();
        assert_eq!(pos.x, 3);
        assert_eq!(pos.y, 3);
        assert_eq!(pos.z, 3);
    }
}
