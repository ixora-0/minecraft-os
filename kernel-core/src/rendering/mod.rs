pub mod text_box;
pub use text_box::{TextBox, TextBoxConfig};
pub mod renderer;
pub use renderer::{Pixel, Rectangle, Renderer};
pub mod color;
pub use color::Color;

use bootloader_api::info::FrameBufferInfo;
use renderer::Renderer3d;

/// Shared staging view for a single draw pass.
///
/// A `Frame` borrows the global staging color buffer and its depth buffer, plus
/// carries the framebuffer metadata. Use `renderer()` for 2D drawing, `renderer3d()`
/// for 3D primitives, `clear_color()` / `clear_depth()` to reset the staging data,
/// and then return to the owning `GlobalRenderer` to `flush()` the finished frame.
pub struct Frame<'a> {
    color: &'a mut [u8],
    depth: &'a mut [f32],
    info: FrameBufferInfo,
}

impl<'a> Frame<'a> {
    pub fn new(color: &'a mut [u8], depth: &'a mut [f32], info: FrameBufferInfo) -> Self {
        Self { color, depth, info }
    }

    pub fn info(&self) -> FrameBufferInfo {
        self.info
    }

    pub fn renderer2d(&mut self) -> Renderer<'_> {
        Renderer::new(&mut *self.color, self.info)
    }

    pub fn renderer3d(&mut self) -> Renderer3d<'_> {
        Renderer3d::new(&mut *self.color, &mut *self.depth, self.info)
    }

    pub fn clear_color(&mut self, color: Color) {
        let mut renderer = Renderer::new(&mut *self.color, self.info);
        renderer.clear(color);
    }

    pub fn clear_depth(&mut self) {
        self.depth.fill(0.0);
    }
}
