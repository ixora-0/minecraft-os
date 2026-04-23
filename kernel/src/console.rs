extern crate alloc;

use alloc::{format, string::String};

use glam::{IVec2, USizeVec2};
use kernel_core::rendering::{Color, Frame, Rectangle, TextBox};

use crate::ps2::keyboard::{KeyCode, KeyboardEvent};

const PROMPT: &str = "> ";
/// Placeholder text shown in console when input is empty and console is inactive.
const PLACEHOLDER_TEXT: &str = "Press [Enter] to type command (try \"help\")";
const MAX_INPUT_LEN: usize = 96;
const FONT_SIZE: u32 = 14;

pub struct Console {
    bounds: Rectangle,
    text_box: TextBox,
    input: String,
    cursor_pos: usize,
    active: bool,
    visible: bool,
}

impl Console {
    pub fn recommended_height() -> usize {
        let mut tb = TextBox::new(Rectangle {
            top_left: IVec2::new(0, 0),
            size: USizeVec2::new(1, 1),
        });
        tb.set_font_size(FONT_SIZE);
        tb.min_dimensions(MAX_INPUT_LEN, 1).y as usize
    }

    pub fn new(bounds: Rectangle) -> Self {
        let text_box_bounds = Rectangle {
            top_left: bounds.top_left,
            size: bounds.size,
        };
        let mut text_box = TextBox::new(text_box_bounds);
        text_box.set_font_size(FONT_SIZE);
        let mut console = Self {
            bounds,
            text_box,
            input: String::new(),
            cursor_pos: 0,
            active: false,
            visible: true,
        };
        console.update_text();
        console
    }

    pub fn set_bounds(&mut self, bounds: Rectangle) {
        let text_box_bounds = Rectangle {
            top_left: bounds.top_left,
            size: bounds.size,
        };
        self.bounds = bounds;
        self.text_box = TextBox::new(text_box_bounds);
        self.text_box.set_font_size(FONT_SIZE);
        self.update_text();
    }

    pub fn toggle(&mut self) {
        self.active = !self.active;
        if self.active {
            self.update_text();
        }
    }

    pub fn set_active(&mut self, active: bool) {
        if self.active == active {
            return;
        }
        self.active = active;
        if active {
            self.cursor_pos = self.input.len();
        }
        self.update_text();
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    pub fn toggle_visible(&mut self) {
        self.visible = !self.visible;
    }

    pub fn process_event(&mut self, event: KeyboardEvent) -> Option<String> {
        if !self.active {
            return None;
        }

        match event.code {
            KeyCode::Return => {
                let command = core::mem::take(&mut self.input);
                self.cursor_pos = 0;
                self.update_text();
                self.set_active(false);
                Some(command)
            }
            KeyCode::Backspace => {
                if self.cursor_pos > 0 {
                    self.input.remove(self.cursor_pos - 1);
                    self.cursor_pos -= 1;
                    self.update_text();
                }
                None
            }
            KeyCode::Escape => {
                self.set_active(false);
                None
            }
            KeyCode::ArrowLeft => {
                self.cursor_pos = self.cursor_pos.saturating_sub(1);
                self.update_text();
                None
            }
            KeyCode::ArrowRight => {
                if self.cursor_pos < self.input.len() {
                    self.cursor_pos += 1;
                    self.update_text();
                }
                None
            }
            _ => {
                let ch = event.character?;
                if !ch.is_control() && self.input.len() < MAX_INPUT_LEN {
                    self.input.insert(self.cursor_pos, ch);
                    self.cursor_pos += 1;
                    self.update_text();
                }
                None
            }
        }
    }

    pub fn render(&mut self, frame: &mut Frame) {
        if !self.visible && !self.active {
            return;
        }

        let mut renderer = frame.renderer2d();
        renderer.fill_solid(&self.bounds, Color::BLACK);
        self.text_box.render(&mut renderer);
    }

    fn update_text(&mut self) {
        let content = if self.input.is_empty() && !self.active {
            let color_fg = Color::LIGHT_GRAY.fg();
            let reset_fg = Color::WHITE.fg();
            format!("{}{}{}{}", PROMPT, color_fg, PLACEHOLDER_TEXT, reset_fg)
        } else {
            format!("{}{}", PROMPT, self.input)
        };
        self.text_box.set_text(&content);
        if self.active {
            self.text_box
                .set_cursor_index(Some(PROMPT.len() + self.cursor_pos));
        } else {
            self.text_box.set_cursor_index(None);
        }
    }
}
