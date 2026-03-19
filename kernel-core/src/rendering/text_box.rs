extern crate alloc;
use core::{cmp::min, ops::Index, ops::Range};

use super::Color;
use alloc::{sync::Arc, vec::Vec};
use fontdue::{Font, Metrics};
use hashbrown::HashMap;

use crate::rendering::Renderer;
use embedded_graphics::{Pixel, draw_target::DrawTarget, geometry::Point, primitives::Rectangle};
use spin::{Lazy, Mutex};

/// Using ascii only font to hopefully make booting faster
static FONT_DATA: &[u8] = include_bytes!("../../../assets/ascii.ttf");
static FONT: Lazy<Font> = Lazy::new(|| {
    Font::from_bytes(FONT_DATA, fontdue::FontSettings::default()).expect("Could not load font")
});

const MAX_CACHE_ENTRIES: usize = 256;
static GLYPH_CACHE: Lazy<Mutex<HashMap<(char, u32), Arc<(Metrics, Vec<u8>)>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
// probably should use linked list or something here. vec for now.
static CACHE_ORDER: Mutex<Vec<(char, u32)>> = Mutex::new(Vec::new());

/// returns metric and bitmap of glyph from FONT
fn get_raster(c: char, size: u32) -> Arc<(Metrics, Vec<u8>)> {
    let size_key = size as u32;
    let mut cache = GLYPH_CACHE.lock();

    // can update order here if we want LRU cache
    if let Some(entry) = cache.get(&(c, size_key)) {
        return Arc::clone(entry);
    }

    let mut order = CACHE_ORDER.lock();
    let entry = FONT.rasterize(c, size as f32);
    if cache.len() >= MAX_CACHE_ENTRIES {
        if let Some(oldest) = order.first().copied() {
            order.remove(0);
            cache.remove(&oldest);
        }
    }

    let entry = Arc::new(entry);
    order.push((c, size_key));
    cache.insert((c, size_key), Arc::clone(&entry));
    entry
}

#[derive(Clone, Copy)]
pub struct TextBoxConfig {
    pub font_size: u32, // not f32 because need to hash
    /// number of pixels to adjust the line gap
    pub line_spacing: i32,

    pub letter_spacing: i32,
    pub padding_left: i32,
    pub padding_right: i32,
    pub padding_bottom: i32,
    pub padding_top: i32,

    /// default text color, can be change through control codes
    pub color_text: super::Color,
    /// background color
    pub color_bg: super::Color,
}
impl TextBoxConfig {
    pub fn default() -> Self {
        Self {
            font_size: 14,
            line_spacing: 0,
            letter_spacing: 0,
            padding_left: 10,
            padding_right: 10,
            padding_bottom: 10,
            padding_top: 10,
            color_text: super::Color::WHITE,
            color_bg: super::Color::BLACK,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Style {
    pub color_text: Color,
}

/// this struct is generated from Parser::push_byte()
/// and we're assuming that that function create a printable glyph.
/// Storing the u8 instead of char to save space.
#[derive(Clone, Copy, Debug)]
pub struct Glyph {
    pub b: u8,
    pub style: Style,
}
impl Glyph {
    pub fn printable_char(&self) -> char {
        return self.b as char;
    }
}

#[derive(Debug)]
pub struct LogicalLine(Vec<Glyph>);
impl Default for LogicalLine {
    fn default() -> Self {
        Self(Vec::new())
    }
}
impl LogicalLine {
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn push(&mut self, glyph: Glyph) {
        self.0.push(glyph);
    }
}
impl Index<Range<usize>> for LogicalLine {
    type Output = [Glyph];

    fn index(&self, range: Range<usize>) -> &Self::Output {
        &self.0[range]
    }
}

#[derive(Debug)]
pub struct VisualLine {
    /// index of logical line in the buffer this visual line is part of
    pub line_idx: usize,
    /// start within logical line
    pub start: usize,
    /// end within logical line (exclusive)
    pub end: usize,
}
impl Default for VisualLine {
    fn default() -> Self {
        Self {
            line_idx: 0,
            start: 0,
            end: 0,
        }
    }
}

fn visual_line_slice<'a>(visual_line: &VisualLine, buffer: &'a Vec<LogicalLine>) -> &'a [Glyph] {
    &buffer[visual_line.line_idx][visual_line.start..visual_line.end]
}

// command format: ESC~<command>BEL
const ESC: u8 = b'\x1B';
const LEADER: u8 = b'~';
const BEL: u8 = b'\x07';

#[derive(Debug)]
pub enum ParserState {
    Text,
    /// after ESC (0x1B)
    Escape,
    /// collecting until BEL (0x07)
    Command,
}
#[derive(Debug)]
pub enum ParserResult {
    PushedGlyph(Glyph),
    NewLogicalLine,
    NoNewGlyph,
}
pub struct Parser {
    pub state: ParserState,
    pub current_style: Style,
    pub cmd_buffer: Vec<u8>,
}
impl Parser {
    pub fn new(initial_style: Style) -> Self {
        Self {
            state: ParserState::Text,
            current_style: initial_style,
            cmd_buffer: Vec::new(),
        }
    }

