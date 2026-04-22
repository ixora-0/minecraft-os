use super::camera::Camera;
use super::world::{World, empty_world};
use glam::Vec3;

const MAGIC: &[u8; 4] = b"MCOS";
const VERSION: u16 = 1;

#[derive(Debug, PartialEq)]
pub enum SaveError {
    InvalidMagic,
    VersionMismatch,
}

/// Serialize world and camera state into a 1024-byte buffer.
///
/// Layout:
/// - 0..4:    magic "MCOS"
/// - 4..6:    version u16 LE
/// - 6..8:    reserved
/// - 8..20:   camera position (3× f32 LE)
/// - 20..24:  yaw f32 LE
/// - 24..28:  pitch f32 LE
/// - 28..32:  v_fov f32 LE
/// - 32..544: world blocks (8×8×8 = 512 bytes, 1 byte per bool)
/// - 544..1024: reserved
pub fn serialize(world: &World, camera: &Camera) -> [u8; 1024] {
    let mut buf = [0u8; 1024];

    // Magic
    buf[0..4].copy_from_slice(MAGIC);

    // Version
    buf[4..6].copy_from_slice(&VERSION.to_le_bytes());

    // Camera position
    buf[8..12].copy_from_slice(&camera.position.x.to_le_bytes());
    buf[12..16].copy_from_slice(&camera.position.y.to_le_bytes());
    buf[16..20].copy_from_slice(&camera.position.z.to_le_bytes());

    // Camera rotation
    buf[20..24].copy_from_slice(&camera.yaw.to_le_bytes());
    buf[24..28].copy_from_slice(&camera.pitch.to_le_bytes());
    buf[28..32].copy_from_slice(&camera.v_fov.to_le_bytes());

    // World blocks
    {
        let mut offset = 32;
        for plane in world {
            for row in plane {
                for &block in row {
                    buf[offset] = if block { 1 } else { 0 };
                    offset += 1;
                }
            }
        }
    }

    buf
}

/// Deserialize world and camera state from a 1024-byte buffer.
pub fn deserialize(buf: &[u8; 1024]) -> Result<(World, Camera), SaveError> {
    // Validate magic
    if &buf[0..4] != MAGIC {
        return Err(SaveError::InvalidMagic);
    }

    // Validate version
    let version = u16::from_le_bytes([buf[4], buf[5]]);
    if version != VERSION {
        return Err(SaveError::VersionMismatch);
    }

    // Camera position
    let px = f32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
    let py = f32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);
    let pz = f32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]);

    // Camera rotation
    let yaw = f32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]);
    let pitch = f32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]);
    let v_fov = f32::from_le_bytes([buf[28], buf[29], buf[30], buf[31]]);

    let camera = Camera {
        position: Vec3::new(px, py, pz),
        yaw,
        pitch,
        v_fov,
    };

    // World blocks
    let mut world = empty_world();
    let mut offset = 32;
    for plane in &mut world {
        for row in plane {
            for block in row {
                *block = buf[offset] != 0;
                offset += 1;
            }
        }
    }

    Ok((world, camera))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_empty_world() {
        let world = empty_world();
        let camera = Camera::default();
        let buf = serialize(&world, &camera);
        let (w2, c2) = deserialize(&buf).unwrap();
        assert_eq!(world, w2);
        assert_eq!(c2.position, camera.position);
        assert_eq!(c2.yaw, camera.yaw);
        assert_eq!(c2.pitch, camera.pitch);
        assert_eq!(c2.v_fov, camera.v_fov);
    }

    #[test]
    fn round_trip_with_blocks_and_camera() {
        let mut world = empty_world();
        world[0][0][0] = true;
        world[3][5][7] = true;
        world[7][7][7] = true;

        let camera = Camera {
            position: Vec3::new(1.5, -2.3, 4.0),
            yaw: 0.5,
            pitch: -0.3,
            v_fov: 70.0,
        };

        let buf = serialize(&world, &camera);
        let (w2, c2) = deserialize(&buf).unwrap();
        assert_eq!(world, w2);
        assert_eq!(c2.position, camera.position);
        assert_eq!(c2.yaw, camera.yaw);
        assert_eq!(c2.pitch, camera.pitch);
        assert_eq!(c2.v_fov, camera.v_fov);
    }

    #[test]
    fn invalid_magic() {
        let mut buf = [0u8; 1024];
        buf[0..4].copy_from_slice(b"XXXX");
        assert!(matches!(deserialize(&buf), Err(SaveError::InvalidMagic)));
    }

    #[test]
    fn unsupported_version() {
        let mut buf = [0u8; 1024];
        buf[0..4].copy_from_slice(b"MCOS");
        buf[4..6].copy_from_slice(&99u16.to_le_bytes());
        assert!(matches!(deserialize(&buf), Err(SaveError::VersionMismatch)));
    }
}
