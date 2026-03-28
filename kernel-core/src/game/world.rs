use super::Triangle;
use alloc::vec::Vec;
use glam::Vec3;
use spin::{Lazy, Mutex};

// World dimensions in blocks (x, y, z)
const WORLD_X: usize = 4;
const WORLD_Y: usize = 4;
const WORLD_Z: usize = 4;

/// Global world blocks storage.
/// Indexed as [x][y][z], where true = solid block, false = air.
pub static WORLD: Lazy<Mutex<[[[bool; WORLD_Z]; WORLD_Y]; WORLD_X]>> = Lazy::new(|| {
    // initialize with a 4-layer thick floor of solid blocks.
    let mut w = [[[false; WORLD_Z]; WORLD_Y]; WORLD_X];
    for y in 0..2 {
        for x in 0..WORLD_Y {
            for z in 0..WORLD_X {
                w[x][y][z] = true;
            }
        }
    }
    w[0][2][0] = true;
    Mutex::new(w)
});

/// Represents one of the 6 cardinal directions of a cube face.
/// Stores the direction offset and the 4 corner offsets for a face's quad.
#[derive(Clone, Copy)]
struct FaceDir {
    /// Direction offset from block center to adjacent block (e.g., [0,0,1] = +Z face)
    offset: [isize; 3],
    /// The 4 corner offsets (as [x,y,z] in 0.0/1.0 units) for this face's quad.
    /// Clockwise winding order when normal points at us.
    quad: [[f32; 3]; 4],
}

/// The 6 faces of a cube, ordered: +Z, -Z, +Y, -Y, +X, -X
const FACES: [FaceDir; 6] = [
    // +Z face (front)
    FaceDir {
        offset: [0, 0, 1],
        quad: [
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 0.0, 1.0],
        ],
    },
    // -Z face (back)
    FaceDir {
        offset: [0, 0, -1],
        quad: [
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0],
        ],
    },
    // +Y face (top)
    FaceDir {
        offset: [0, 1, 0],
        quad: [
            [0.0, 1.0, 1.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
        ],
    },
    // -Y face (bottom)
    FaceDir {
        offset: [0, -1, 0],
        quad: [
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
        ],
    },
    // +X face (right)
    FaceDir {
        offset: [1, 0, 0],
        quad: [
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
        ],
    },
    // -X face (left)
    FaceDir {
        offset: [-1, 0, 0],
        quad: [
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 1.0],
            [0.0, 0.0, 1.0],
        ],
    },
];