    fn is_printable(b: u8) -> bool {
        // assuming we're only wanting to print ASCII
        // 0x20 is space, 0x7E is tilde (~)
        (0x20..=0x7E).contains(&b)
    }

    fn push_text(&self, b: u8, buffer: &mut Vec<LogicalLine>) -> ParserResult {
        if !Parser::is_printable(b) {
            return ParserResult::NoNewGlyph;
        }

        // safety check, buffer shouldn't be empty when function is called
        if buffer.len() == 0 {
            buffer.push(LogicalLine::default());
        }

        let glyph = Glyph {
            b,
            style: self.current_style,
        };
        buffer.last_mut().unwrap().push(glyph);
        ParserResult::PushedGlyph(glyph)
    }

    pub fn push_byte(&mut self, b: u8, buffer: &mut Vec<LogicalLine>) -> ParserResult {
        match self.state {
            ParserState::Text => match b {
                ESC => {
                    self.state = ParserState::Escape;
                    ParserResult::NoNewGlyph
                }
                b'\n' => {
                    buffer.push(LogicalLine::default());
                    ParserResult::NewLogicalLine
                }
                _ => self.push_text(b, buffer),
            },
            ParserState::Escape => match b {
                LEADER => {
                    self.state = ParserState::Command;
                    ParserResult::NoNewGlyph
                }
                _ => {
                    self.state = ParserState::Text;
                    self.push_byte(b, buffer)
                }
            },
            ParserState::Command => match b {
                BEL => {
                    self.state = ParserState::Text;
                    self.exec_cmd();
                    ParserResult::NoNewGlyph
                }
                _ => {
                    self.cmd_buffer.push(b);
                    ParserResult::NoNewGlyph
                }
            },
        }
    }

    fn exec_cmd(&mut self) {
        if self.cmd_buffer.len() < 2 {
            log::warn!("Skipping command, too short: {:?}", self.cmd_buffer);
            self.cmd_buffer.clear();
            return;
        }

        let command_type: &[u8] = &self.cmd_buffer[..2];
        let args: &[u8] = &self.cmd_buffer[2..];
        match command_type {
            b"FG" => {
                if let Ok(args_str) = core::str::from_utf8(args) {
                    if let Some(color) = Color::parse_hex(args_str) {
                        self.current_style.color_text = color;
                    } else {
                        log::warn!("Skipping command, invalid color code: {:?}", args_str);
                    }
                } else {
                    log::warn!("Skipping command, args not valid UTF-8: {:?}", args);
                }
            }
            unknown_cmd => log::warn!(
                "Unknown command type: {:?}",
                core::str::from_utf8(unknown_cmd).unwrap_or("<invalid UTF-8>")
            ),
        }
        self.cmd_buffer.clear();
    }
}

pub struct TextBox {
    bounding_box: Rectangle,
    config: TextBoxConfig,
    /// Relative x of currennt line within the bounding box.
    cursor_x: i32,

    /// Difference between two baseline of two neighboring lines.
    line_height: i32,
    /// offset from baseline to top of ascent. typically negative
    line_ascent: i32,
    /// offset from baseline to bottom of descent. typically positive
    line_descent: i32,

    buffer: Vec<LogicalLine>,
    visual_lines: Vec<VisualLine>,
    scroll_offset: usize,
    parser: Parser,
}
impl TextBox {
    pub fn new(bounding_box: Rectangle) -> Self {
        let config = TextBoxConfig::default();
        let mut text_box = Self {
            bounding_box,
            config,
            buffer: alloc::vec![LogicalLine::default()],
            visual_lines: alloc::vec![VisualLine::default()],
            scroll_offset: 0,
            parser: Parser::new(Style {
                color_text: config.color_text,
            }),
            cursor_x: config.padding_left,

            // calculated in recalculate_metrics
            line_height: 0,
            line_ascent: 0,
            line_descent: 0,
        };
        text_box.recalculate_metrics();
        text_box
    }

    fn recalculate_metrics(&mut self) {
        let line_metrics = FONT
            .horizontal_line_metrics(self.config.font_size as f32)
            .expect("Could not get font metrics");

        self.line_height = line_metrics.new_line_size as i32 + self.config.line_spacing;
        self.line_ascent = -line_metrics.ascent as i32; // +y is upwards in metrics
        self.line_descent = -line_metrics.descent as i32; // +y is upwards in metrics
    }

    /// need to call render() to re-render the text box with the new font size
    pub fn set_font_size(&mut self, size: u32) {
        self.config.font_size = size.clamp(4, 72);
        self.scroll_offset = 0;
        self.recalculate_metrics();
    }

    pub fn max_visible_lines(&self) -> usize {
        let effective_height = self.bounding_box.size.height as i32
            - self.config.padding_bottom
            - self.config.padding_top;
        let effective_line_height = self.line_height + self.config.line_spacing;
        (effective_height / effective_line_height) as usize
    }

