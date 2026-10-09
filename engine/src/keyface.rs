//! Stream Deck XL key faces: one visual system for every key, free of HID and codecs.
//!
//! Every 96x96 key has the same three bands, so nothing moves when state changes:
//!
//! ```text
//!   head   0..20   reserved; carries the OFFLINE banner
//!   label 20..76   main label, centred, at the largest scale that fits
//!   foot  76..96   status word (LIVE, LOADING, NO FILE ...) or caption
//! ```
//!
//! A clip or shader with a thumbnail shows the picture over head and label and its name in the foot.
//! Colour keeps its existing meaning (green active, amber loading, red failed, dark red
//! unavailable) but never stands alone: state also changes the foot word and the frame.
use crate::{
    control::Command,
    surface::{KeyDescriptor, KeyKind, KeyState},
};
mod font;

pub const SIZE: usize = 96;
/// Thumbnails are supplied as tightly packed RGB of this size and sit above the foot.
pub const THUMB_WIDTH: usize = SIZE;
pub const THUMB_HEIGHT: usize = SIZE - FOOT;
const HEAD: usize = 20;
const FOOT: usize = 20;
/// Nothing is written closer to the key edge than this; the bezel hides a little.
const INSET: usize = 5;
const SAFE: usize = SIZE - 2 * INSET;
const FRAME: usize = 4;
/// Inset of the guard outline on reset and blackout.
const GUARD: usize = 4;
const ARROW: usize = 13;
/// A line may be narrowed to this percentage of its natural width before the label
/// wraps or steps down a scale.
const CONDENSE: usize = 85;

type Rgb = [u8; 3];
const WHITE: Rgb = [255, 255, 255];
const INK: Rgb = [10, 12, 16];
const BLANK: Rgb = [13, 14, 17];
/// The recessed panel that marks an unbound key as deliberately empty.
const BLANK_PANEL: Rgb = [24, 26, 32];
const CAPTION_BAND: Rgb = [16, 18, 22];
const OFFLINE: Rgb = [160, 60, 0];
const CROSS: Rgb = [235, 20, 20];

/// The type scale. A label takes the largest step that fits its zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Scale {
    Small,
    Medium,
    Large,
}
impl Scale {
    const DESCENDING: [Scale; 3] = [Scale::Large, Scale::Medium, Scale::Small];
    fn face(self) -> &'static font::Face {
        match self {
            Scale::Small => &font::SMALL,
            Scale::Medium => &font::MEDIUM,
            Scale::Large => &font::LARGE,
        }
    }
    /// Smaller type may wrap onto more lines; the zone height can reduce this further.
    fn lines(self, height: usize) -> usize {
        let face = self.face();
        let fit = 1 + height.saturating_sub(face.cap_height as usize) / face.line_height as usize;
        fit.min(match self {
            Scale::Small => 3,
            Scale::Medium => 2,
            Scale::Large => 1,
        })
    }
}

/// A label broken into centred lines at one scale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub scale: Scale,
    pub lines: Vec<String>,
    /// The label did not fit even at the smallest scale and ends in an ellipsis.
    pub truncated: bool,
}

