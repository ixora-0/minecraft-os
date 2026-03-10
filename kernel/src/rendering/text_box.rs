use core::fmt;

use noto_sans_mono_bitmap::RasterizedChar;

use crate::{
    rendering::{EXPECT_MSG_FRAMEBUFFER_NOT_INITIALIZED, GLOBAL_RENDERER, Renderer},
    serial_print,
};
use embedded_graphics::{Pixel, draw_target::DrawTarget, geometry::Point, primitives::Rectangle};

/// Constants for the usage of the [`noto_sans_mono_bitmap`] crate.
/// Need to pre determined compile time. No adjustable font size for now.
/// Also defined in features in Cargo.toml.
mod font_constants {
    use noto_sans_mono_bitmap::{FontWeight, RasterHeight, get_raster_width};
    pub const CHAR_RASTER_HEIGHT: RasterHeight = RasterHeight::Size16;
    pub const CHAR_RASTER_WIDTH: usize = get_raster_width(FontWeight::Regular, CHAR_RASTER_HEIGHT);
    pub const BACKUP_CHAR: char = '�';
    pub const FONT_WEIGHT: FontWeight = FontWeight::Regular;
}

fn get_raster(c: char) -> RasterizedChar {
    fn get(c: char) -> Option<RasterizedChar> {
        noto_sans_mono_bitmap::get_raster(
            c,
            font_constants::FONT_WEIGHT,
            font_constants::CHAR_RASTER_HEIGHT,
        )
    }
    get(c).unwrap_or_else(|| {
        get(font_constants::BACKUP_CHAR).expect("Should be able to get raster of backup char.")
    })
}

#[derive(Clone, Copy)]
pub struct TextBoxConfig {
    pub line_spacing: i32,
    pub letter_spacing: i32,
    pub padding_left: i32,
    pub padding_right: i32,
    pub padding_bottom: i32,
    pub padding_top: i32,
    pub fg_color: super::Color,
    pub bg_color: super::Color,
}
impl TextBoxConfig {
    pub fn default() -> Self {
        Self {
            line_spacing: 0,
            letter_spacing: 0,
            padding_left: 10,
            padding_right: 10,
            padding_bottom: 10,
            padding_top: 10,
            fg_color: super::Color::WHITE,
            bg_color: super::Color::BLACK,
        }
    }
}
pub struct TextBox {
    bounding_box: Rectangle,
    /// Whether to log to serial port as well.
    serial: bool,
    config: TextBoxConfig,
    /// Relative x of the cursor within the bounding box.
    cursor_x: i32,
    /// Relative y of the cursor within the bounding box.
    cursor_y: i32,
}
impl TextBox {
    pub fn new(bounding_box: Rectangle, serial: bool) -> Self {
        let config = TextBoxConfig::default();
        Self {
            bounding_box,
            config,
            serial,
            cursor_x: config.padding_left,
            cursor_y: config.padding_top,
        }
    }
    fn newline(&mut self) {
        self.cursor_y += font_constants::CHAR_RASTER_HEIGHT.val() as i32 + self.config.line_spacing;
        self.carriage_return()
    }
    fn carriage_return(&mut self) {
        self.cursor_x = self.config.padding_left;
    }
    pub fn clear(&mut self, renderer: &mut Renderer) {
        // renderer.draw_rectangle(self.bounding_box, self.config.bg_color);
        renderer.fill_solid(&self.bounding_box, self.config.bg_color);
        self.cursor_x = self.config.padding_left;
        self.cursor_y = self.config.padding_top;
    }
    fn write_char(&mut self, c: char, renderer: &mut Renderer) {
        match c {
            '\n' => self.newline(),
            '\r' => self.carriage_return(),
            c => {
                if (self.cursor_x + font_constants::CHAR_RASTER_WIDTH as i32)
                    > self.bounding_box.size.width as i32
                {
                    self.newline();
                }
                if (self.cursor_y + font_constants::CHAR_RASTER_HEIGHT.val() as i32)
                    > self.bounding_box.size.height as i32
                {
                    self.clear(renderer);
                }
                self.render_char(c, renderer);
                self.cursor_x +=
                    font_constants::CHAR_RASTER_WIDTH as i32 + self.config.letter_spacing;
            }
        }
    }

    /// Renders a character at the current cursor position.
    fn render_char(&mut self, c: char, renderer: &mut Renderer) {
        let tx = self.bounding_box.top_left.x as i32 + self.cursor_x;
        let ty = self.bounding_box.top_left.y as i32 + self.cursor_y;
        let color = self.config.fg_color;

        let raster = get_raster(c);
        let pixels = raster.raster().iter().enumerate().flat_map(|(y, row)| {
            row.iter()
                .enumerate()
                .filter(|(_x, intensity)| **intensity != 0)
                .map(move |(x, intensity)| {
                    Pixel(
                        Point::new(tx + x as i32, ty + y as i32),
                        color.with_intensity(*intensity),
                    )
                })
        });
        renderer.draw_iter(pixels);
    }
}
impl fmt::Write for TextBox {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if self.serial {
            serial_print!("{}", s);
        }
        let mut renderer_guard = GLOBAL_RENDERER.lock();
        let mut renderer = renderer_guard
            .get_mut()
            .expect(EXPECT_MSG_FRAMEBUFFER_NOT_INITIALIZED);
        for c in s.chars() {
            self.write_char(c, &mut renderer);
        }
        Ok(())
    }
}
