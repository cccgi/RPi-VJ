//! 96x96 icon canvas, drawing primitives, font rendering, and baseline JPEG encoder.
//!
//! Stream Deck XL requires 96x96 baseline JPEG images for key icons.

pub const ICON_WIDTH: usize = 96;
pub const ICON_HEIGHT: usize = 96;
pub const ICON_PIXELS: usize = ICON_WIDTH * ICON_HEIGHT;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const BLACK: Rgb = Rgb { r: 0, g: 0, b: 0 };
    pub const WHITE: Rgb = Rgb { r: 255, g: 255, b: 255 };
    pub const RED: Rgb = Rgb { r: 220, g: 38, b: 38 };
    pub const GREEN: Rgb = Rgb { r: 34, g: 197, b: 94 };
    pub const BLUE: Rgb = Rgb { r: 59, g: 130, b: 246 };
    pub const YELLOW: Rgb = Rgb { r: 234, g: 179, b: 8 };
    pub const AMBER: Rgb = Rgb { r: 245, g: 158, b: 11 };
    pub const CYAN: Rgb = Rgb { r: 6, g: 182, b: 212 };
    pub const MAGENTA: Rgb = Rgb { r: 217, g: 70, b: 239 };
    pub const PURPLE: Rgb = Rgb { r: 168, g: 85, b: 247 };
    pub const DARK_GRAY: Rgb = Rgb { r: 30, g: 30, b: 35 };
    pub const MID_GRAY: Rgb = Rgb { r: 75, g: 85, b: 99 };
    pub const LIGHT_GRAY: Rgb = Rgb { r: 156, g: 163, b: 175 };
    pub const ORANGE: Rgb = Rgb { r: 249, g: 115, b: 22 };
    pub const DARK_BLUE: Rgb = Rgb { r: 15, g: 23, b: 42 };
    pub const DARK_RED: Rgb = Rgb { r: 69, g: 10, b: 10 };

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn blend(self, other: Rgb, factor: f32) -> Rgb {
        let f = factor.clamp(0.0, 1.0);
        let inv = 1.0 - f;
        Rgb {
            r: (self.r as f32 * inv + other.r as f32 * f).round() as u8,
            g: (self.g as f32 * inv + other.g as f32 * f).round() as u8,
            b: (self.b as f32 * inv + other.b as f32 * f).round() as u8,
        }
    }
}

/// 96x96 RGB Canvas for Stream Deck XL icon rendering.
#[derive(Clone, Debug)]
pub struct Canvas {
    pub pixels: [Rgb; ICON_PIXELS],
}

impl Default for Canvas {
    fn default() -> Self {
        Self::new(Rgb::BLACK)
    }
}

impl Canvas {
    pub fn new(bg: Rgb) -> Self {
        Self {
            pixels: [bg; ICON_PIXELS],
        }
    }

    #[inline]
    pub fn set_pixel(&mut self, x: usize, y: usize, color: Rgb) {
        if x < ICON_WIDTH && y < ICON_HEIGHT {
            self.pixels[y * ICON_WIDTH + x] = color;
        }
    }

    #[inline]
    pub fn get_pixel(&self, x: usize, y: usize) -> Rgb {
        if x < ICON_WIDTH && y < ICON_HEIGHT {
            self.pixels[y * ICON_WIDTH + x]
        } else {
            Rgb::BLACK
        }
    }

    pub fn clear(&mut self, color: Rgb) {
        self.pixels.fill(color);
    }

    /// Replace the whole canvas with a pre-decoded 96x96 image (a cached
    /// clip thumbnail). Callers draw borders/labels/status text on top of
    /// this afterward, same as any other background fill.
    pub fn blit(&mut self, pixels: &[Rgb; ICON_PIXELS]) {
        self.pixels.copy_from_slice(pixels);
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: Rgb) {
        let x_end = (x + w).min(ICON_WIDTH);
        let y_end = (y + h).min(ICON_HEIGHT);
        for py in y..y_end {
            for px in x..x_end {
                self.set_pixel(px, py, color);
            }
        }
    }

    pub fn draw_rect_border(&mut self, x: usize, y: usize, w: usize, h: usize, border_width: usize, color: Rgb) {
        let x_end = (x + w).min(ICON_WIDTH);
        let y_end = (y + h).min(ICON_HEIGHT);

        for py in y..y_end {
            for px in x..x_end {
                if px < x + border_width
                    || px >= x_end - border_width
                    || py < y + border_width
                    || py >= y_end - border_width
                {
                    self.set_pixel(px, py, color);
                }
            }
        }
    }

