extern crate alloc;

use alloc::string::String;
use core::fmt::{self, Write};

use glam::{IVec2, USizeVec2, USizeVec3, Vec3};
use kernel_core::{
    game::{Camera, Face},
    rendering::{Frame, Rectangle, TextBox},
};

use crate::allocator::{self, ALLOCATOR};

const FONT_SIZE: u32 = 14;
const MAX_LINE_LEN: usize = 44;
const NUM_LINES: usize = 8;
/// How often the displayed FPS is recomputed.
const FPS_SAMPLE_NS: u64 = 500_000_000;

/// Per-frame values the overlay displays.
pub struct DebugInfo<'a> {
    pub camera: &'a Camera,
    pub targeted_block: Option<(USizeVec3, Face)>,
    pub triangle_count: usize,
    pub uptime_ns: u64,
}

pub struct DebugOverlay {
    text_box: TextBox,
    text: String,
    visible: bool,
    fps: f32,
    frame_time_ms: f32,
    sample_start_ns: u64,
    sample_frames: u32,
}

impl DebugOverlay {
    pub fn new(top_left: IVec2) -> Self {
        let mut text_box = TextBox::new(Rectangle {
            top_left,
            size: USizeVec2::new(1, 1),
        });
        text_box.set_font_size(FONT_SIZE);
        let size = text_box
            .min_dimensions(MAX_LINE_LEN, NUM_LINES)
            .as_usizevec2();
        let mut text_box = TextBox::new(Rectangle { top_left, size });
        text_box.set_font_size(FONT_SIZE);
        Self {
            text_box,
            text: String::new(),
            visible: false,
            fps: 0.0,
            frame_time_ms: 0.0,
            sample_start_ns: 0,
            sample_frames: 0,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn toggle_visible(&mut self) {
        self.visible = !self.visible;
    }

    /// Call once per frame, even while hidden, used to calculate FPS.
    pub fn record_frame(&mut self, now_ns: u64) {
        self.sample_frames += 1;
        let elapsed = now_ns - self.sample_start_ns;
        if elapsed >= FPS_SAMPLE_NS {
            self.fps = self.sample_frames as f32 * 1e9 / elapsed as f32;
            self.frame_time_ms = elapsed as f32 / 1e6 / self.sample_frames as f32;
            self.sample_start_ns = now_ns;
            self.sample_frames = 0;
        }
    }

    pub fn render(&mut self, frame: &mut Frame, info: &DebugInfo) {
        if !self.visible {
            return;
        }
        self.update_text(info);
        self.text_box.set_text(&self.text);
        let mut renderer = frame.renderer2d();
        self.text_box.render(&mut renderer);
    }

    fn update_text(&mut self, info: &DebugInfo) {
        self.text.clear();
        // writing to a String can't fail
        write_info(&mut self.text, self.fps, self.frame_time_ms, info).unwrap();
    }
}

fn write_info(t: &mut String, fps: f32, frame_time_ms: f32, info: &DebugInfo) -> fmt::Result {
    let camera = info.camera;
    let pos = camera.position;
    let block = pos.floor();
    let yaw = wrap_degrees(camera.yaw.to_degrees());
    let pitch = camera.pitch.to_degrees();
    let heap_used = ALLOCATOR.get_allocated_bytes();
    let heap_total = allocator::HEAP_SIZE;
    let uptime_s = info.uptime_ns / 1_000_000_000;

    writeln!(t, "FPS: {:.0} ({:.1} ms)", fps, frame_time_ms)?;
    writeln!(t, "XYZ: {:.2} / {:.2} / {:.2}", pos.x, pos.y, pos.z)?;
    writeln!(t, "Block: {} {} {}", block.x, block.y, block.z)?;
    writeln!(
        t,
        "Facing: {} (yaw {:.1}, pitch {:.1})",
        facing(camera.forward()),
        yaw,
        pitch
    )?;
    match info.targeted_block {
        Some((b, face)) => writeln!(t, "Target: {} {} {} ({:?})", b.x, b.y, b.z, face)?,
        None => writeln!(t, "Target: none")?,
    }
    writeln!(t, "Triangles: {}", info.triangle_count)?;
    writeln!(
        t,
        "Heap: {} / {} KiB ({}%)",
        heap_used / 1024,
        heap_total / 1024,
        heap_used * 100 / heap_total
    )?;
    write!(
        t,
        "Uptime: {}:{:02}:{:02}",
        uptime_s / 3600,
        uptime_s / 60 % 60,
        uptime_s % 60
    )
}

/// Wraps an angle into [0, 360).
fn wrap_degrees(deg: f32) -> f32 {
    let deg = deg % 360.0;
    if deg < 0.0 { deg + 360.0 } else { deg }
}

/// Dominant horizontal axis of a direction.
fn facing(dir: Vec3) -> &'static str {
    if dir.x.abs() > dir.z.abs() {
        if dir.x > 0.0 { "+X" } else { "-X" }
    } else if dir.z > 0.0 {
        "+Z"
    } else {
        "-Z"
    }
}
