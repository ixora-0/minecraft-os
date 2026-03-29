use bootloader_api::info::FrameBuffer;
use kernel_core::rendering::Renderer;
use spin::{Mutex, Once};

mod text_box;
pub use text_box::TextBox;

pub static GLOBAL_RENDERER: Mutex<Once<Renderer>> = Mutex::new(Once::new());
pub const EXPECT_MSG_FRAMEBUFFER_NOT_INITIALIZED: &str =
    "Global framebuffer not initialized. Probably haven't run init_framebuffer()";

pub fn init_global_renderer(framebuffer: &'static mut FrameBuffer) {
    GLOBAL_RENDERER
        .lock()
        .call_once(|| Renderer::from_framebuffer(framebuffer));
}

pub fn with_global_renderer<F, R>(f: F) -> R
where
    F: FnOnce(&Renderer) -> R,
{
    let renderer_guard = GLOBAL_RENDERER.lock();
    let renderer = renderer_guard
        .get()
        .expect(EXPECT_MSG_FRAMEBUFFER_NOT_INITIALIZED);
    f(renderer)
}

pub fn with_global_renderer_mut<F, R>(f: F) -> R
where
    F: FnOnce(&mut Renderer) -> R,
{
    let mut renderer_guard = GLOBAL_RENDERER.lock();
    let renderer = renderer_guard
        .get_mut()
        .expect(EXPECT_MSG_FRAMEBUFFER_NOT_INITIALIZED);
    f(renderer)
}