    pub fn draw_progress_bar(&mut self, x: usize, y: usize, w: usize, h: usize, progress: f32, fg: Rgb, bg: Rgb) {
        let p = progress.clamp(0.0, 1.0);
        self.fill_rect(x, y, w, h, bg);
        let fill_w = ((w as f32) * p).round() as usize;
        if fill_w > 0 {
            self.fill_rect(x, y, fill_w, h, fg);
        }
        self.draw_rect_border(x, y, w, h, 1, Rgb::WHITE);
    }

    /// Draw circular / ring hold countdown progress
    pub fn draw_hold_ring(&mut self, cx: usize, cy: usize, radius: usize, progress: f32, color: Rgb) {
        let p = progress.clamp(0.0, 1.0);
        let r_f = radius as f32;
        let r_inner = r_f - 4.0;
        let angle_end = p * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;

        for py in 0..ICON_HEIGHT {
            for px in 0..ICON_WIDTH {
                let dx = px as f32 - cx as f32;
                let dy = py as f32 - cy as f32;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist <= r_f && dist >= r_inner {
                    let mut angle = dy.atan2(dx);
                    if angle < -std::f32::consts::FRAC_PI_2 {
                        angle += std::f32::consts::TAU;
                    }
                    if angle <= angle_end {
                        self.set_pixel(px, py, color);
                    }
                }
            }
        }
    }

    /// Render a single 5x7 character bitmap at (x, y) scaled by `scale`.
    pub fn draw_char(&mut self, ch: char, x: usize, y: usize, scale: usize, color: Rgb) {
        let glyph = get_glyph(ch);
        let s = scale.max(1);
        for row in 0..7 {
            let row_bits = glyph[row];
            for col in 0..5 {
                if (row_bits & (1 << (4 - col))) != 0 {
                    for dy in 0..s {
                        for dx in 0..s {
                            self.set_pixel(x + col * s + dx, y + row * s + dy, color);
                        }
                    }
                }
            }
        }
    }

    /// Render a string of text with optional scale.
    pub fn draw_text(&mut self, text: &str, x: usize, y: usize, scale: usize, color: Rgb) {
        let s = scale.max(1);
        let char_width = 6 * s;
        let mut curr_x = x;
        for ch in text.chars() {
            if ch == '\n' {
                break;
            }
            self.draw_char(ch, curr_x, y, scale, color);
            curr_x += char_width;
        }
    }

    /// Draw centered text horizontally within a bounding box [x, x + w].
    pub fn draw_text_centered(&mut self, text: &str, x: usize, y: usize, w: usize, scale: usize, color: Rgb) {
        let s = scale.max(1);
        let text_w = text.len() * 6 * s;
        let start_x = if text_w < w {
            x + (w - text_w) / 2
        } else {
            x
        };
        self.draw_text(text, start_x, y, scale, color);
    }

    /// Render a single 5x7 character bitmap with bold 2px horizontal stroke.
    pub fn draw_char_bold(&mut self, ch: char, x: usize, y: usize, scale: usize, color: Rgb) {
        let glyph = get_glyph(ch);
        let s = scale.max(1);
        for row in 0..7 {
            let row_bits = glyph[row];
            for col in 0..5 {
                if (row_bits & (1 << (4 - col))) != 0 {
                    for dy in 0..s {
                        for dx in 0..=s {
                            self.set_pixel(x + col * s + dx, y + row * s + dy, color);
                        }
                    }
                }
            }
        }
    }

    /// Render a string of text with bold stroke.
    pub fn draw_text_bold(&mut self, text: &str, x: usize, y: usize, scale: usize, color: Rgb) {
        let s = scale.max(1);
        let char_width = 6 * s + 1;
        let mut curr_x = x;
        for ch in text.chars() {
            if ch == '\n' {
                break;
            }
            self.draw_char_bold(ch, curr_x, y, scale, color);
            curr_x += char_width;
        }
    }

    /// Draw centered bold text horizontally within a bounding box [x, x + w].
    pub fn draw_text_centered_bold(&mut self, text: &str, x: usize, y: usize, w: usize, scale: usize, color: Rgb) {
        let s = scale.max(1);
        let char_width = 6 * s + 1;
        let text_w = text.len() * char_width;
        let start_x = if text_w < w {
            x + (w - text_w) / 2
        } else {
            x
        };
        self.draw_text_bold(text, start_x, y, scale, color);
    }

