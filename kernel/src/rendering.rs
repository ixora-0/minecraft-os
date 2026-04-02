extern crate alloc;

use alloc::{vec, vec::Vec};
use bootloader_api::info::{FrameBuffer, FrameBufferInfo};
use kernel_core::rendering::Frame;
use spin::{Mutex, Once};
use x86_64::instructions::interrupts;

pub const EXPECT_MSG_FRAMEBUFFER_NOT_INITIALIZED: &str =
    "Global framebuffer not initialized. Probably haven't run init_framebuffer()";

pub struct GlobalRenderer {
    framebuffer: &'static mut FrameBuffer,
    staging: Vec<u8>,
    depth: Vec<f32>,
    info: FrameBufferInfo,
}

impl GlobalRenderer {
    fn new(framebuffer: &'static mut FrameBuffer) -> Self {
        let info = framebuffer.info();
        let staging = vec![0u8; info.byte_len];
        let depth = vec![0.0f32; info.width * info.height];
        Self {
            framebuffer,
            staging,
            depth,
            info,
        }
    }

    pub fn info(&self) -> FrameBufferInfo {
        self.info
    }

    pub fn frame(&mut self) -> Frame<'_> {
        Frame::new(
            self.staging.as_mut_slice(),
            self.depth.as_mut_slice(),
            self.info,
        )
    }

    pub fn flush(&mut self) {
        let dst = self.framebuffer.buffer_mut();
        debug_assert_eq!(dst.len(), self.staging.len());
        dst.copy_from_slice(&self.staging);
    }
}

pub static GLOBAL_RENDERER: Mutex<Once<GlobalRenderer>> = Mutex::new(Once::new());

pub fn init_global_renderer(framebuffer: &'static mut FrameBuffer) {
    GLOBAL_RENDERER
        .lock()
        .call_once(|| GlobalRenderer::new(framebuffer));
}

pub fn with_global_renderer<F, R>(f: F) -> R
where
    F: FnOnce(&GlobalRenderer) -> R,
{
    interrupts::without_interrupts(|| {
        let renderer_guard = GLOBAL_RENDERER.lock();
        let renderer = renderer_guard
            .get()
            .expect(EXPECT_MSG_FRAMEBUFFER_NOT_INITIALIZED);
        f(renderer)
    })
}

pub fn with_global_renderer_mut<F, R>(f: F) -> R
where
    F: FnOnce(&mut GlobalRenderer) -> R,
{
    interrupts::without_interrupts(|| {
        let mut renderer_guard = GLOBAL_RENDERER.lock();
        let renderer = renderer_guard
            .get_mut()
            .expect(EXPECT_MSG_FRAMEBUFFER_NOT_INITIALIZED);
        f(renderer)
    })
}

pub use kernel_core::rendering::TextBox;