    pub fn scroll(&mut self, delta: isize) {
        if delta < 0 {
            self.scroll_offset = self.scroll_offset.saturating_add_signed(delta);
        } else {
            let delta = delta as usize;
            let max_scroll_offset = self
                .visual_lines
                .len()
                .saturating_sub(self.max_visible_lines());
            self.scroll_offset = min(self.scroll_offset + delta, max_scroll_offset);
        }
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.scroll_offset = 0;
    }

    pub fn push_byte(&mut self, b: u8) {
        match self.parser.push_byte(b, &mut self.buffer) {
            ParserResult::PushedGlyph(glyph) => self.update_last_line_layout(glyph),
            ParserResult::NewLogicalLine => {
                self.cursor_x = self.config.padding_left;
                self.visual_lines.push(VisualLine {
                    line_idx: self.buffer.len() - 1,
                    start: 0,
                    end: 0,
                })
            }
            ParserResult::NoNewGlyph => {}
        }
    }

    fn calc_advance(&self, metrics: &Metrics) -> i32 {
        (libm::roundf(metrics.advance_width) as i32) + self.config.letter_spacing
    }
    /// update the last visual line based on the last pushed glyph
    /// must be ran after pushing every glyph to keep visual lines accurate
    fn update_last_line_layout(&mut self, pushed_glyph: Glyph) {
        let glyph_data = get_raster(pushed_glyph.printable_char(), self.config.font_size);
        let (metrics, _bitmap) = glyph_data.as_ref();
        let advance = self.calc_advance(metrics);

        // visual lines shouldn't be empty, just unwrap
        let last_visual_line = self.visual_lines.last_mut().unwrap();

        self.cursor_x += advance;
        // let line_end = self.bounding_box.size.width as i32 - self.config.padding_right;
        // let line_end = self.bounding_box.size.width as i32;
        let line_end = 700;
        if self.cursor_x > line_end {
            // need new visual line
            // buffer shouldn't be empty
            let last_logical_line = self.buffer.last().unwrap();

            // index of pushed glyph
            let i = last_logical_line.len() - 1;

            last_visual_line.end = i; // end is exclusive
            self.visual_lines.push(VisualLine {
                line_idx: self.buffer.len() - 1,
                start: i,
                end: i + 1,
            });
            self.cursor_x = self.config.padding_left + advance;
        } else {
            last_visual_line.end += 1;
        }
    }

    pub fn render(&mut self, renderer: &mut Renderer) {
        renderer.fill_solid(&self.bounding_box, self.config.color_bg);

        // visual lines within view
        let lines_to_render = {
            let end = self.visual_lines.len() - self.scroll_offset;
            let start = end.saturating_sub(self.max_visible_lines());
            &self.visual_lines[start..end]
        };

        // Y value of baseline of current line, relative to top of bounding box
        let mut y =
            self.bounding_box.size.height as i32 - self.config.padding_bottom - self.line_descent;
        for line in lines_to_render.iter().rev() {
            let mut x = self.config.padding_left;
            let visual_line = visual_line_slice(line, &self.buffer);
            for glyph in visual_line {
                let glyph_data = get_raster(glyph.printable_char(), self.config.font_size);
                let (metrics, bitmap) = glyph_data.as_ref();
                self.render_char(bitmap, *metrics, glyph.style.color_text, x, y, renderer);
                x += self.calc_advance(metrics);
            }
            y -= self.line_height + self.config.line_spacing;
        }
    }

    /// render bitmap at x and y. y is at baseline
    fn render_char(
        &self,
        bitmap: &Vec<u8>,
        metrics: Metrics,
        color: Color,
        x: i32,
        y: i32,
        renderer: &mut Renderer,
    ) {
        let tx = self.bounding_box.top_left.x as i32 + x + metrics.xmin;
        let ty = self.bounding_box.top_left.y as i32 + y - metrics.height as i32 - metrics.ymin;

        let pixels = bitmap.iter().enumerate().flat_map(|(i, intensity)| {
            let px = i % metrics.width;
            let py = i / metrics.width;
            if *intensity != 0 {
                Some(Pixel(
                    Point::new(tx + px as i32, ty + py as i32),
                    color.with_intensity(*intensity),
                ))
            } else {
                None
            }
        });
        renderer.draw_iter(pixels);
    }

    pub fn get_current_text_color(&self) -> Color {
        self.parser.current_style.color_text
    }
}

impl core::fmt::Write for TextBox {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for b in s.bytes() {
            self.push_byte(b);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loading_fonts() {
        use embedded_graphics::prelude::Size;
        let config = TextBoxConfig::default();
        let _line_metrics = FONT
            .horizontal_line_metrics(config.font_size as f32)
            .expect("Could not get font metrics");
        let _text_box = TextBox::new(Rectangle::new(Point::new(0, 0), Size::new(100, 100)));
    }
}