    /// Encode canvas into standard baseline JPEG format with 180-degree hardware rotation for Stream Deck XL.
    pub fn encode_jpeg(&self) -> Vec<u8> {
        let mut rotated = [Rgb::BLACK; ICON_PIXELS];
        for y in 0..ICON_HEIGHT {
            for x in 0..ICON_WIDTH {
                let src_idx = y * ICON_WIDTH + x;
                let dst_idx = (ICON_HEIGHT - 1 - y) * ICON_WIDTH + (ICON_WIDTH - 1 - x);
                rotated[dst_idx] = self.pixels[src_idx];
            }
        }
        encode_baseline_jpeg(&rotated, ICON_WIDTH, ICON_HEIGHT)
    }
}

/// 5x7 bitmap font for ASCII 32..126
fn get_glyph(ch: char) -> [u8; 7] {
    let c = ch.to_ascii_uppercase();
    match c {
        ' ' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        '!' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x00, 0x04],
        '"' => [0x0A, 0x0A, 0x0A, 0x00, 0x00, 0x00, 0x00],
        '#' => [0x0A, 0x0A, 0x1F, 0x0A, 0x1F, 0x0A, 0x0A],
        '$' => [0x04, 0x1E, 0x05, 0x0E, 0x14, 0x0F, 0x04],
        '%' => [0x19, 0x19, 0x02, 0x04, 0x08, 0x13, 0x13],
        '&' => [0x0C, 0x12, 0x14, 0x08, 0x15, 0x12, 0x0D],
        '\'' => [0x0C, 0x04, 0x08, 0x00, 0x00, 0x00, 0x00],
        '(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        ')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        '*' => [0x00, 0x04, 0x15, 0x0E, 0x15, 0x04, 0x00],
        '+' => [0x00, 0x04, 0x04, 0x1F, 0x04, 0x04, 0x00],
        ',' => [0x00, 0x00, 0x00, 0x00, 0x0C, 0x04, 0x08],
        '-' => [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
        '/' => [0x01, 0x02, 0x04, 0x08, 0x10, 0x00, 0x00],
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        ':' => [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x0C, 0x00],
        ';' => [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x04, 0x08],
        '<' => [0x02, 0x04, 0x08, 0x10, 0x08, 0x04, 0x02],
        '=' => [0x00, 0x1F, 0x00, 0x1F, 0x00, 0x00, 0x00],
        '>' => [0x08, 0x04, 0x02, 0x01, 0x02, 0x04, 0x08],
        '?' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x00, 0x04],
        '@' => [0x0E, 0x11, 0x01, 0x0D, 0x15, 0x15, 0x0E],
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1C, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1C],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0E, 0x11, 0x10, 0x0E, 0x01, 0x11, 0x0E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1B, 0x11],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        '[' => [0x0E, 0x08, 0x08, 0x08, 0x08, 0x08, 0x0E],
        '\\' => [0x10, 0x08, 0x04, 0x02, 0x01, 0x00, 0x00],
        ']' => [0x0E, 0x02, 0x02, 0x02, 0x02, 0x02, 0x0E],
        '^' => [0x04, 0x0A, 0x11, 0x00, 0x00, 0x00, 0x00],
        '_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1F],
        '`' => [0x08, 0x04, 0x02, 0x00, 0x00, 0x00, 0x00],
        '|' => [0x04, 0x04, 0x04, 0x00, 0x04, 0x04, 0x04],
        '~' => [0x00, 0x0A, 0x15, 0x00, 0x00, 0x00, 0x00],
        _ => [0x1F, 0x11, 0x15, 0x15, 0x11, 0x1F, 0x00],
    }
}

// ---------------------------------------------------------------------------
// Compact Pure Rust Baseline JPEG Encoder for 96x96 RGB
// ---------------------------------------------------------------------------

const STD_LUMA_Q: [u8; 64] = [
    2, 1, 1, 2, 2, 4, 5, 6,
    1, 1, 1, 2, 3, 6, 6, 6,
    1, 1, 2, 2, 4, 6, 7, 6,
    1, 2, 2, 3, 5, 9, 8, 6,
    2, 2, 4, 6, 7, 11, 10, 8,
    2, 4, 6, 6, 8, 10, 11, 9,
    5, 6, 8, 9, 10, 12, 12, 10,
    7, 9, 10, 10, 11, 10, 10, 10,
];