/// Generates a triangle mesh from the block world data.
/// Uses face culling, so only return faces that are adjacent to air (empty space).
pub fn get_world_mesh(world: &[[[bool; WORLD_Z]; WORLD_Y]; WORLD_X]) -> Vec<Triangle> {
    let mut mesh = Vec::new();
    for z in 0..WORLD_Z {
        for y in 0..WORLD_Y {
            for x in 0..WORLD_X {
                if !world[x][y][z] {
                    continue;
                }

                for face in &FACES {
                    // check if neighbor is a solid block (for face culling)
                    // out-of-bounds coordinates are treated as air (exposed face)
                    let [ox, oy, oz] = face.offset;
                    let nx = x as isize + ox;
                    let ny = y as isize + oy;
                    let nz = z as isize + oz;
                    let neighbor_solid = nx >= 0
                        && nx < WORLD_X as isize
                        && ny >= 0
                        && ny < WORLD_Y as isize
                        && nz >= 0
                        && nz < WORLD_Z as isize
                        && world[nx as usize][ny as usize][nz as usize];

                    // skip this face if neighbor is solid (occluded)
                    if neighbor_solid {
                        continue;
                    }

                    // build quad from the face definition
                    let [q0, q1, q2, q3] = face.quad;
                    let fx = x as f32;
                    let fy = y as f32;
                    let fz = z as f32;
                    // relative quad corners to world coordinates
                    let v = |q: [f32; 3]| Vec3::new(fx + q[0], fy + q[1], fz + q[2]);
                    let (v0, v1, v2, v3) = (v(q0), v(q1), v(q2), v(q3));

                    let normal = Vec3::new(ox as f32, oy as f32, oz as f32);

                    // split quad into two triangles
                    mesh.push(Triangle { v0, v1, v2, normal });
                    mesh.push(Triangle {
                        v0: v2,
                        v1: v3,
                        v2: v0,
                        normal,
                    });
                }
            }
        }
    }

    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates an empty 8x8x8 world filled with air
    fn empty_world() -> [[[bool; WORLD_Z]; WORLD_Y]; WORLD_X] {
        [[[false; WORLD_Z]; WORLD_Y]; WORLD_X]
    }

    /// Count triangles with a given normal direction
    fn tris_with_normal(mesh: &[Triangle], normal: Vec3) -> usize {
        mesh.iter().filter(|t| t.normal == normal).count()
    }

    #[test]
    fn single_block_has_12_triangles() {
        let mut world = empty_world();
        world[1][1][1] = true;
        let mesh = get_world_mesh(&world);
        assert_eq!(mesh.len(), 12);
    }

    #[test]
    fn empty_world_produces_no_triangles() {
        let world = empty_world();
        let mesh = get_world_mesh(&world);
        assert!(mesh.is_empty());
    }

    #[test]
    fn single_block_two_tris_per_face() {
        let mut world = empty_world();
        world[1][1][1] = true;
        let mesh = get_world_mesh(&world);

        for face in &FACES {
            let [ox, oy, oz] = face.offset;
            let n = Vec3::new(ox as f32, oy as f32, oz as f32);
            assert_eq!(
                tris_with_normal(mesh.as_slice(), n),
                2,
                "expected 2 triangles for normal {:?}",
                n
            );
        }
    }

    // two adjacent blocks: the shared face is culled -> 10 faces exposed = 20 triangles
    #[test]
    fn two_adjacent_blocks_cull_shared_face() {
        let mut world = empty_world();
        world[1][1][1] = true;
        world[2][1][1] = true; // neighbor in +x direction
        let mesh = get_world_mesh(&world);
        assert_eq!(mesh.len(), 20);
    }

    // 6 sides × 4 faces each = 24 exposed faces -> 48 triangles
    #[test]
    fn solid_2x2x2_cube() {
        let mut world = empty_world();
        for x in 0..2 {
            for y in 0..2 {
                for z in 0..2 {
                    world[x][y][z] = true;
                }
            }
        }
        let mesh = get_world_mesh(&world);
        assert_eq!(mesh.len(), 48);
    }

    // a block at the world border still gets its out-of-bounds faces rendered
    #[test]
    fn border_block_faces_are_exposed() {
        let mut world = empty_world();
        world[0][0][0] = true; // corner block
        let mesh = get_world_mesh(&world);
        assert_eq!(mesh.len(), 12);
    }

    // all triangles for a single block are within [x, x+1] × [y, y+1] × [z, z+1]
    #[test]
    fn triangle_vertices_within_block_bounds() {
        let mut world = empty_world();
        world[2][3][2] = true;
        let mesh = get_world_mesh(&world);

        for tri in &mesh {
            for v in [tri.v0, tri.v1, tri.v2] {
                assert!((2.0..=3.0).contains(&v.x), "x out of range: {}", v.x);
                assert!((3.0..=4.0).contains(&v.y), "y out of range: {}", v.y);
                assert!((2.0..=3.0).contains(&v.z), "z out of range: {}", v.z);
            }
        }
    }

    #[test]
    fn all_normals_are_unit_length() {
        let mut world = empty_world();
        world[1][1][1] = true;
        let mesh = get_world_mesh(&world);

        for tri in &mesh {
            let len = (tri.normal.x.powi(2) + tri.normal.y.powi(2) + tri.normal.z.powi(2)).sqrt();
            assert_abs_diff_eq!(len, 1.0);
        }
    }

    // two triangles making a quad should not be degenerate (non-zero area)
    #[test]
    fn no_degenerate_triangles() {
        let mut world = empty_world();
        world[1][1][1] = true;
        let mesh = get_world_mesh(&world);

        for tri in &mesh {
            let edge1 = tri.v1 - tri.v0;
            let edge2 = tri.v2 - tri.v0;
            let cross = edge1.cross(edge2);
            let area = cross.length();
            assert!(area > 1e-6, "degenerate triangle with near-zero area");
        }
    }
}
