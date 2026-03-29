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