const STD_CHROMA_Q: [u8; 64] = [
    2, 2, 2, 5, 10, 10, 10, 10,
    2, 2, 3, 7, 10, 10, 10, 10,
    2, 3, 6, 10, 10, 10, 10, 10,
    5, 7, 10, 10, 10, 10, 10, 10,
    10, 10, 10, 10, 10, 10, 10, 10,
    10, 10, 10, 10, 10, 10, 10, 10,
    10, 10, 10, 10, 10, 10, 10, 10,
    10, 10, 10, 10, 10, 10, 10, 10,
];

const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10,
    17, 24, 32, 25, 18, 11, 4, 5,
    12, 19, 26, 33, 40, 48, 41, 34,
    27, 20, 13, 6, 7, 14, 21, 28,
    35, 42, 49, 56, 57, 50, 43, 36,
    29, 22, 15, 23, 30, 37, 44, 51,
    58, 59, 52, 45, 38, 31, 39, 46,
    53, 60, 61, 54, 47, 55, 62, 63,
];

const DC_LUMA_BITS: [u8; 16] = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];
const DC_LUMA_VALS: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

const AC_LUMA_BITS: [u8; 16] = [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7d];
const AC_LUMA_VALS: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07,
    0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08, 0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0,
    0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28,
    0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
    0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
    0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
    0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7,
    0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5,
    0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2,
    0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];

const DC_CHROMA_BITS: [u8; 16] = [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0];
const DC_CHROMA_VALS: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

const AC_CHROMA_BITS: [u8; 16] = [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77];
const AC_CHROMA_VALS: [u8; 162] = [
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71,
    0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33, 0x52, 0xf0,
    0x15, 0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34, 0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a, 0x26,
    0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
    0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
    0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
    0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5,
    0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3,
    0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda,
    0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];

struct BitWriter {
    buffer: Vec<u8>,
    accumulator: u32,
    bits_in_acc: u8,
}

impl BitWriter {
    fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(4096),
            accumulator: 0,
            bits_in_acc: 0,
        }
    }

    fn write_bits(&mut self, code: u16, len: u8) {
        if len == 0 {
            return;
        }
        self.accumulator = (self.accumulator << len) | (code as u32 & ((1 << len) - 1));
        self.bits_in_acc += len;
        while self.bits_in_acc >= 8 {
            self.bits_in_acc -= 8;
            let byte = ((self.accumulator >> self.bits_in_acc) & 0xFF) as u8;
            self.buffer.push(byte);
            if byte == 0xFF {
                self.buffer.push(0x00);
            }
        }
    }

    fn flush(&mut self) {
        if self.bits_in_acc > 0 {
            let pad = 8 - self.bits_in_acc;
            self.write_bits((1 << pad) - 1, pad);
        }
    }
}

struct HuffmanTable {
    codes: [u16; 256],
    lengths: [u8; 256],
}

impl HuffmanTable {
    fn build(bits: &[u8; 16], values: &[u8]) -> Self {
        let mut codes = [0u16; 256];
        let mut lengths = [0u8; 256];
        let mut code = 0u16;
        let mut val_idx = 0;

        for (len_minus_1, &count) in bits.iter().enumerate() {
            let len = (len_minus_1 + 1) as u8;
            for _ in 0..count {
                if val_idx < values.len() {
                    let val = values[val_idx] as usize;
                    codes[val] = code;
                    lengths[val] = len;
                    val_idx += 1;
                    code += 1;
                }
            }
            code <<= 1;
        }

        Self { codes, lengths }
    }
}

fn dct_8x8(block: &[f32; 64], out: &mut [f32; 64]) {
    use std::f32::consts::PI;
    for u in 0..8 {
        for v in 0..8 {
            let mut sum = 0.0f32;
            for x in 0..8 {
                for y in 0..8 {
                    let sample = block[y * 8 + x];
                    let cos_x = ((2.0 * x as f32 + 1.0) * u as f32 * PI / 16.0).cos();
                    let cos_y = ((2.0 * y as f32 + 1.0) * v as f32 * PI / 16.0).cos();
                    sum += sample * cos_x * cos_y;
                }
            }
            let cu = if u == 0 { 1.0 / std::f32::consts::SQRT_2 } else { 1.0 };
            let cv = if v == 0 { 1.0 / std::f32::consts::SQRT_2 } else { 1.0 };
            // Natural-order storage is row-major by vertical frequency v, then
            // horizontal frequency u (index = 8*v + u), matching the convention
            // ZIGZAG and the quantization tables below assume.
            out[v * 8 + u] = 0.25 * cu * cv * sum;
        }
    }
}