/// Capitals only: one cap height per scale keeps every line optically centred.
/// Underscores separate words; common accents and typographic marks fold to ASCII.
pub fn normalize(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    for ch in label.chars().flat_map(char::to_uppercase) {
        let ch = match ch {
            'À'..='Å' => 'A',
            'Ç' => 'C',
            'È'..='Ë' => 'E',
            'Ì'..='Ï' => 'I',
            'Ñ' => 'N',
            'Ò'..='Ö' | 'Ø' => 'O',
            'Ù'..='Ü' => 'U',
            'Ý' => 'Y',
            '‘' | '’' => '\'',
            '“' | '”' => '"',
            '–' | '—' => '-',
            '×' => 'X',
            '_' => ' ',
            ch if ch.is_whitespace() || ch.is_control() => ' ',
            ch => ch,
        };
        if ch != ' ' || !(out.is_empty() || out.ends_with(' ')) {
            out.push(ch);
        }
    }
    out.truncate(out.trim_end().len());
    out
}
fn glyph(face: &font::Face, ch: char) -> Option<&'static font::Glyph> {
    let index = face.glyphs.binary_search_by_key(&ch, |glyph| glyph.ch).ok()?;
    Some(&face.glyphs[index])
}
/// Characters outside the embedded set are drawn as a hollow box of this width.
fn box_width(face: &font::Face) -> usize {
    face.cap_height as usize * 2 / 3
}
/// Pen advance in quarter pixels.
fn advance(face: &font::Face, ch: char) -> usize {
    glyph(face, ch).map_or((box_width(face) + 2) * 4, |glyph| glyph.advance as usize)
}
/// Width of one line in pixels, proportional and rounded up.
pub fn width(scale: Scale, text: &str) -> usize {
    let face = scale.face();
    text.chars().map(|ch| advance(face, ch)).sum::<usize>().div_ceil(4)
}
/// Width a line occupies once drawn. A line wider than `limit` is condensed to just
/// inside it, because resampling softens the outer edges by up to a pixel each.
fn drawn(scale: Scale, text: &str, limit: usize) -> usize {
    let natural = width(scale, text);
    if natural > limit {
        limit.saturating_sub(2)
    } else {
        natural
    }
}
/// Word wrap without breaking words: the fewest lines, then evenly filled ones.
/// A line up to 100/`CONDENSE` of the limit still counts as fitting.
fn wrap(scale: Scale, words: &[&str], limit: usize) -> Option<Vec<String>> {
    let span = |from: usize, to: usize| {
        let natural = width(scale, &words[from..to].join(" "));
        if natural * CONDENSE <= limit * 100 {
            natural.min(limit)
        } else {
            natural
        }
    };
    let mut lines = 1;
    let mut start = 0;
    for end in 1..=words.len() {
        if span(start, end) > limit {
            start = end - 1;
            lines += 1;
            if span(start, end) > limit {
                return None;
            }
        }
    }
    // A key holds three lines at most; the caller rejects anything longer.
    let count = words.len();
    let splits: Vec<Vec<usize>> = match lines {
        1 => vec![vec![count]],
        2 => (1..count).map(|cut| vec![cut, count]).collect(),
        3 => (1..count)
            .flat_map(|first| (first + 1..count).map(move |second| vec![first, second, count]))
            .collect(),
        _ => return Some(vec![String::new(); lines]),
    };
    let widest = |cuts: &[usize]| {
        let mut from = 0;
        cuts.iter()
            .map(|to| span(std::mem::replace(&mut from, *to), *to))
            .max()
            .unwrap_or(0)
    };
    // Even lines look best, but among near-equal splits fill the upper lines first so
    // sibling labels ("FX AMT 25", "FX AMT 100") break at the same place.
    let slack = scale.face().cap_height as usize / 2;
    let narrowest = splits.iter().map(|cuts| widest(cuts)).min()?;
    let cuts = splits
        .into_iter()
        .filter(|cuts| widest(cuts) <= (narrowest + slack).min(limit))
        .max()?;
    let mut from = 0;
    Some(
        cuts.into_iter()
            .map(|to| words[std::mem::replace(&mut from, to)..to].join(" "))
            .collect(),
    )
}
/// Last resort at the smallest scale: break inside words, then cut with an ellipsis.
fn force(scale: Scale, text: &str, limit: usize, max_lines: usize) -> (Vec<String>, bool) {
    let mut lines = Vec::new();
    let mut line = String::new();
    'words: for word in text.split(' ') {
        let joined = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if width(scale, &joined) <= limit {
            line = joined;
            continue;
        }
        if !line.is_empty() {
            if lines.len() + 1 >= max_lines {
                // The last line takes as much of the next word as fits before the ellipsis.
                lines.push(format!("{line} {word}"));
                lines.push(String::new());
                line.clear();
                break 'words;
            }
            lines.push(std::mem::take(&mut line));
        }
        for ch in word.chars() {
            line.push(ch);
            if width(scale, &line) > limit && line.chars().count() > 1 {
                line.pop();
                lines.push(std::mem::replace(&mut line, ch.to_string()));
                if lines.len() > max_lines {
                    break 'words;
                }
            }
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    let truncated = lines.len() > max_lines;
    if truncated {
        lines.truncate(max_lines);
        if let Some(last) = lines.last_mut() {
            loop {
                let cut = format!("{}…", last.trim_end());
                if width(scale, &cut) <= limit || last.is_empty() {
                    *last = cut;
                    break;
                }
                last.pop();
            }
        }
    }
    (lines, truncated)
}
/// Chooses the largest scale, no larger than `largest`, at which `label` fits a zone of
/// `width` x `height` pixels without breaking a word.
pub fn fit(label: &str, width: usize, height: usize, largest: Scale) -> Layout {
    let text = normalize(label);
    if text.is_empty() {
        return Layout {
            scale: largest,
            lines: Vec::new(),
            truncated: false,
        };
    }
    let words: Vec<&str> = text.split(' ').collect();
    for scale in Scale::DESCENDING.into_iter().filter(|scale| *scale <= largest) {
        if let Some(lines) = wrap(scale, &words, width) {
            if lines.len() <= scale.lines(height) {
                return Layout {
                    scale,
                    lines,
                    truncated: false,
                };
            }
        }
    }
    let (lines, truncated) = force(Scale::Small, &text, width, Scale::Small.lines(height));
    Layout {
        scale: Scale::Small,
        lines,
        truncated,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Zone {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}
const HEAD_ZONE: Zone = Zone {
    x: INSET,
    y: 0,
    width: SAFE,
    height: HEAD,
};
const FOOT_ZONE: Zone = Zone {
    x: INSET,
    y: SIZE - FOOT,
    width: SAFE,
    height: FOOT,
};
/// The main label zone. A navigation key gives up one side to its arrow; danger and
/// info keys keep their label clear of their outline.
fn label_zone(key: &KeyDescriptor) -> Zone {
    let (left, right) = match (key.kind, key.command) {
        (KeyKind::Navigate, Some(Command::Page { delta })) if delta < 0 => (ARROW + 3, 0),
        (KeyKind::Navigate, Some(Command::Page { .. })) => (0, ARROW + 3),
        (KeyKind::Danger | KeyKind::Info, _) => (GUARD + 2, GUARD + 2),
        _ => (0, 0),
    };
    Zone {
        x: INSET + left,
        y: HEAD,
        width: SAFE - left - right,
        height: SIZE - HEAD - FOOT,
    }
}
/// Top-left corner of each line: lines are centred in the zone on their cap height.
fn place(layout: &Layout, zone: Zone) -> Vec<(i32, i32)> {
    let face = layout.scale.face();
    let block = face.cap_height as i32
        + face.line_height as i32 * (layout.lines.len() as i32 - 1).max(0);
    let top = zone.y as i32 + (zone.height as i32 - block) / 2;
    layout
        .lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let line_width = drawn(layout.scale, line, zone.width) as i32;
            (
                zone.x as i32 + (zone.width as i32 - line_width) / 2,
                top + index as i32 * face.line_height as i32,
            )
        })
        .collect()
}

/// One key as the operator sees it: 96x96, tightly packed RGB, row-major from top left.
#[derive(Clone, PartialEq, Eq)]
pub struct Canvas {
    rgb: Vec<u8>,
}
impl Canvas {
    fn new(color: Rgb) -> Self {
        Self {
            rgb: color.repeat(SIZE * SIZE),
        }
    }
    pub fn rgb(&self) -> &[u8] {
        &self.rgb
    }
    /// Turned by 180 degrees, as the XL expects its key images.
    pub fn rotated(&self) -> Vec<u8> {
        self.rgb.rchunks_exact(3).flatten().copied().collect()
    }
    pub fn pixel(&self, x: usize, y: usize) -> Rgb {
        let at = (y * SIZE + x) * 3;
        [self.rgb[at], self.rgb[at + 1], self.rgb[at + 2]]
    }
    /// Blends `color` at `alpha`/255; coordinates outside the key are ignored.
    fn blend(&mut self, x: i32, y: i32, color: Rgb, alpha: u32) {
        if x < 0 || y < 0 || x >= SIZE as i32 || y >= SIZE as i32 || alpha == 0 {
            return;
        }
        let at = (y as usize * SIZE + x as usize) * 3;
        for (channel, value) in self.rgb[at..at + 3].iter_mut().zip(color) {
            *channel = ((*channel as u32 * (255 - alpha) + value as u32 * alpha + 127) / 255) as u8;
        }
    }
    fn fill(&mut self, x: usize, y: usize, width: usize, height: usize, color: Rgb) {
        for row in y..(y + height).min(SIZE) {
            let from = (row * SIZE + x.min(SIZE)) * 3;
            let to = (row * SIZE + (x + width).min(SIZE)) * 3;
            for pixel in self.rgb[from..to].chunks_exact_mut(3) {
                pixel.copy_from_slice(&color);
            }
        }
    }
    /// A rectangular outline `thickness` wide, `inset` pixels in from the key edge.
    fn outline(&mut self, inset: usize, thickness: usize, color: Rgb) {
        let side = SIZE - 2 * inset;
        self.fill(inset, inset, side, thickness, color);
        self.fill(inset, SIZE - inset - thickness, side, thickness, color);
        self.fill(inset, inset, thickness, side, color);
        self.fill(SIZE - inset - thickness, inset, thickness, side, color);
    }
    /// `thumb` must be `THUMB_WIDTH` x `THUMB_HEIGHT` RGB.
    /// A large red X over everything above the foot: this one is marked for removal.
    fn cross(&mut self) {
        let (width, height) = (SIZE as i32, (SIZE - FOOT) as i32);
        for y in 4..height - 4 {
            // Where each stroke crosses this row, and half its width along the row.
            let along = (y - 4) * (width - 20) / (height - 9) + 10;
            for x in 0..width {
                if (x - along).abs() <= 6 || (x - (width - 1 - along)).abs() <= 6 {
                    self.blend(x, y, CROSS, 235);
                }
            }
        }
    }
    fn picture(&mut self, thumb: &[u8], dim: bool) {
        let bytes = THUMB_WIDTH * THUMB_HEIGHT * 3;
        self.rgb[..bytes].copy_from_slice(thumb);
        if dim {
            for value in &mut self.rgb[..bytes] {
                *value = (*value as u16 * 3 / 10) as u8;
            }
        }
    }
    /// A solid arrowhead centred on the label band, hugging the left or right safe edge.
    fn arrow(&mut self, left: bool, color: Rgb) {
        let middle = (HEAD + (SIZE - HEAD - FOOT) / 2) as i32;
        let half = ARROW as i32;
        for dy in -half..=half {
            // Quarter-pixel edge position gives the slanted sides a clean antialiased edge.
            let reach = (half - dy.abs()) * 4 * ARROW as i32 / half + 2;
            for step in 0..=reach / 4 {
                let alpha = if step == reach / 4 { (reach % 4) as u32 * 64 } else { 255 };
                let x = if left {
                    (INSET + ARROW) as i32 - 1 - step
                } else {
                    (SIZE - INSET - ARROW) as i32 + step
                };
                self.blend(x, middle + dy, color, alpha);
            }
        }
    }
    fn text(&mut self, layout: &Layout, zone: Zone, color: Rgb, alpha: u32) {
        let face = layout.scale.face();
        let cap = face.cap_height as usize;
        // Each line is rasterised into its own coverage strip, which leaves room above
        // and below the capitals for brackets and tails, then blended in one pass.
        let (margin, rise) = (2, cap / 3 + 1);
        let rows = cap + 2 * rise;
        for (line, (left, top)) in layout.lines.iter().zip(place(layout, zone)) {
            let natural = width(layout.scale, line);
            let columns = natural + 2 * margin;
            let mut strip = vec![0_u8; columns * rows];
            let mut put = |x: i32, y: i32, coverage: u8| {
                if x >= 0 && y >= 0 && (x as usize) < columns && (y as usize) < rows {
                    let cell = &mut strip[y as usize * columns + x as usize];
                    *cell = cell.saturating_add(coverage);
                }
            };
            let baseline = (rise + cap) as i32;
            let mut pen = margin * 4;
            for ch in line.chars() {
                let x = ((pen + 2) / 4) as i32;
                match glyph(face, ch) {
                    Some(glyph) => {
                        let bitmap = &font::COVERAGE[glyph.offset as usize..];
                        for row in 0..glyph.height as usize {
                            for column in 0..glyph.width as usize {
                                put(
                                    x + glyph.left as i32 + column as i32,
                                    baseline - glyph.top as i32 + row as i32,
                                    bitmap[row * glyph.width as usize + column],
                                );
                            }
                        }
                    }
                    None => {
                        let stroke = (cap / 8).max(1);
                        let wide = box_width(face);
                        for row in 0..cap {
                            for column in 0..wide {
                                let edge = row < stroke
                                    || column < stroke
                                    || row >= cap - stroke
                                    || column >= wide - stroke;
                                if edge {
                                    put(x + 1 + column as i32, (rise + row) as i32, 255);
                                }
                            }
                        }
                    }
                }
                pen += advance(face, ch);
            }
            // Destination columns each average `step`/256 strip columns: 256 is natural
            // width, more condenses the line into the zone.
            let target = drawn(layout.scale, line, zone.width);
            let step = (natural * 256).div_ceil(target.max(1)).max(256);
            for column in 0..(columns * 256).div_ceil(step) {
                let (from, to) = (column * step, ((column + 1) * step).min(columns * 256));
                for row in 0..rows {
                    let mut sum = 0;
                    let mut at = from;
                    while at < to {
                        let next = ((at / 256 + 1) * 256).min(to);
                        sum += strip[row * columns + at / 256] as usize * (next - at);
                        at = next;
                    }
                    self.blend(
                        left + column as i32 - (margin * 256 / step) as i32,
                        top + row as i32 - rise as i32,
                        color,
                        (sum / step) as u32 * alpha / 255,
                    );
                }
            }
        }
    }
}

fn mix(from: Rgb, to: Rgb, amount: u32) -> Rgb {
    std::array::from_fn(|i| ((from[i] as u32 * (255 - amount) + to[i] as u32 * amount) / 255) as u8)
}
/// WCAG relative luminance, in thousandths.
fn luminance(color: Rgb) -> u32 {
    let linear = |value: u8| {
        let v = value as f32 / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    ((0.2126 * linear(color[0]) + 0.7152 * linear(color[1]) + 0.0722 * linear(color[2])) * 1000.0)
        as u32
}
/// White or near-black, whichever contrasts more with `background`.
fn ink(background: Rgb) -> Rgb {
    // Equal contrast against white and against INK falls at a luminance of about 0.18.
    if luminance(background) > 180 {
        INK
    } else {
        WHITE
    }
}
/// A manifest colour made bright enough to draw lines and text on a blank key.
fn bright(color: Rgb) -> Rgb {
    if luminance(color) < 180 {
        mix(color, WHITE, 150)
    } else {
        color
    }
}
fn is_index(label: &str) -> bool {
    label.len() > 1 && label.starts_with('#') && label[1..].bytes().all(|b| b.is_ascii_digit())
}
/// Kinds that show a picture when they have one.
fn pictured(key: &KeyDescriptor) -> bool {
    matches!(key.kind, KeyKind::Clip | KeyKind::Shader)
}
fn has_label(key: &KeyDescriptor, thumbnail: bool) -> bool {
    key.kind != KeyKind::Empty && !(pictured(key) && thumbnail)
}

/// The scale each key's main label is drawn at. Each label is fitted on its own, then
/// keys of one kind settle on the largest scale that at least three in four of them can
/// use, so a page does not mix type sizes within a kind; only a rare over-long label
/// steps down by itself. `#123` index labels form their own group and stay large
/// beside longer names.
pub fn scales(keys: &[KeyDescriptor], thumbnails: &[bool]) -> Vec<Scale> {
    let group = |key: &KeyDescriptor| (key.kind, is_index(&key.label));
    let own: Vec<Option<Scale>> = keys
        .iter()
        .enumerate()
        .map(|(index, key)| {
            has_label(key, thumbnails.get(index).copied().unwrap_or(false)).then(|| {
                let zone = label_zone(key);
                let largest = if key.kind == KeyKind::Navigate { Scale::Medium } else { Scale::Large };
                fit(&key.label, zone.width, zone.height, largest).scale
            })
        })
        .collect();
    keys.iter()
        .zip(&own)
        .map(|(key, scale)| {
            let peers: Vec<Scale> = keys
                .iter()
                .zip(&own)
                .filter(|(other, _)| group(other) == group(key))
                .filter_map(|(_, scale)| *scale)
                .collect();
            let shared = Scale::DESCENDING
                .into_iter()
                .find(|step| peers.iter().filter(|peer| *peer >= step).count() * 4 >= peers.len() * 3)
                .unwrap_or(Scale::Small);
            scale.map_or(shared, |own| own.min(shared))
        })
        .collect()
}

/// Draws one key. `thumbnail` is `THUMB_WIDTH` x `THUMB_HEIGHT` RGB; `largest` comes from
/// [`scales`] (pass `Scale::Large` to fit the key on its own).
pub fn draw(
    key: &KeyDescriptor,
    thumbnail: Option<&[u8]>,
    largest: Scale,
    renderer_online: bool,
) -> Canvas {
    // A wrongly sized picture is ignored rather than drawn skewed.
    let thumbnail = thumbnail
        .filter(|rgb| pictured(key) && rgb.len() == THUMB_WIDTH * THUMB_HEIGHT * 3);
    let outlined = matches!(key.kind, KeyKind::Empty | KeyKind::Info);
    let body = if outlined { BLANK } else { key.color };
    let mut canvas = Canvas::new(body);
    let attention = matches!(key.state, KeyState::Active | KeyState::Loading | KeyState::Failed);
    let faded = if key.state == KeyState::Unavailable { 130 } else { 255 };
    // Head and foot hold one small line; anything longer ends in an ellipsis.
    let caption = |text: &str| fit(text, SAFE, FOOT, Scale::Small);
    match key.kind {
        // Flat and aligned to the JPEG's 8 px blocks, so an empty key stays one HID packet.
        KeyKind::Empty => canvas.fill(8, 8, SIZE - 16, SIZE - 16, BLANK_PANEL),
        KeyKind::Info => canvas.outline(INSET - 2, 2, mix(BLANK, bright(key.color), 150)),
        KeyKind::Danger => canvas.outline(GUARD, 2, mix(key.color, ink(key.color), 110)),
        KeyKind::Navigate => {
            if let Some(Command::Page { delta }) = key.command {
                canvas.arrow(delta < 0, ink(body));
            }
        }
        KeyKind::Clip | KeyKind::Shader | KeyKind::Setting => {}
    }
    if let Some(thumb) = thumbnail {
        // Picture keys carry their name, or their state, in the foot.
        canvas.picture(thumb, key.state == KeyState::Unavailable);
        let band = if key.state == KeyState::Idle { CAPTION_BAND } else { key.color };
        canvas.fill(0, SIZE - FOOT, SIZE, FOOT, band);
        let words = if key.caption.is_empty() { &key.label } else { &key.caption };
        canvas.text(&caption(words), FOOT_ZONE, ink(band), 255);
    } else if has_label(key, false) {
        let zone = label_zone(key);
        let color = if key.kind == KeyKind::Info { bright(key.color) } else { ink(body) };
        canvas.text(&fit(&key.label, zone.width, zone.height, largest), zone, color, faded);
        if !key.caption.is_empty() {
            if attention {
                // A white plate joined to the frame: the status word must be read first.
                canvas.fill(0, SIZE - FOOT, SIZE, FOOT, WHITE);
                canvas.text(&caption(&key.caption), FOOT_ZONE, INK, 255);
            } else {
                canvas.text(&caption(&key.caption), FOOT_ZONE, ink(body), 200);
            }
        }
    }
    if key.crossed {
        canvas.cross();
    }
    if attention {
        // A picture key has no body to colour, so its frame carries the state colour.
        let frame = if thumbnail.is_some() { key.color } else { WHITE };
        canvas.outline(0, FRAME, frame);
    }
    if !renderer_online {
        // Keys keep their bindings while the renderer recovers; the banner has its own band.
        canvas.fill(0, 0, SIZE, HEAD, OFFLINE);
        canvas.text(&caption("OFFLINE"), HEAD_ZONE, ink(OFFLINE), 255);
    }
    canvas
}

/// A whole page with harmonised type; `thumbnails[i]` belongs to `keys[i]`.
pub fn page(
    keys: &[KeyDescriptor],
    thumbnails: &[Option<&[u8]>],
    renderer_online: bool,
) -> Vec<Canvas> {
    let present: Vec<bool> = thumbnails.iter().map(Option::is_some).collect();
    keys.iter()
        .zip(scales(keys, &present))
        .enumerate()
        .map(|(index, (key, scale))| {
            draw(key, thumbnails.get(index).copied().flatten(), scale, renderer_online)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    const LABEL: Zone = Zone {
        x: INSET,
        y: HEAD,
        width: SAFE,
        height: SIZE - HEAD - FOOT,
    };
    fn fitted(label: &str) -> Layout {
        fit(label, LABEL.width, LABEL.height, Scale::Large)
    }
    fn lines(label: &str) -> Vec<String> {
        fitted(label).lines
    }
    fn key(label: &str, command: Option<Command>, kind: KeyKind) -> KeyDescriptor {
        KeyDescriptor {
            label: label.into(),
            command,
            thumbnail: None,
            color: [25, 75, 120],
            kind,
            state: KeyState::Idle,
            caption: String::new(),
            crossed: false,
        }
    }
    /// Bounding box (left, top, right, bottom) of pixels that differ from `background`.
    fn extent(canvas: &Canvas, background: Rgb) -> Option<(usize, usize, usize, usize)> {
        let mut found: Option<(usize, usize, usize, usize)> = None;
        for y in 0..SIZE {
            for x in 0..SIZE {
                if canvas.pixel(x, y) != background {
                    let (l, t, r, b) = found.unwrap_or((x, y, x, y));
                    found = Some((l.min(x), t.min(y), r.max(x), b.max(y)));
                }
            }
        }
        found
    }
    #[test]
    fn embedded_font_is_complete_and_inside_its_atlas() {
        for scale in Scale::DESCENDING {
            let face = scale.face();
            assert!(face.glyphs.windows(2).all(|pair| pair[0].ch < pair[1].ch));
            for ch in (' '..='~').filter(|ch| !ch.is_ascii_lowercase()).chain(['…']) {
                let glyph = glyph(face, ch).unwrap_or_else(|| panic!("{scale:?} lacks {ch:?}"));
                let end = glyph.offset as usize + glyph.width as usize * glyph.height as usize;
                assert!(end <= font::COVERAGE.len());
                assert!(glyph.advance > 0);
            }
        }
        assert!(font::SMALL.cap_height < font::MEDIUM.cap_height);
        assert!(font::MEDIUM.cap_height < font::LARGE.cap_height);
    }
    #[test]
    fn short_labels_are_large_and_longer_ones_step_down() {
        assert_eq!(fitted("#301").scale, Scale::Large);
        assert_eq!(fitted("tap").scale, Scale::Large);
        assert_eq!(fitted("4tBXRh").scale, Scale::Medium);
        assert_eq!(fitted("BEATTUNNEL").scale, Scale::Small);
        for label in ["#301", "RESET", "BEATTUNNEL", "OVERLAY SCREEN", "1 BDAY VIDEO 1"] {
            let layout = fitted(label);
            assert!(!layout.truncated, "{label}");
            assert!(layout.lines.len() <= layout.scale.lines(LABEL.height), "{label}");
            for line in &layout.lines {
                assert!(width(layout.scale, line) <= LABEL.width, "{label}: {line}");
            }
        }
    }
    #[test]
    fn a_line_just_too_wide_is_condensed_instead_of_stepping_down() {
        // Wider than the zone at Medium, but within the condensing allowance.
        let layout = fitted("MULTIBAND");
        assert_eq!((layout.scale, layout.lines.len()), (Scale::Medium, 1));
        let natural = width(Scale::Medium, "MULTIBAND");
        assert!(natural > LABEL.width && natural * CONDENSE <= LABEL.width * 100);
        assert_eq!(drawn(Scale::Medium, "MULTIBAND", LABEL.width), LABEL.width - 2);
        let canvas = draw(&key("MULTIBAND", None, KeyKind::Shader), None, Scale::Large, true);
        let (left, _, right, _) = extent(&canvas, [25, 75, 120]).unwrap();
        assert!(left >= INSET && right < SIZE - INSET, "{left}..{right}");
        // The same holds for a phrase, so "FX AMT 100" matches its shorter siblings;
        // beyond the allowance the label wraps.
        assert_eq!(lines("FX AMT 100"), ["FX AMT 100"]);
        assert_eq!(fitted("FX AMT 100").scale, fitted("FX AMT 25").scale);
        assert_eq!(lines("OVERLAY SCREEN"), ["OVERLAY", "SCREEN"]);
        // A one-line band cannot wrap: a caption is cut with an ellipsis instead.
        let title = fit("Blend modes", SAFE, FOOT, Scale::Small);
        assert_eq!((title.lines.as_slice(), title.truncated), (&["BLEND MODES".to_string()][..], false));
        assert!(width(Scale::Small, "BLEND MODES") > SAFE);
        let long = fit("Blend modes and more", SAFE, FOOT, Scale::Small);
        assert!(long.truncated && long.lines.len() == 1 && long.lines[0].ends_with('…'));
        assert!(width(Scale::Small, &long.lines[0]) <= SAFE);
    }
    #[test]
    fn spacing_is_proportional() {
        assert!(width(Scale::Medium, "III") < width(Scale::Medium, "WWW"));
        assert_eq!(width(Scale::Small, ""), 0);
    }
    #[test]
    fn wrapping_breaks_at_words_and_balances_lines() {
        assert_eq!(lines("OVERLAY SCREEN"), ["OVERLAY", "SCREEN"]);
        assert_eq!(lines("audio_oscilloscope"), ["AUDIO", "OSCILLOSCOPE"]);
        assert_eq!(lines("  speed   - "), ["SPEED -"]);
        assert_eq!(lines("1 BDAY VIDEO 1"), ["1 BDAY", "VIDEO 1"]);
        assert_eq!(fitted("1 BDAY VIDEO 1").scale, Scale::Medium);
        // Greedy wrapping would fill the first line and strand the last word.
        let words = ["AAAA", "BBBB", "CCCC", "DDDD"];
        let limit = width(Scale::Small, "AAAA BBBB CCCC");
        assert_eq!(wrap(Scale::Small, &words, limit).unwrap(), ["AAAA BBBB", "CCCC DDDD"]);
        assert!(wrap(Scale::Small, &words, width(Scale::Small, "AAA")).is_none());
        // Siblings break at the same place even when another split is a pixel narrower.
        let split = wrap(Scale::Large, &["FX", "AMT", "25"], LABEL.width + 10).unwrap();
        assert_eq!(split, ["FX AMT", "25"]);
        let three = fitted("beattunnel spectrum analyser");
        assert_eq!(three.scale, Scale::Small);
        assert_eq!(three.lines, ["BEATTUNNEL", "SPECTRUM", "ANALYSER"]);
    }
    #[test]
    fn overlong_labels_break_inside_words_then_end_in_an_ellipsis() {
        let word = fitted("SUPERCALIFRAGILISTIC");
        assert_eq!((word.scale, word.truncated), (Scale::Small, false));
        assert!(word.lines.len() > 1);
        assert_eq!(word.lines.concat(), "SUPERCALIFRAGILISTIC");
        let long = fitted(&"very long label ".repeat(400));
        assert_eq!((long.scale, long.truncated), (Scale::Small, true));
        assert_eq!(long.lines.len(), 3);
        assert!(long.lines[2].ends_with('…'));
        for layout in [&word, &long] {
            for line in &layout.lines {
                assert!(width(layout.scale, line) <= LABEL.width, "{line}");
            }
        }
        // A zone too narrow for a single glyph still terminates with one glyph per line.
        assert_eq!(fit("WWW", 2, 56, Scale::Large).lines.len(), 3);
    }
    #[test]
    fn empty_and_non_ascii_labels_are_handled() {
        for label in ["", "   ", "_", "\t\n"] {
            assert!(lines(label).is_empty(), "{label:?}");
        }
        assert_eq!(normalize("Café  déjà_vu"), "CAFE DEJA VU");
        assert_eq!(normalize("straße"), "STRASSE");
        // No glyphs for these: each becomes a box with a real advance, so layout still holds.
        let layout = fitted("東京 ナイト 🎛");
        assert!(!layout.lines.is_empty());
        for line in &layout.lines {
            let natural = width(layout.scale, line);
            assert!(natural > 0 && natural * CONDENSE <= LABEL.width * 100);
        }
        let canvas = draw(&key("東京", None, KeyKind::Shader), None, Scale::Large, true);
        assert!(extent(&canvas, [25, 75, 120]).is_some());
    }
    #[test]
    fn labels_are_centred_in_their_zone_at_every_scale() {
        for label in ["#301", "RESET", "MULTIBAND", "OPACITY 100", "A B C D E F G H I J K L"] {
            let layout = fitted(label);
            let face = layout.scale.face();
            let spots = place(&layout, LABEL);
            let top = spots[0].1;
            let bottom = spots[spots.len() - 1].1 + face.cap_height as i32;
            let above = top - LABEL.y as i32;
            let below = (LABEL.y + LABEL.height) as i32 - bottom;
            assert!(above >= 0 && (above - below).abs() <= 1, "{label}: {above} {below}");
            for (line, (left, _)) in layout.lines.iter().zip(&spots) {
                let before = left - LABEL.x as i32;
                let after = (LABEL.x + LABEL.width) as i32
                    - left
                    - drawn(layout.scale, line, LABEL.width) as i32;
                assert!(before >= 0 && (before - after).abs() <= 1, "{label}: {before} {after}");
            }
        }
    }
    #[test]
    fn drawn_label_sits_in_the_middle_of_the_key_and_inside_the_safe_area() {
        for label in ["#301", "HOLD", "kaleidosco", "OPACITY 100"] {
            let canvas = draw(&key(label, None, KeyKind::Shader), None, Scale::Large, true);
            let (left, top, right, bottom) = extent(&canvas, [25, 75, 120]).unwrap();
            assert!(left >= INSET && right < SIZE - INSET, "{label}: {left}..{right}");
            assert!(top >= HEAD && bottom < SIZE - FOOT, "{label}: {top}..{bottom}");
            let (centre_x, centre_y) = (left + right, top + bottom);
            assert!(centre_x.abs_diff(SIZE - 1) <= 4, "{label}: x {centre_x}");
            assert!(centre_y.abs_diff(SIZE - 1) <= 3, "{label}: y {centre_y}");
        }
    }
    #[test]
    fn keys_of_one_kind_share_a_scale_and_indices_stay_large() {
        let shader = |label| key(label, Some(Command::TriggerShader { shader: 0 }), KeyKind::Shader);
        let keys = [
            shader("#301"),
            shader("4tBXRh"),
            shader("beattunnel"),
            key("TAP", Some(Command::ToggleHud), KeyKind::Setting),
            key("", None, KeyKind::Empty),
            key("3 / 9", Some(Command::Page { delta: 1 }), KeyKind::Navigate),
        ];
        assert_eq!(
            scales(&keys, &[false; 6]),
            [
                Scale::Large,
                Scale::Small,
                Scale::Small,
                Scale::Large,
                Scale::Large,
                Scale::Medium
            ]
        );
        // One over-long name in a page of short ones steps down alone.
        let mut names: Vec<KeyDescriptor> = ["4tBXRh", "Ds3fDX", "ltXyD7", "beattunnel"]
            .map(shader)
            .into();
        assert_eq!(
            scales(&names, &[false; 4]),
            [Scale::Medium, Scale::Medium, Scale::Medium, Scale::Small]
        );
        // A clip showing its thumbnail has no main label and does not vote.
        names.push(key("x", Some(Command::TriggerClip { clip: 0 }), KeyKind::Clip));
        names.push(key("a much longer clip name", Some(Command::TriggerClip { clip: 1 }), KeyKind::Clip));
        assert_eq!(scales(&names, &[false, false, false, false, false, true])[4], Scale::Large);
    }
    #[test]
    fn state_is_shown_by_more_than_colour() {
        let mut live = key("#301", Some(Command::TriggerShader { shader: 0 }), KeyKind::Shader);
        let idle = draw(&live, None, Scale::Large, true);
        live.state = KeyState::Active;
        live.caption = "LIVE".into();
        // Same colour on purpose: frame and foot alone must tell the two apart.
        let active = draw(&live, None, Scale::Large, true);
        assert_ne!(idle.pixel(0, 0), active.pixel(0, 0));
        assert_ne!(idle.pixel(1, SIZE - FOOT + 1), active.pixel(FRAME + 1, SIZE - FOOT + 1));
        // The label itself does not move when state changes.
        for y in HEAD..SIZE - FOOT {
            for x in FRAME..SIZE - FRAME {
                assert_eq!(idle.pixel(x, y), active.pixel(x, y));
            }
        }
        let offline = draw(&live, None, Scale::Large, false);
        assert_eq!(offline.pixel(SIZE / 2, 1), OFFLINE);
        assert_eq!(offline.pixel(SIZE / 2, HEAD + 1), active.pixel(SIZE / 2, HEAD + 1));
    }
    #[test]
    fn thumbnail_keys_put_the_picture_above_a_caption_band() {
        let thumb = [200_u8, 100, 50].repeat(THUMB_WIDTH * THUMB_HEIGHT);
        let mut clip = key("12 Laser", Some(Command::TriggerClip { clip: 0 }), KeyKind::Clip);
        let canvas = draw(&clip, Some(&thumb), Scale::Large, true);
        assert_eq!(canvas.pixel(48, 40), [200, 100, 50]);
        assert_eq!(canvas.pixel(1, SIZE - 2), CAPTION_BAND);
        clip.state = KeyState::Loading;
        clip.color = [180, 110, 15];
        clip.caption = "LOADING".into();
        let canvas = draw(&clip, Some(&thumb), Scale::Large, true);
        assert_eq!(canvas.pixel(0, 40), [180, 110, 15]);
        assert_eq!(canvas.pixel(48, 40), [200, 100, 50]);
        // A wrongly sized picture falls back to the plain label anatomy.
        let canvas = draw(&clip, Some(&thumb[3..]), Scale::Large, true);
        assert_eq!(canvas.pixel(48, 5), [180, 110, 15]);
    }
    #[test]
    fn empty_keys_are_blank_but_visibly_deliberate_and_rotation_is_a_half_turn() {
        let canvas = draw(&key("", None, KeyKind::Empty), None, Scale::Large, true);
        assert_eq!(canvas.pixel(48, 48), BLANK_PANEL);
        assert_eq!((canvas.pixel(7, 48), canvas.pixel(8, 48)), (BLANK, BLANK_PANEL));
        assert_eq!((canvas.pixel(48, 87), canvas.pixel(48, 88)), (BLANK_PANEL, BLANK));
        let mut marked = canvas.clone();
        marked.blend(3, 7, WHITE, 255);
        assert!(marked != canvas);
        let turned = marked.rotated();
        let at = ((SIZE - 1 - 7) * SIZE + (SIZE - 1 - 3)) * 3;
        assert_eq!(turned[at..at + 3], WHITE);
        assert_eq!(turned.len(), SIZE * SIZE * 3);
    }
}
