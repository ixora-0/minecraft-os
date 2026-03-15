extern crate alloc;
use alloc::vec::Vec;
use fontdue::{Font, Metrics};

use crate::rendering::Renderer;
use embedded_graphics::{Pixel, draw_target::DrawTarget, geometry::Point, primitives::Rectangle};
use spin::Once;

/// Using ascii only font to hopefully make booting faster
static FONT_DATA: &[u8] = include_bytes!("../../../assets/ascii.ttf");
static FONT: Once<Font> = Once::new();
pub const FONT_SIZE: f32 = 14.0;

#[derive(Clone, Copy)]
pub struct TextBoxConfig {
    /// number of pixels to adjust the line gap
    pub line_spacing: i32,

    pub letter_spacing: i32,
    pub padding_left: i32,
    pub padding_right: i32,
    pub padding_bottom: i32,
    pub padding_top: i32,
    pub color_fg: super::Color,
    pub color_bg: super::Color,
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
            color_fg: super::Color::WHITE,
            color_bg: super::Color::BLACK,
        }
    }
}

fn get_glyph(c: char) -> (Metrics, Vec<u8>) {
    // TODO: cache
    let font = get_font();
    font.rasterize(c, FONT_SIZE)
}

fn get_font() -> &'static Font {
    match FONT.is_completed() {
        true => unsafe { FONT.get_unchecked() },
        false => FONT.call_once(|| {
            Font::from_bytes(FONT_DATA, fontdue::FontSettings::default())
                .expect("Could not load font")
        }),
    }
}

pub struct TextBox {
    bounding_box: Rectangle,
    config: TextBoxConfig,
    /// Relative x of the cursor within the bounding box.
    cursor_x: i32,
    /// Relative y of the cursor within the bounding box.
    /// Y value of baseline of current line.
    cursor_y: i32,

    /// Difference between two baseline of two neighboring lines.
    line_height: i32,

    line_ascent: i32,
}
impl TextBox {
    pub fn new(bounding_box: Rectangle) -> Self {
        let config = TextBoxConfig::default();
        let font = get_font();
        let line_metrics = font
            .horizontal_line_metrics(FONT_SIZE)
            .expect("Could not get font metrics");

        Self {
            bounding_box,
            config,
            cursor_x: config.padding_left,
            cursor_y: config.padding_top + line_metrics.ascent as i32,
            line_height: line_metrics.new_line_size as i32 + config.line_spacing,
            line_ascent: line_metrics.ascent as i32,
        }
    }
    fn newline(&mut self) {
        self.cursor_y += self.line_height + self.config.line_spacing;
        self.carriage_return()
    }
    fn carriage_return(&mut self) {
        self.cursor_x = self.config.padding_left;
    }
    pub fn clear(&mut self, renderer: &mut Renderer) {
        renderer.fill_solid(&self.bounding_box, self.config.color_bg);
        self.cursor_x = self.config.padding_left;
        self.cursor_y = self.config.padding_top + self.line_ascent;
    }
    pub fn write_char(&mut self, c: char, renderer: &mut Renderer) {
        match c {
            '\n' => self.newline(),
            '\r' => self.carriage_return(),
            c => {
                let (metrics, bitmap) = get_glyph(c);
                let advance_x = libm::ceilf(metrics.advance_width) as i32;
                let next_cursor_x = self.cursor_x + advance_x;
                if next_cursor_x >= self.bounding_box.size.width as i32 {
                    self.newline();
                }

                let line_bottom = self.cursor_y - metrics.ymin; // +y is upwards in metrics
                if line_bottom >= self.bounding_box.size.height as i32 {
                    self.clear(renderer);
                }

                self.render_char(metrics, bitmap, renderer);
                self.cursor_x += advance_x + self.config.letter_spacing;
            }
        }
    }

    fn render_char(&self, metrics: Metrics, bitmap: Vec<u8>, renderer: &mut Renderer) {
        let tx = self.bounding_box.top_left.x as i32 + self.cursor_x + metrics.xmin;
        let ty = self.bounding_box.top_left.y as i32 + self.cursor_y
            - metrics.height as i32 // bitmap starts from top left
            - metrics.ymin; // +y is upwards in metrics
        let color = self.config.color_fg;

        let pixels = bitmap.iter().enumerate().flat_map(|(i, intensity)| {
            let x = i % metrics.width;
            let y = i / metrics.width;
            if *intensity != 0 {
                Some(Pixel(
                    Point::new(tx + x as i32, ty + y as i32),
                    color.with_intensity(*intensity),
                ))
            } else {
                None
            }
        });
        renderer.draw_iter(pixels);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loading_fonts() {
        use embedded_graphics::prelude::Size;
        let font = get_font();
        let _line_metrics = font
            .horizontal_line_metrics(FONT_SIZE)
            .expect("Could not get font metrics");
        let _text_box = TextBox::new(Rectangle::new(Point::new(0, 0), Size::new(100, 100)));
    }
}