fn quantize_and_zigzag(dct: &[f32; 64], q_table: &[u8; 64], out: &mut [i16; 64]) {
    // ZIGZAG[scan_pos] = natural_pos. Walk scan order and pull each
    // coefficient from its natural-order position so `out` ends up holding
    // coefficients in zigzag scan order, as `encode_block` expects.
    for scan_pos in 0..64 {
        let natural = ZIGZAG[scan_pos];
        let q = q_table[natural] as f32;
        out[scan_pos] = (dct[natural] / q).round() as i16;
    }
}

fn encode_block(
    quantized: &[i16; 64],
    prev_dc: &mut i16,
    dc_table: &HuffmanTable,
    ac_table: &HuffmanTable,
    writer: &mut BitWriter,
) {
    let diff = quantized[0] - *prev_dc;
    *prev_dc = quantized[0];

    let (category, bits) = int_to_category_and_bits(diff);
    writer.write_bits(dc_table.codes[category as usize], dc_table.lengths[category as usize]);
    if category > 0 {
        writer.write_bits(bits, category);
    }

    let mut r = 0u8;
    for k in 1..64 {
        let val = quantized[k];
        if val == 0 {
            r += 1;
        } else {
            while r >= 16 {
                writer.write_bits(ac_table.codes[0xF0], ac_table.lengths[0xF0]);
                r -= 16;
            }
            let (category, bits) = int_to_category_and_bits(val);
            let symbol = (r << 4) | category;
            writer.write_bits(ac_table.codes[symbol as usize], ac_table.lengths[symbol as usize]);
            writer.write_bits(bits, category);
            r = 0;
        }
    }

    if r > 0 {
        writer.write_bits(ac_table.codes[0x00], ac_table.lengths[0x00]);
    }
}

fn int_to_category_and_bits(v: i16) -> (u8, u16) {
    if v == 0 {
        return (0, 0);
    }
    let abs_v = v.unsigned_abs();
    let cat = 16 - abs_v.leading_zeros() as u8;
    let bits = if v < 0 {
        // JPEG "extra bits" for a negative coefficient of category `cat` is
        // its magnitude bitwise-complemented within `cat` bits, i.e.
        // mask - abs_v (since abs_v <= mask, complement == mask - abs_v).
        // There is no "-1" here: that would compute mask - abs_v + 1,
        // silently corrupting every negative coefficient by one.
        abs_v ^ ((1 << cat) - 1)
    } else {
        abs_v
    };
    (cat, bits)
}

