use bootloader_api::info::PixelFormat;
use core::fmt;
use embedded_graphics::pixelcolor::{PixelColor, raw::RawU24};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}
impl Color {
    pub const BLACK: Color = Color {
        red: 0,
        green: 0,
        blue: 0,
    };
    pub const WHITE: Color = Color {
        red: 255,
        green: 255,
        blue: 255,
    };
    pub const RED: Color = Color {
        red: 255,
        green: 0,
        blue: 0,
    };
    pub const YELLOW: Color = Color {
        red: 255,
        green: 255,
        blue: 0,
    };
    pub const GREEN: Color = Color {
        red: 0,
        green: 255,
        blue: 0,
    };
    pub const CYAN: Color = Color {
        red: 0,
        green: 255,
        blue: 255,
    };
    pub const LIGHT_GRAY: Color = Color {
        red: 180,
        green: 180,
        blue: 180,
    };
    pub const DARK_GRAY: Color = Color {
        red: 120,
        green: 120,
        blue: 120,
    };
    /// intensity between 0 and 255, 0 is black
    pub fn with_intensity(&self, intensity: u8) -> Color {
        Color {
            red: ((self.red as u16 * intensity as u16) / 255) as u8,
            green: ((self.green as u16 * intensity as u16) / 255) as u8,
            blue: ((self.blue as u16 * intensity as u16) / 255) as u8,
        }
    }
    /// intensity between 0.0 and 1.0, 0.0 is black
    pub fn with_intensity_f(&self, intensity: f32) -> Color {
        Color {
            red: (self.red as f32 * intensity) as u8,
            green: (self.green as f32 * intensity) as u8,
            blue: (self.blue as f32 * intensity) as u8,
        }
    }
    pub fn to_gray(&self) -> u8 {
        self.red / 3 + self.green / 3 + self.blue / 3
    }
    pub fn parse_hex(hex: &str) -> Option<Color> {
        let hex = hex.strip_prefix('#')?;
        let num = u32::from_str_radix(hex, 16).ok()?;
        // RGB format in bits: 00000000 RRRRRRRR GGGGGGGG BBBBBBBB
        Some(Color {
            red: (num >> 16) as u8,
            green: (num >> 8) as u8,
            blue: num as u8,
        })
    }
    pub fn write_to(&self, chunk: &mut [u8], format: PixelFormat) {
        match format {
            PixelFormat::Rgb => {
                chunk[0] = self.red;
                chunk[1] = self.green;
                chunk[2] = self.blue;
            }
            PixelFormat::Bgr => {
                chunk[0] = self.blue;
                chunk[1] = self.green;
                chunk[2] = self.red;
            }
            PixelFormat::U8 => {
                chunk[0] = self.to_gray();
            }
            other => panic!("unknown pixel format {other:?}"),
        }
    }

    pub fn build_48_byte_pattern(&self, format: PixelFormat, bpp: usize) -> [u8; 48] {
        let mut pattern = [0u8; 48];
        for chunk in pattern.chunks_exact_mut(bpp) {
            self.write_to(chunk, format);
        }
        pattern
    }

    pub fn fg(&self) -> Fg<'_> {
        Fg(self)
    }
}

pub struct Fg<'a>(&'a Color);
impl fmt::Display for Fg<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let c = self.0;
        // custom control protocol
        // format:
        // ESC ~ <COMMAND> BEL
        //
        // set foreground format
        // ESC ~ FG#RRGGBB BEL
        write!(f, "\x1B~FG#{:02X}{:02X}{:02X}\x07", c.red, c.green, c.blue)
    }
}

impl PixelColor for Color {
    type Raw = RawU24;
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn color_with_intensity() {
        // tests for overflow
        let color = Color::WHITE;
        let color_with_intensity = color.with_intensity(255);
        assert_eq!(color_with_intensity.red, 255);
        assert_eq!(color_with_intensity.green, 255);
        assert_eq!(color_with_intensity.blue, 255);

        let color_with_intensity_f = color.with_intensity_f(1.0);
        assert_eq!(color_with_intensity_f.red, 255);
        assert_eq!(color_with_intensity_f.green, 255);
        assert_eq!(color_with_intensity_f.blue, 255);

        let color_with_intensity_f_half = color.with_intensity_f(0.5);
        assert_eq!(color_with_intensity_f_half.red, 127);
        assert_eq!(color_with_intensity_f_half.green, 127);
        assert_eq!(color_with_intensity_f_half.blue, 127);
    }

    #[test]
    fn build_48_byte_pattern_rgb() {
        let color = Color {
            red: 0x12,
            green: 0x34,
            blue: 0x56,
        };
        let pattern = color.build_48_byte_pattern(PixelFormat::Rgb, 4);
        for chunk in pattern.chunks(4) {
            assert_eq!(chunk, &[0x12, 0x34, 0x56, 0x00]);
        }
    }
}