/// Encode raw RGB pixels to a valid baseline JPEG byte vector.
pub fn encode_baseline_jpeg(pixels: &[Rgb], width: usize, height: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(8192);

    // 1. SOI (Start of Image)
    out.extend_from_slice(&[0xFF, 0xD8]);

    // 2. APP0 (JFIF Header)
    out.extend_from_slice(&[
        0xFF, 0xE0, 0x00, 0x10,
        b'J', b'F', b'I', b'F', 0x00,
        0x01, 0x01,
        0x00,
        0x00, 0x48, 0x00, 0x48,
        0x00, 0x00,
    ]);

    // 3. DQT (Quantization tables)
    out.extend_from_slice(&[0xFF, 0xDB, 0x00, 0x84]);
    out.push(0x00);
    for &z in &ZIGZAG {
        out.push(STD_LUMA_Q[z]);
    }
    out.push(0x01);
    for &z in &ZIGZAG {
        out.push(STD_CHROMA_Q[z]);
    }

    // 4. SOF0 (Start of Frame: Baseline DCT, 8-bit precision, 3 components YCbCr)
    let w_u16 = width as u16;
    let h_u16 = height as u16;
    out.extend_from_slice(&[
        0xFF, 0xC0, 0x00, 0x11,
        0x08,
        (h_u16 >> 8) as u8, (h_u16 & 0xFF) as u8,
        (w_u16 >> 8) as u8, (w_u16 & 0xFF) as u8,
        0x03,
        0x01, 0x11, 0x00,
        0x02, 0x11, 0x01,
        0x03, 0x11, 0x01,
    ]);

    // 5. DHT (Huffman Tables)
    out.extend_from_slice(&[0xFF, 0xC4, 0x00, 0x1F, 0x00]);
    out.extend_from_slice(&DC_LUMA_BITS);
    out.extend_from_slice(&DC_LUMA_VALS);

    out.extend_from_slice(&[0xFF, 0xC4, 0x00, 0xB5, 0x10]);
    out.extend_from_slice(&AC_LUMA_BITS);
    out.extend_from_slice(&AC_LUMA_VALS);

    out.extend_from_slice(&[0xFF, 0xC4, 0x00, 0x1F, 0x01]);
    out.extend_from_slice(&DC_CHROMA_BITS);
    out.extend_from_slice(&DC_CHROMA_VALS);

    out.extend_from_slice(&[0xFF, 0xC4, 0x00, 0xB5, 0x11]);
    out.extend_from_slice(&AC_CHROMA_BITS);
    out.extend_from_slice(&AC_CHROMA_VALS);

    // 6. SOS (Start of Scan)
    out.extend_from_slice(&[
        0xFF, 0xDA, 0x00, 0x0C,
        0x03,
        0x01, 0x00,
        0x02, 0x11,
        0x03, 0x11,
        0x00, 0x3F, 0x00,
    ]);

    // 7. Compress blocks
    let dc_luma_table = HuffmanTable::build(&DC_LUMA_BITS, &DC_LUMA_VALS);
    let ac_luma_table = HuffmanTable::build(&AC_LUMA_BITS, &AC_LUMA_VALS);
    let dc_chroma_table = HuffmanTable::build(&DC_CHROMA_BITS, &DC_CHROMA_VALS);
    let ac_chroma_table = HuffmanTable::build(&AC_CHROMA_BITS, &AC_CHROMA_VALS);

    let mut writer = BitWriter::new();
    let mut prev_dc_y = 0i16;
    let mut prev_dc_cb = 0i16;
    let mut prev_dc_cr = 0i16;

    let mcu_x = width.div_ceil(8);
    let mcu_y = height.div_ceil(8);

    let mut block_y = [0.0f32; 64];
    let mut block_cb = [0.0f32; 64];
    let mut block_cr = [0.0f32; 64];

    let mut dct_y = [0.0f32; 64];
    let mut dct_cb = [0.0f32; 64];
    let mut dct_cr = [0.0f32; 64];

    let mut q_y = [0i16; 64];
    let mut q_cb = [0i16; 64];
    let mut q_cr = [0i16; 64];

    for my in 0..mcu_y {
        for mx in 0..mcu_x {
            for by in 0..8 {
                let py = (my * 8 + by).min(height - 1);
                for bx in 0..8 {
                    let px = (mx * 8 + bx).min(width - 1);
                    let rgb = pixels[py * width + px];

                    let r = rgb.r as f32;
                    let g = rgb.g as f32;
                    let b = rgb.b as f32;

                    let y = 0.299 * r + 0.587 * g + 0.114 * b - 128.0;
                    let cb = -0.168736 * r - 0.331264 * g + 0.5 * b;
                    let cr = 0.5 * r - 0.418688 * g - 0.081312 * b;

                    let idx = by * 8 + bx;
                    block_y[idx] = y;
                    block_cb[idx] = cb;
                    block_cr[idx] = cr;
                }
            }

            dct_8x8(&block_y, &mut dct_y);
            dct_8x8(&block_cb, &mut dct_cb);
            dct_8x8(&block_cr, &mut dct_cr);

            quantize_and_zigzag(&dct_y, &STD_LUMA_Q, &mut q_y);
            quantize_and_zigzag(&dct_cb, &STD_CHROMA_Q, &mut q_cb);
            quantize_and_zigzag(&dct_cr, &STD_CHROMA_Q, &mut q_cr);

            encode_block(&q_y, &mut prev_dc_y, &dc_luma_table, &ac_luma_table, &mut writer);
            encode_block(&q_cb, &mut prev_dc_cb, &dc_chroma_table, &ac_chroma_table, &mut writer);
            encode_block(&q_cr, &mut prev_dc_cr, &dc_chroma_table, &ac_chroma_table, &mut writer);
        }
    }

    writer.flush();
    out.extend_from_slice(&writer.buffer);

    // 8. EOI (End of Image)
    out.extend_from_slice(&[0xFF, 0xD9]);

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canvas_basic_drawing() {
        let mut canvas = Canvas::new(Rgb::BLACK);
        canvas.fill_rect(10, 10, 20, 20, Rgb::RED);
        assert_eq!(canvas.get_pixel(10, 10), Rgb::RED);
        assert_eq!(canvas.get_pixel(29, 29), Rgb::RED);
        assert_eq!(canvas.get_pixel(30, 30), Rgb::BLACK);

        canvas.draw_text("TEST", 0, 0, 1, Rgb::WHITE);
        assert_ne!(canvas.get_pixel(0, 0), Rgb::BLACK);
    }

    #[test]
    fn test_jpeg_encoder_output_structure() {
        let canvas = Canvas::new(Rgb::BLUE);
        let jpeg = canvas.encode_jpeg();

        assert!(jpeg.len() > 100);
        assert_eq!(&jpeg[0..2], &[0xFF, 0xD8]);
        assert_eq!(&jpeg[2..4], &[0xFF, 0xE0]);
        assert_eq!(&jpeg[jpeg.len() - 2..], &[0xFF, 0xD9]);
    }

    #[test]
    fn test_draw_progress_and_hold_ring() {
        let mut canvas = Canvas::new(Rgb::BLACK);
        canvas.draw_progress_bar(5, 5, 86, 10, 0.5, Rgb::GREEN, Rgb::DARK_GRAY);
        canvas.draw_hold_ring(48, 48, 30, 0.75, Rgb::YELLOW);
        let jpeg = canvas.encode_jpeg();
        assert!(!jpeg.is_empty());
    }

    /// Round-trips a non-trivial pattern (text + shapes, not a flat color)
    /// through `encode_baseline_jpeg` and a real JPEG decoder to catch
    /// encoder bugs (zigzag/natural-order mixups, transposed DCT output)
    /// that a structural-only test can't see: those scramble AC
    /// coefficients rather than corrupting the container, so the file
    /// stays "valid" JPEG while the decoded image is garbage.
    #[test]
    fn test_jpeg_encoder_roundtrip_matches_source() {
        let mut canvas = Canvas::new(Rgb::DARK_BLUE);
        canvas.fill_rect(10, 10, 30, 20, Rgb::RED);
        canvas.fill_rect(50, 40, 25, 35, Rgb::GREEN);
        canvas.draw_text_bold("HI", 20, 70, 2, Rgb::WHITE);
        canvas.draw_hold_ring(48, 48, 20, 0.6, Rgb::AMBER);

        // Encode without the 180-degree rotation so decoded pixels line up
        // 1:1 with `canvas.pixels` for comparison.
        let jpeg = encode_baseline_jpeg(&canvas.pixels, ICON_WIDTH, ICON_HEIGHT);

        let decoded = image::load_from_memory_with_format(&jpeg, image::ImageFormat::Jpeg)
            .expect("encoder must produce a JPEG a standard decoder can read")
            .to_rgb8();
        assert_eq!(decoded.width() as usize, ICON_WIDTH);
        assert_eq!(decoded.height() as usize, ICON_HEIGHT);

        let mut sum_abs_err: i64 = 0;
        let mut max_abs_err: i64 = 0;
        for y in 0..ICON_HEIGHT {
            for x in 0..ICON_WIDTH {
                let expected = canvas.get_pixel(x, y);
                let got = decoded.get_pixel(x as u32, y as u32);
                for (e, g) in [
                    (expected.r, got[0]),
                    (expected.g, got[1]),
                    (expected.b, got[2]),
                ] {
                    let err = (e as i64 - g as i64).abs();
                    sum_abs_err += err;
                    max_abs_err = max_abs_err.max(err);
                }
            }
        }
        let mean_abs_err = sum_abs_err as f64 / (ICON_PIXELS * 3) as f64;

        // A correct (lossy) baseline JPEG encoder at these quant tables
        // stays close to the source; a scan/natural-order bug or a
        // transposed DCT output produces mean error in the tens and max
        // error saturating near 255.
        assert!(
            mean_abs_err < 15.0,
            "mean abs channel error too high: {mean_abs_err} (max {max_abs_err}) \
             — indicates AC coefficients are being written to the wrong position"
        );
        assert!(
            max_abs_err < 120,
            "max abs channel error too high: {max_abs_err} \
             — indicates AC coefficients are being written to the wrong position"
        );
    }
}
