//! RPi-VJ Desktop, phase 4: a composition of layers, each a row of cells holding shaders
//! or video clips with a play time each, played by hand, in sequence or shuffled; a
//! library with folders, a checked list that plays by itself and presets of it;
//! per-layer opacity, blend, effect and react mode; MIDI control by learning;
//! compositions saved to and opened from files; a small preview; projection on a
//! chosen monitor.
mod audio_in;
mod gfx;
mod library;
mod midi;
mod mpv;

use anyhow::Result;
use eframe::egui;
use rpi_vj_native::{
    audio::{
        idle_audio, AudioFrame, BeatClock, BeatPredictor, Reactor, SectionDetector, BEAT_LEAD, FLOW_RATE,
        REACT_MODES, SPEC_FLOW_RATE,
    },
    shader,
    texture::Channel,
};
use std::{
    collections::{BTreeSet, HashMap},
    path::PathBuf,
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};

const RENDER_SIZES: [(u32, u32); 3] = [(1280, 720), (1920, 1080), (2560, 1440)];
const THUMB: (u32, u32) = (192, 108);
const CELL: egui::Vec2 = egui::vec2(160.0, 112.0);
const CELL_PICTURE: f32 = 90.0;
const IMAGE_TYPES: [&str; 6] = ["png", "jpg", "jpeg", "gif", "webp", "bmp"];
/// A file a layer can show: a video clip or a picture.
fn is_media(path: &std::path::Path) -> bool {
    let kind = path.extension().and_then(|kind| kind.to_str()).unwrap_or("").to_lowercase();
    CLIP_TYPES.contains(&kind.as_str()) || IMAGE_TYPES.contains(&kind.as_str())
}
/// Put the window with this title exactly over a rectangle of the desktop, in screen
/// pixels, and say where it was found. Positions given through the UI library are in
/// points of one screen's scaling, which lands a window in the wrong place on a screen
/// scaled differently; the system call takes pixels and has the last word.
#[cfg(windows)]
fn place_window(title: &str, position: (i32, i32), size: (u32, u32)) -> Option<[i32; 4]> {
    use windows_sys::Win32::{
        Foundation::{HWND, RECT},
        UI::WindowsAndMessaging::{FindWindowW, GetWindowRect, SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER},
    };
    let wide: Vec<u16> = title.encode_utf16().chain([0]).collect();
    // SAFETY: plain calls with a terminated string and a rectangle that outlives them.
    unsafe {
        let window = FindWindowW(std::ptr::null(), wide.as_ptr());
        if window as usize == 0 {
            return None;
        }
        let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        GetWindowRect(window, &mut rect);
        let found = [rect.left, rect.top, rect.right, rect.bottom];
        let wanted = [position.0, position.1, position.0 + size.0 as i32, position.1 + size.1 as i32];
        if found != wanted {
            let none: HWND = std::mem::zeroed();
            SetWindowPos(window, none, position.0, position.1, size.0 as i32, size.1 as i32, SWP_NOACTIVATE | SWP_NOZORDER);
        }
        Some(found)
    }
}
#[cfg(not(windows))]
fn place_window(_: &str, _: (i32, i32), _: (u32, u32)) -> Option<[i32; 4]> {
    None
}
/// What undo compares and brings back: the saved state without the settings of this
/// computer and its devices, and without which cells happen to be playing.
fn undo_view(full: &serde_json::Value) -> serde_json::Value {
    let mut view = full.clone();
    if let Some(map) = view.as_object_mut() {
        for key in ["gain", "lead_ms", "size", "output_display", "audio", "detached", "led", "apc", "midi", "assets_dir"] {
            map.remove(key);
        }
    }
    for layer in view["layers"].as_array_mut().into_iter().flatten() {
        if let Some(layer) = layer.as_object_mut() {
            layer.remove("active");
        }
    }
    view
}
/// What was dropped on a widget, if it is of this kind. Asking egui for a payload of
/// one kind throws away a payload of another, so the kind is looked at first.
fn dropped<T: std::any::Any + Send + Sync>(response: &egui::Response) -> Option<Arc<T>> {
    if egui::DragAndDrop::has_payload_of_type::<T>(&response.ctx) {
        response.dnd_release_payload::<T>()
    } else {
        None
    }
}
/// A still picture of a file for the Assets tab, as RGB of the given size: pictures are
/// read directly, clips through `ffmpeg` if it is installed. Kept on disk once made.
fn asset_thumb(cache: &std::path::Path, path: &std::path::Path) -> Option<(Vec<u8>, usize, usize)> {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    std::fs::metadata(path).ok()?.modified().ok()?.hash(&mut hasher);
    let kept = cache.join(format!("{:016x}.jpg", hasher.finish()));
    if !kept.is_file() {
        std::fs::create_dir_all(cache).ok()?;
        let kind = path.extension().and_then(|kind| kind.to_str()).unwrap_or("").to_lowercase();
        if IMAGE_TYPES.contains(&kind.as_str()) {
            image::open(path).ok()?.thumbnail(192, 108).to_rgb8().save(&kept).ok()?;
        } else {
            // A frame from a second in; a clip shorter than that gives its first frame.
            let grab = |seek: &str| {
                std::process::Command::new("ffmpeg")
                    .args(["-v", "error", "-y", "-ss", seek, "-i"])
                    .arg(path)
                    .args(["-frames:v", "1", "-vf", "scale=192:-2"])
                    .arg(&kept)
                    .stdin(std::process::Stdio::null())
                    .status()
                    .is_ok_and(|status| status.success())
                    && kept.is_file()
            };
            if !grab("1") && !grab("0") {
                return None;
            }
        }
    }
    let picture = image::open(&kept).ok()?.to_rgb8();
    let size = (picture.width() as usize, picture.height() as usize);
    Some((picture.into_raw(), size.0, size.1))
}
/// Whether a slider was double-clicked. A slider itself only notices drags, so this
/// lays a click over it; drags still reach the slider.
fn double_clicked(ui: &egui::Ui, slider: &egui::Response) -> bool {
    ui.interact(slider.rect, slider.id.with("reset"), egui::Sense::click()).double_clicked()
}
/// Where an effect chain sits.
#[derive(Clone, Copy, PartialEq)]
enum ChainAt {
    Cell(usize, usize),
    Layer(usize),
    Composition,
}
/// Assets gathered from anywhere on the computer under one name.
struct Collection {
    name: String,
    items: Vec<PathBuf>,
}
/// How a layer moves on by itself, in the order of `Layer::auto`.
const AUTO_MODES: [&str; 3] = ["Manual", "Sequence", "Shuffle"];
/// A change of song section moves on only after this long on one cell, seconds.
const SECTION_MIN: f32 = 4.0;
const HEADER_WIDTH: f32 = 250.0;
const CLIP_TYPES: [&str; 6] = ["mp4", "mov", "mkv", "webm", "avi", "m4v"];
/// A render target's first row is its bottom one; this turns it the right way up.
const FLIPPED: egui::Rect = egui::Rect { min: egui::pos2(0.0, 1.0), max: egui::pos2(1.0, 0.0) };
const RED: egui::Color32 = egui::Color32::from_rgb(235, 90, 90);
const GREEN: egui::Color32 = egui::Color32::from_rgb(60, 200, 90);

/// What the command line asks for besides the normal app; all of it is for testing, and
/// any of it keeps the saved composition out of the way.
#[derive(Default)]
struct Options {
    root: Option<PathBuf>,
    /// Compile every runnable shader, write a report and leave.
    compile_all: bool,
    /// Put this shader id on the second layer and launch it.
    live: Option<String>,
    /// Put this clip on the first layer and launch it.
    clip: Option<PathBuf>,
    /// Put this many shaders of the Pi bank on the second layer.
    fill: usize,
    /// Effect number for the second layer.
    fx: usize,
    /// Play the second layer in sequence, a second per cell.
    sequence: bool,
    /// Check this many shaders of the Pi bank and play them from the library, a second each.
    autoplay: usize,
    /// Open the Effects tab, preview this shader id, detach the preview, draw every hover picture.
    tab: usize,
    preview: Option<String>,
    detach: bool,
    thumbs: bool,
    /// Drag the first row of the left panel's list onto the first cell of the second
    /// layer with a scripted pointer.
    drag_test: bool,
    /// Clear two cells, undo, redo, undo.
    undo_test: bool,
    /// Show the Assets tab as thumbnails, with its first folder opened.
    tiles: bool,
    /// Start with the grid scrolled this far sideways.
    scroll: f32,
    /// Open the editor for the first cell of the second layer.
    edit_test: bool,
    /// Put an effect on the first cell of the second layer.
    cell_fx: bool,
    /// Move two of the filled cells to the top layer.
    move_test: bool,
    /// Download these Shadertoy ids or links, print the log and leave.
    download: Option<String>,
    /// Give the previewed shader a look: zoom, turn, speed, colour and its first parameter.
    look_test: bool,
    /// Open this composition file at start.
    open: Option<PathBuf>,
    /// Bind and send a few MIDI messages, and take the checked list through a preset.
    midi_test: bool,
    /// Save the output picture, or a picture of the app window, here after `frames`
    /// frames and leave.
    snapshot: Option<PathBuf>,
    ui_snapshot: Option<PathBuf>,
    frames: u64,
    gain: Option<f32>,
    /// Project on this monitor (by number) at start.
    output: Option<usize>,
    /// Keep the composition in this file, whatever else is asked.
    state: Option<PathBuf>,
    /// Leave after this many seconds, printing the frame rate.
    exit_after: Option<f32>,
}
impl Options {
    fn parse() -> Self {
        let mut options = Self { frames: 90, ..Self::default() };
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            let mut value = || args.next().unwrap_or_default();
            match arg.as_str() {
                "--root" => options.root = Some(value().into()),
                "--compile-all" => options.compile_all = true,
                "--live" => options.live = Some(value()),
                "--clip" => options.clip = Some(value().into()),
                "--fill" => options.fill = value().parse().unwrap_or(0),
                "--fx" => options.fx = value().parse().unwrap_or(0),
                "--sequence" => options.sequence = true,
                "--autoplay" => options.autoplay = value().parse().unwrap_or(0),
                "--midi-test" => options.midi_test = true,
                "--open" => options.open = Some(value().into()),
                "--edit-test" => options.edit_test = true,
                "--scroll" => options.scroll = value().parse().unwrap_or(0.0),
                "--cell-fx" => options.cell_fx = true,
                "--drag-test" => options.drag_test = true,
                "--tiles" => options.tiles = true,
                "--undo-test" => options.undo_test = true,
                "--move-test" => options.move_test = true,
                "--download" => options.download = Some(value()),
                "--look-test" => options.look_test = true,
                "--tab" => options.tab = value().parse().unwrap_or(0),
                "--preview" => options.preview = Some(value()),
                "--detach" => options.detach = true,
                "--thumbs" => options.thumbs = true,
                "--snapshot" => options.snapshot = Some(value().into()),
                "--ui-snapshot" => options.ui_snapshot = Some(value().into()),
                "--gain" => options.gain = value().parse().ok(),
                "--frames" => options.frames = value().parse().unwrap_or(90),
                "--output" => options.output = value().parse().ok(),
                "--state" => options.state = Some(value().into()),
                "--exit-after" => options.exit_after = value().parse().ok(),
                other => eprintln!("unknown option {other}"),
            }
        }
        options
    }
    fn testing(&self) -> bool {
        self.compile_all
            || self.live.is_some()
            || self.clip.is_some()
            || self.fill > 0
            || self.autoplay > 0
            || self.open.is_some()
            || self.preview.is_some()
            || self.thumbs
            || self.download.is_some()
            || self.snapshot.is_some()
            || self.ui_snapshot.is_some()
            || self.exit_after.is_some()
    }
}
struct Display {
    label: String,
    /// Position and size in physical pixels.
    position: (i32, i32),
    size: (u32, u32),
}
fn displays() -> Vec<Display> {
    display_info::DisplayInfo::all()
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .map(|(index, display)| Display {
            label: format!(
                "{}: {}x{}{}",
                index + 1,
                display.width,
                display.height,
                if display.is_primary { " (main)" } else { "" }
            ),
            position: (display.x, display.y),
            size: (display.width, display.height),
        })
        .collect()
}
/// What a cell holds: a shader, by its place in the library, or a video clip.
#[derive(Clone, PartialEq)]
enum Source {
    Shader(usize),
    Clip(PathBuf),
}
/// What is being dragged: from the library, or files chosen for a layer.
struct Dragged(Vec<Source>);
/// Cells of the composition being dragged to another place, as (layer, column).
struct DraggedCells(Vec<(usize, usize)>);
/// An effect being dragged from the Effects tab, by its number.
struct DraggedEffect(usize);
/// One link of an effect chain.
#[derive(Clone)]
struct Effect {
    /// Index into `gfx::EFFECTS`.
    kind: usize,
    amount: f32,
    bypass: bool,
}
/// How one shader is adjusted wherever it plays: its place and size, its pace and how
/// far it follows the music, its colours, and the numbers its code declares up front.
#[derive(Clone, PartialEq)]
struct Look {
    zoom: f32,
    /// Degrees.
    rotate: f32,
    /// Across and up, in picture heights.
    pan: [f32; 2],
    speed: f32,
    react: f32,
    hue: f32,
    saturation: f32,
    brightness: f32,
    contrast: f32,
    gamma: f32,
    invert: bool,
    /// Its own react mode, which then wins over the cell's, the layer's and the autoplay's.
    react_mode: Option<u8>,
    params: Vec<(String, f32)>,
}
impl Default for Look {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            rotate: 0.0,
            pan: [0.0; 2],
            speed: 1.0,
            react: 1.0,
            hue: 0.0,
            saturation: 1.0,
            brightness: 1.0,
            contrast: 1.0,
            gamma: 1.0,
            invert: false,
            react_mode: None,
            params: Vec::new(),
        }
    }
}
impl Look {
    fn canvas(&self) -> [f32; 4] {
        let angle = self.rotate.to_radians();
        [angle.cos(), angle.sin(), self.pan[0], self.pan[1]]
    }
    /// The colour settings, when any of them is off its neutral value.
    fn grade(&self) -> Option<[f32; 6]> {
        let grade = [self.hue, self.saturation, self.brightness, self.contrast, self.gamma, f32::from(u8::from(self.invert))];
        (grade != [0.0, 1.0, 1.0, 1.0, 1.0, 0.0]).then_some(grade)
    }
    fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "zoom": self.zoom, "rotate": self.rotate, "pan": self.pan, "speed": self.speed, "react": self.react,
            "hue": self.hue, "saturation": self.saturation, "brightness": self.brightness, "contrast": self.contrast,
            "gamma": self.gamma, "invert": self.invert, "react_mode": self.react_mode, "params": self.params,
        })
    }
    fn from(saved: &serde_json::Value) -> Self {
        let number = |name: &str, otherwise: f32| saved[name].as_f64().map_or(otherwise, |value| value as f32);
        let pair = |value: &serde_json::Value| Some((value[0].as_str()?.to_owned(), value[1].as_f64()? as f32));
        Self {
            zoom: number("zoom", 1.0).clamp(0.05, 20.0),
            rotate: number("rotate", 0.0),
            pan: [saved["pan"][0].as_f64().unwrap_or(0.0) as f32, saved["pan"][1].as_f64().unwrap_or(0.0) as f32],
            speed: number("speed", 1.0),
            react: number("react", 1.0),
            hue: number("hue", 0.0),
            saturation: number("saturation", 1.0),
            brightness: number("brightness", 1.0),
            contrast: number("contrast", 1.0),
            gamma: number("gamma", 1.0),
            invert: saved["invert"].as_bool().unwrap_or(false),
            react_mode: saved["react_mode"].as_u64().map(|mode| (mode as u8).min(REACT_MODES.len() as u8 - 1)),
            params: saved["params"].as_array().into_iter().flatten().filter_map(pair).collect(),
        }
    }
}
/// A number a shader declares up front, `#define NAME 1.5` or `const float NAME = 1.5;`:
/// its name, its value, and whether it is written as a whole number.
fn param_of(line: &str) -> Option<(&str, f32, bool)> {
    let words: Vec<&str> = line.split_whitespace().collect();
    let (name, value) = match words.as_slice() {
        ["#define", name, value] => (*name, *value),
        ["const", "float" | "int", name, "=", value] => (*name, value.strip_suffix(';')?),
        _ => return None,
    };
    let plain = !name.is_empty() && name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
    // The two the converter writes into every file are not the shader's own.
    if !plain || ["HW_PERFORMANCE", "iFrameRate"].contains(&name) {
        return None;
    }
    let number = value.strip_suffix('f').unwrap_or(value);
    let starts = number.bytes().next().is_some_and(|byte| byte.is_ascii_digit() || byte == b'.' || byte == b'-');
    let whole = !number.contains('.') && !number.contains('e');
    Some((name, number.parse().ok().filter(|value: &f32| starts && value.is_finite())?, whole))
}
/// `text` with the declared numbers named in `params` set to their new values.
fn with_params(text: &str, params: &[(String, f32)]) -> String {
    if params.is_empty() {
        return text.to_owned();
    }
    let rewrite = |line: &str| {
        let (name, _, whole) = param_of(line)?;
        let value = params.iter().find(|(known, _)| known == name)?.1;
        let number = if whole { format!("{}", value.round() as i64) } else { format!("{value:?}") };
        Some(match line.split_whitespace().nth(1) {
            Some(kind) if !line.trim_start().starts_with('#') => format!("const {kind} {name} = {number};"),
            _ => format!("#define {name} {number}"),
        })
    };
    text.lines().map(|line| rewrite(line).unwrap_or_else(|| line.to_owned())).collect::<Vec<_>>().join("\n")
}
/// The levels a shader is fed, scaled: 0 is a shader that ignores the music.
fn scaled(audio: &AudioFrame, amount: f32) -> AudioFrame {
    let mut out = audio.clone();
    for level in [
        &mut out.bass,
        &mut out.mid,
        &mut out.treble,
        &mut out.volume,
        &mut out.onset,
        &mut out.sub,
        &mut out.low_mid,
        &mut out.high_mid,
    ] {
        *level = (*level * amount).clamp(0.0, 1.0);
    }
    out
}
/// The shader a layer is playing, if it is playing one.
fn playing(layer: &Layer) -> Option<usize> {
    match layer.active.and_then(|column| layer.cells[column].as_ref())?.source {
        Source::Shader(entry) => Some(entry),
        Source::Clip(_) => None,
    }
}
/// A layer's picture with its shader's colour settings applied, if it has any.
fn graded(gfx: &gfx::Gfx, looks: &HashMap<usize, Look>, layer: &Layer, into: &gfx::Target) -> glow::Texture {
    match playing(layer).and_then(|entry| looks.get(&entry)).and_then(Look::grade) {
        Some(grade) => {
            gfx.grade(into, layer.target.texture, grade);
            into.texture
        }
        None => layer.target.texture,
    }
}
/// Shadertoy ids out of pasted text: links, or bare six-character ids.
fn shadertoy_ids(text: &str) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for word in text.split(|c: char| c.is_whitespace() || c == ',' || c == ';') {
        let id: String = match word.find("view/") {
            Some(at) => word[at + 5..].chars().take_while(char::is_ascii_alphanumeric).collect(),
            None => word.to_owned(),
        };
        if id.len() == 6 && id.bytes().all(|byte| byte.is_ascii_alphanumeric()) && !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids
}
/// A download from Shadertoy running in the background, and what it has said so far.
struct Download {
    log: Arc<std::sync::Mutex<Vec<String>>>,
    done: Arc<std::sync::atomic::AtomicBool>,
    ids: Vec<String>,
}
/// Fetch, convert and catalogue `ids` with the repository's own tools, saying what
/// happens line by line.
fn run_download(root: PathBuf, ids: Vec<String>, log: Arc<std::sync::Mutex<Vec<String>>>) {
    use std::io::BufRead;
    let say = |line: String| {
        if let Ok(mut lines) = log.lock() {
            lines.push(line);
        }
    };
    let list = root.join("target/desktop/download-ids.txt");
    let incoming = root.join("target/desktop/incoming");
    let ready = std::fs::create_dir_all(&incoming).and_then(|()| std::fs::write(&list, ids.join("\n")));
    if let Err(error) = ready {
        return say(format!("could not write {}: {error}", list.display()));
    }
    let (list, folder) = (list.to_string_lossy().into_owned(), incoming.to_string_lossy().into_owned());
    let run = |what: &str, args: &[&str]| -> bool {
        say(format!("== {what}"));
        let started = std::process::Command::new("python")
            .args(args)
            .current_dir(&root)
            .env("PYTHONUTF8", "1")
            .env("PYTHONUNBUFFERED", "1")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn();
        let mut child = match started {
            Ok(child) => child,
            Err(error) => {
                say(format!("could not start python: {error}"));
                return false;
            }
        };
        if let Some(out) = child.stdout.take() {
            for line in std::io::BufReader::new(out).lines().map_while(|line| line.ok()) {
                say(line);
            }
        }
        match child.wait_with_output() {
            Ok(output) if output.status.success() => true,
            Ok(output) => {
                let errors = String::from_utf8_lossy(&output.stderr);
                let lines: Vec<&str> = errors.lines().collect();
                lines[lines.len().saturating_sub(6)..].iter().for_each(|line| say((*line).to_owned()));
                say(format!("{what} failed"));
                false
            }
            Err(error) => {
                say(format!("{what} failed: {error}"));
                false
            }
        }
    };
    if !run("Downloading from Shadertoy (a browser window opens)", &["engine/tools/fetch_shadertoy_user.py", "--ids-file", &list, "--name", "desktop"]) {
        return;
    }
    if !run("Converting for the engine", &["engine/tools/make_native_shadertoy.py", "--ids-file", &list, "--out", &folder]) {
        return;
    }
    // Only new files go into the library; a shader already there is left as it is.
    let mut added = 0;
    for id in &ids {
        let (from, to) = (incoming.join(format!("{id}.fs")), root.join(format!("target/native-13-shaders/{id}.fs")));
        if from.is_file() && !to.exists() && std::fs::copy(&from, &to).is_ok() {
            added += 1;
        }
    }
    say(format!("{added} new shader file(s) in the library"));
    if added > 0 {
        run("Rebuilding the catalog", &["engine/tools/shader_bank/catalog.py"]);
        run("Fetching input textures", &["engine/tools/shader_bank/fetch_assets.py"]);
    }
}
/// The size of the pictures shown when a library shader is hovered.
const PREVIEW: (u32, u32) = (320, 180);
/// A picture through a chain of effects; the texture that holds the result.
#[allow(clippy::too_many_arguments)]
fn run_effects(
    gfx: &gfx::Gfx,
    scratch: &[gfx::Target; 2],
    source: glow::Texture,
    effects: &[Effect],
    first: usize,
    time: f32,
    beat: f32,
    bass: f32,
) -> (glow::Texture, usize) {
    // `first` is the scratch target to write first: the one `source` is not in.
    let (mut picture, mut next) = (source, first);
    for effect in effects.iter().filter(|effect| !effect.bypass && effect.kind > 0) {
        gfx.effect(&scratch[next], picture, effect.kind as i32, effect.amount, time, beat, bass);
        picture = scratch[next].texture;
        next ^= 1;
    }
    (picture, next)
}
/// Keep what a layer shows now, for its transition to what it shows next.
fn begin_transition(gfx: &gfx::Gfx, layer: &mut Layer) {
    if layer.transition == 0 {
        return;
    }
    if layer.drawn {
        gfx.mix(&layer.from, &gfx::Mix::copy(layer.target.texture));
    } else {
        gfx.clear(&layer.from);
    }
    layer.switched = Instant::now();
}
fn effects_json(effects: &[Effect]) -> serde_json::Value {
    effects
        .iter()
        .map(|effect| serde_json::json!({ "name": gfx::EFFECTS[effect.kind].0, "amount": effect.amount, "bypass": effect.bypass }))
        .collect()
}
fn effects_from(saved: &serde_json::Value) -> Vec<Effect> {
    let known = |effect: &serde_json::Value| {
        let kind = gfx::EFFECTS.iter().position(|(name, _)| Some(*name) == effect["name"].as_str())?;
        Some(Effect {
            kind,
            amount: effect["amount"].as_f64().map_or(0.5, |amount| amount as f32).clamp(0.0, 1.0),
            bypass: effect["bypass"].as_bool().unwrap_or(false),
        })
    };
    saved.as_array().into_iter().flatten().filter_map(known).collect()
}
struct Cell {
    source: Source,
    /// A shader's compiled program and its still picture, made soon after it is placed.
    program: Option<gfx::Program>,
    thumb: Option<(gfx::Target, egui::TextureId)>,
    prepared: bool,
    error: Option<String>,
    /// How long it plays before a layer on Sequence or Shuffle moves on.
    seconds: f32,
    /// Its own react mode; `None` takes the layer's.
    react: Option<u8>,
    /// Effects on this cell alone, worked through before the layer's.
    effects: Vec<Effect>,
}
impl Cell {
    fn new(source: Source) -> Self {
        Self { source, program: None, thumb: None, prepared: false, error: None, seconds: 20.0, react: None, effects: Vec::new() }
    }
}
struct Layer {
    name: String,
    cells: Vec<Option<Cell>>,
    active: Option<usize>,
    opacity: f32,
    blend: usize,
    effects: Vec<Effect>,
    react: u8,
    bypass: bool,
    /// Index into `AUTO_MODES`.
    auto: usize,
    /// Move on at a new section of the song as well as when the time is up.
    on_sections: bool,
    /// When the live cell was launched, and the cells played since the last shuffle round.
    launched: Instant,
    played: Vec<usize>,
    /// The shader clock: it runs at the pace the shader's look sets.
    time: f32,
    /// The change from one cell to the next: which kind (`gfx::TRANSITIONS`), how
    /// long, when the last one began, and the picture the layer showed before it.
    transition: usize,
    transition_seconds: f32,
    switched: Instant,
    from: gfx::Target,
    reactor: Reactor,
    clock: BeatClock,
    flow: [f32; 4],
    spec_flow: [f32; 32],
    flash: [f32; 4],
    target: gfx::Target,
    /// Whether `target` holds this frame's picture.
    drawn: bool,
    player: Option<mpv::Player>,
    error: Option<String>,
}
struct Folder {
    name: String,
    ids: Vec<String>,
}
#[derive(Clone, Copy, PartialEq)]
enum View {
    All,
    PiBank,
    Folder(usize),
}
/// Something a MIDI knob or pad can be bound to.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Control {
    Gain,
    Lead,
    Opacity(usize),
    FxAmount(usize),
    Eject(usize),
    Cell(usize, usize),
    AutoOpacity,
    AutoToggle,
    AutoNext,
}
impl Control {
    /// How it is written in the saved state.
    fn name(self) -> String {
        match self {
            Control::Gain => "gain".into(),
            Control::Lead => "lead".into(),
            Control::Opacity(layer) => format!("opacity:{layer}"),
            Control::FxAmount(layer) => format!("fx:{layer}"),
            Control::Eject(layer) => format!("eject:{layer}"),
            Control::Cell(layer, column) => format!("cell:{layer}:{column}"),
            Control::AutoOpacity => "auto_opacity".into(),
            Control::AutoToggle => "auto_toggle".into(),
            Control::AutoNext => "auto_next".into(),
        }
    }
    fn parse(name: &str) -> Option<Self> {
        let mut parts = name.split(':');
        let (kind, mut number) = (parts.next()?, || parts.next()?.parse::<usize>().ok());
        Some(match kind {
            "gain" => Control::Gain,
            "lead" => Control::Lead,
            "opacity" => Control::Opacity(number()?),
            "fx" => Control::FxAmount(number()?),
            "eject" => Control::Eject(number()?),
            "cell" => Control::Cell(number()?, number()?),
            "auto_opacity" => Control::AutoOpacity,
            "auto_toggle" => Control::AutoToggle,
            "auto_next" => Control::AutoNext,
            _ => return None,
        })
    }
}
/// A MIDI message as a binding knows it: kind (0x90 note, 0xB0 controller), channel, number.
type MidiKey = (u8, u8, u8);
/// The Akai APC Key 25 as it comes: an 8 by 5 pad grid (notes 0..40 from the bottom left),
/// the arrow keys, "stop all clips", the scene buttons beside the rows, and eight knobs.
mod apc {
    pub const NAME: &str = "APC Key 25";
    pub const COLUMNS: usize = 8;
    pub const ROWS: usize = 5;
    pub const UP: u8 = 64;
    pub const DOWN: u8 = 65;
    pub const LEFT: u8 = 66;
    pub const RIGHT: u8 = 67;
    pub const STOP_ALL: u8 = 81;
    /// Scene buttons, top row first.
    pub const SCENE: u8 = 82;
    pub const KNOB: u8 = 48;
}
/// Pad velocities for a cell that is empty, filled and playing, and for a button that
/// is on and off. Each controller has its own colour numbers.
fn led_defaults(midi: &midi::Midi) -> [u8; 5] {
    if midi.has(apc::NAME) {
        // Off, yellow, green.
        [0, 5, 1, 1, 0]
    } else if midi.has("Launchpad") {
        // Off, dim amber, green on the first Launchpads.
        [12, 29, 60, 60, 12]
    } else if midi.has("X-TOUCH") {
        [0, 0, 1, 1, 0]
    } else {
        [0, 1, 127, 127, 0]
    }
}
/// What the library plays by itself, kept under a name.
struct Preset {
    name: String,
    ids: Vec<String>,
    shuffle: bool,
    seconds: f32,
    on_sections: bool,
    react: u8,
    opacity: f32,
    blend: usize,
}
/// In MIDI learn, note where a control sits and what was done to it.
fn spot(actions: &mut Vec<Action>, learn: bool, response: &egui::Response, control: Control) {
    if learn {
        let picked = response.clicked() || response.drag_started();
        actions.push(Action::Spot(response.rect, control, picked, response.secondary_clicked()));
    }
}
/// What the UI asks for while it is being drawn; carried out once drawing is done.
enum Action {
    Launch(usize, usize),
    Eject(usize),
    Place(usize, usize, Vec<Source>),
    ClearCell(usize, usize),
    ClearLayer(usize),
    AddColumn,
    AddLayer,
    RemoveLayer(usize),
    Send(usize),
    SetSeconds(usize, usize, f32),
    SetReact(usize, usize, Option<u8>),
    AddToFolder(usize, Vec<usize>),
    RemoveFromFolder(usize, Vec<usize>),
    DeleteFolder(usize),
    /// MIDI learn: a control's place, whether it was picked, whether to forget its binding.
    Spot(egui::Rect, Control, bool, bool),
    /// Composition cells: add to or take from the selection (Ctrl) or select a run of
    /// one layer (Shift); move cells to start at a layer and column; clear cells.
    PickCell(usize, usize, bool),
    MoveCells(Vec<(usize, usize)>, usize, usize),
    ClearCells(Vec<(usize, usize)>),
    /// Effect chains: of a layer, or of the whole composition for `None`.
    AddEffect(ChainAt, usize),
    RemoveEffect(ChainAt, usize),
    MoveEffect(ChainAt, usize, bool),
    /// An effect double-clicked in the Effects tab: onto the highlighted cells, or onto
    /// the layer when the layer was highlighted last.
    AddEffectToFocus(usize),
    /// Assets: put a file on the selected layer and launch it; fill and empty collections.
    SendClip(PathBuf),
    /// Open the shader editor for a shader that is on a layer.
    Edit(usize),
    Undo,
    Redo,
    AddToCollection(usize, Vec<PathBuf>),
    RemoveFromCollection(usize, Vec<PathBuf>),
    DeleteCollection(usize),
    /// Library: run one in the preview, mark favourites, remove from or restore to the library.
    Preview(usize),
    Favorite(Vec<usize>, bool),
    Remove(Vec<usize>, bool),
    Reviewed(Vec<usize>),
    LoadPreset(usize),
    DeletePreset(usize),
    OpenComposition(serde_json::Value),
}
/// The checked shaders of the library being played one after another.
struct AutoPlay {
    order: Vec<usize>,
    at: usize,
    /// The next shader, compiled ahead so the change does not stall a frame.
    ready: Option<(usize, gfx::Program)>,
}
struct CompileRun {
    next: usize,
    passed: usize,
    failures: Vec<(String, String)>,
    started: Instant,
}
struct App {
    options: Options,
    root: PathBuf,
    gfx: gfx::Gfx,
    // Library.
    library: Vec<library::Entry>,
    index_of: HashMap<String, usize>,
    channels: HashMap<String, Vec<Channel>>,
    bank: Vec<usize>,
    folders: Vec<Folder>,
    view: View,
    filter: String,
    show_unrunnable: bool,
    new_folder: String,
    selection: BTreeSet<usize>,
    anchor: Option<usize>,
    // Library autoplay: the checked shaders, on a layer of their own above the composition.
    checked: BTreeSet<usize>,
    autoplay: Option<AutoPlay>,
    auto: Layer,
    auto_shuffle: bool,
    auto_seconds: f32,
    auto_sections: bool,
    auto_changes: u32,
    seed: u64,
    presets: Vec<Preset>,
    preset_name: String,
    /// Which tab of the left panel is open: 0 shaders, 1 effects.
    tab: usize,
    fx_filter: String,
    /// Groups worked out from the catalog, and the one chosen: 0 all, 1 favourites,
    /// 2 removed, then `groups` in order.
    groups: Vec<(String, BTreeSet<usize>)>,
    group: usize,
    favorites: BTreeSet<usize>,
    removed: BTreeSet<usize>,
    /// Hover pictures: loaded ones, the ones that cannot be made, the one asked for, and
    /// where a run over the whole library has got to.
    thumbs: HashMap<usize, egui::TextureHandle>,
    thumb_failed: BTreeSet<usize>,
    thumb_wanted: Option<usize>,
    thumb_run: Option<usize>,
    thumb_target: gfx::Target,
    /// A library shader running in the preview only, not in the output.
    pre: Layer,
    pre_texture: Option<egui::TextureId>,
    detached: bool,
    /// Effects on the whole composition, and two targets effects are worked through.
    master: Vec<Effect>,
    scratch: [gfx::Target; 2],
    /// Each shader's look, by library index; the picture a look's colours are worked
    /// into; and the preview's own finished picture.
    looks: HashMap<usize, Look>,
    graded: gfx::Target,
    pre_out: gfx::Target,
    /// The editor: the numbers the previewed shader declares, its code while that is
    /// open (shader, text, compile error), and when a changed number asks for a rebuild.
    declared: Option<(usize, Vec<(String, f32, bool)>)>,
    code: Option<(usize, String, Option<String>)>,
    rebuild_at: Option<Instant>,
    /// For the scripted pointer: where the first list row and the first cell of the
    /// second layer were drawn, and what the drag reported.
    seen_row: std::cell::Cell<Option<egui::Rect>>,
    seen_cell: std::cell::Cell<Option<egui::Rect>>,
    drag_notes: std::cell::RefCell<Vec<String>>,
    /// How far the grid's cells are scrolled sideways.
    grid_x: f32,
    /// Where the editor is folded away; whether an effect goes to the layer (it was
    /// highlighted last) or to the highlighted cells; a target for transitions.
    editor_open: bool,
    /// The shader whose editor a double-click on a cell opened.
    editing: Option<usize>,
    focus_layer: bool,
    crossing: gfx::Target,
    /// Assets: the folder being looked at and what is in it (path, is a folder), the
    /// files picked there, and the collections.
    assets_dir: PathBuf,
    /// What each folder read so far holds (path, is a folder); the folders opened in
    /// the tree; list or thumbnails; and the thumbnails made, asked for (`None`), and
    /// the two ends of the line to the thread that makes them.
    assets_cache: HashMap<PathBuf, Vec<(PathBuf, bool)>>,
    assets_open: BTreeSet<PathBuf>,
    assets_tiles: bool,
    asset_thumbs: HashMap<PathBuf, Option<egui::TextureHandle>>,
    thumb_ask: std::sync::mpsc::Sender<PathBuf>,
    thumb_got: std::sync::mpsc::Receiver<(PathBuf, Option<(Vec<u8>, usize, usize)>)>,
    assets_stale: bool,
    assets_picked: BTreeSet<PathBuf>,
    assets_anchor: Option<usize>,
    collections: Vec<Collection>,
    collection_view: Option<usize>,
    new_collection: String,
    /// Downloading from Shadertoy, and the shaders that came in and are not reviewed yet.
    download: Option<Download>,
    download_text: String,
    download_open: bool,
    review: BTreeSet<usize>,
    // MIDI.
    midi: midi::Midi,
    bindings: Vec<(MidiKey, Control)>,
    learn: bool,
    learn_target: Option<Control>,
    midi_last: Option<String>,
    /// What each light was last told, by (status, number); a light is sent only on change.
    lights: HashMap<(u8, u8), u8>,
    /// Velocities for empty, filled, playing, on, off.
    led: [u8; 5],
    /// The APC Key 25 layout is in use, and where its pad grid sits: first column, first layer.
    apc: bool,
    pads: (usize, usize),
    // Composition.
    layers: Vec<Layer>,
    columns: usize,
    selected_layer: usize,
    /// Cells picked with Ctrl- or Shift-click, as (layer, column), and where a Shift run starts.
    picked: BTreeSet<(usize, usize)>,
    pick_anchor: Option<(usize, usize)>,
    size: usize,
    stack: [gfx::Target; 2],
    out_target: gfx::Target,
    out_texture: Option<egui::TextureId>,
    // Audio.
    sources: Vec<audio_in::Source>,
    source: Option<usize>,
    capture: Option<audio_in::Capture>,
    shared: Arc<audio_in::Shared>,
    audio_error: Option<String>,
    gain: f32,
    level: f32,
    held: (f32, Instant),
    clipped: Option<Instant>,
    heard: AudioFrame,
    predictor: BeatPredictor,
    sections: SectionDetector,
    section_count: u32,
    lead_ms: f32,
    idle: f32,
    idle_since: Instant,
    // Output.
    displays: Vec<Display>,
    output_on: bool,
    output_display: usize,
    /// Where the projection window ended up, in points; for the test printout.
    output_rect: std::cell::Cell<Option<egui::Rect>>,
    /// Where the system says the projection window is, in screen pixels.
    output_px: std::cell::Cell<Option<[i32; 4]>>,
    /// Undo: states to go back to, states to go forward to, the state as last looked
    /// at, and when that was.
    undo: Vec<serde_json::Value>,
    redo: Vec<serde_json::Value>,
    undo_base: serde_json::Value,
    undo_looked: Instant,
    /// Clips on layers whose thumbnail has not been asked for yet.
    clip_thumbs_wanted: std::cell::RefCell<Vec<PathBuf>>,
    // Housekeeping.
    message: Option<String>,
    state_path: Option<PathBuf>,
    saved_at: Instant,
    start: Instant,
    last: Instant,
    frames: u64,
    fps: f32,
    compile: Option<CompileRun>,
}
fn new_layer(gfx: &gfx::Gfx, name: String, columns: usize, size: usize) -> Result<Layer> {
    Ok(Layer {
        name,
        cells: (0..columns).map(|_| None).collect(),
        active: None,
        opacity: 1.0,
        blend: 0,
        effects: Vec::new(),
        react: 0,
        bypass: false,
        auto: 0,
        on_sections: false,
        launched: Instant::now(),
        played: Vec::new(),
        time: 0.0,
        transition: 1,
        transition_seconds: 0.5,
        switched: Instant::now(),
        from: gfx.target(RENDER_SIZES[size])?,
        reactor: Reactor::default(),
        clock: BeatClock::default(),
        flow: [0.0; 4],
        spec_flow: [0.0; 32],
        flash: [1.0, 0.0, 0.0, 0.0],
        target: gfx.target(RENDER_SIZES[size])?,
        drawn: false,
        player: None,
        error: None,
    })
}
/// A stand-in for music, for a shader's still picture.
fn still_audio() -> AudioFrame {
    AudioFrame {
        bass: 0.5,
        sub: 0.4,
        mid: 0.4,
        low_mid: 0.4,
        high_mid: 0.3,
        treble: 0.3,
        volume: 0.5,
        spectrum: [0.3; rpi_vj_native::audio::BINS],
        ..AudioFrame::default()
    }
}
impl App {
    fn new(cc: &eframe::CreationContext<'_>, options: Options) -> Result<Self> {
        cc.egui_ctx.set_theme(egui::Theme::Dark);
        // Scroll bars that are there whenever there is more to see, not only under the pointer.
        cc.egui_ctx.all_styles_mut(|style| style.spacing.scroll = egui::style::ScrollStyle::solid());
        let gl = cc.gl.clone().ok_or_else(|| anyhow::anyhow!("no OpenGL context"))?;
        let gfx = gfx::Gfx::new(gl)?;
        let root = options.root.clone().unwrap_or_else(library::root);
        let library = library::load(&root)?;
        let index_of: HashMap<String, usize> =
            library.iter().enumerate().map(|(index, entry)| (entry.id.clone(), index)).collect();
        let bank = library::pi_bank(&root).iter().filter_map(|id| index_of.get(id).copied()).collect();
        let size = 1;
        let displays = displays();
        let kept = std::env::var_os("APPDATA").map(|folder| PathBuf::from(folder).join("RPi-VJ/desktop.json"));
        let state_path = options.state.clone().or(kept.filter(|_| !options.testing()));
        let now = Instant::now();
        // Asset thumbnails are made on a thread of their own, one at a time.
        let (thumb_ask, asked) = std::sync::mpsc::channel::<PathBuf>();
        let (made, thumb_got) = std::sync::mpsc::channel();
        let cache = root.join("target/desktop/asset-thumbs");
        std::thread::spawn(move || {
            while let Ok(path) = asked.recv() {
                let thumb = asset_thumb(&cache, &path);
                if made.send((path, thumb)).is_err() {
                    break;
                }
            }
        });
        let mut auto = new_layer(&gfx, "Library".into(), 1, size)?;
        auto.blend = 2;
        let mut app = Self {
            checked: BTreeSet::new(),
            autoplay: None,
            auto,
            auto_shuffle: false,
            auto_seconds: 20.0,
            auto_sections: true,
            auto_changes: 0,
            presets: Vec::new(),
            preset_name: String::new(),
            tab: 0,
            fx_filter: String::new(),
            groups: library::groups(&library),
            group: 0,
            favorites: BTreeSet::new(),
            removed: BTreeSet::new(),
            thumbs: HashMap::new(),
            thumb_failed: BTreeSet::new(),
            thumb_wanted: None,
            thumb_run: None,
            thumb_target: gfx.target(PREVIEW)?,
            pre: new_layer(&gfx, "Preview".into(), 1, size)?,
            pre_texture: None,
            detached: false,
            master: Vec::new(),
            scratch: [gfx.target(RENDER_SIZES[size])?, gfx.target(RENDER_SIZES[size])?],
            looks: HashMap::new(),
            seen_row: std::cell::Cell::new(None),
            seen_cell: std::cell::Cell::new(None),
            drag_notes: std::cell::RefCell::new(Vec::new()),
            grid_x: 0.0,
            editor_open: true,
            editing: None,
            focus_layer: true,
            crossing: gfx.target(RENDER_SIZES[size])?,
            assets_dir: std::env::var_os("USERPROFILE").map(PathBuf::from).map(|home| if home.join("Videos").is_dir() { home.join("Videos") } else { home }).unwrap_or_else(|| PathBuf::from("/")),
            assets_cache: HashMap::new(),
            assets_open: BTreeSet::new(),
            assets_tiles: false,
            asset_thumbs: HashMap::new(),
            thumb_ask,
            thumb_got,
            assets_stale: true,
            assets_picked: BTreeSet::new(),
            assets_anchor: None,
            collections: Vec::new(),
            collection_view: None,
            new_collection: String::new(),
            graded: gfx.target(RENDER_SIZES[size])?,
            pre_out: gfx.target(RENDER_SIZES[size])?,
            declared: None,
            code: None,
            rebuild_at: None,
            download: None,
            download_text: String::new(),
            download_open: false,
            review: BTreeSet::new(),
            lights: HashMap::new(),
            led: [0, 1, 127, 127, 0],
            apc: false,
            pads: (0, 0),
            midi: midi::open(),
            bindings: Vec::new(),
            learn: false,
            learn_target: None,
            midi_last: None,
            seed: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |t| t.as_nanos() as u64) | 1,
            stack: [gfx.target(RENDER_SIZES[size])?, gfx.target(RENDER_SIZES[size])?],
            out_target: gfx.target(RENDER_SIZES[size])?,
            out_texture: None,
            channels: library::channels(&root),
            root,
            gfx,
            library,
            index_of,
            bank,
            folders: Vec::new(),
            view: View::All,
            filter: String::new(),
            show_unrunnable: false,
            new_folder: String::new(),
            selection: BTreeSet::new(),
            anchor: None,
            layers: Vec::new(),
            columns: 8,
            selected_layer: 1,
            picked: BTreeSet::new(),
            pick_anchor: None,
            size,
            sources: audio_in::sources(),
            source: None,
            capture: None,
            shared: Arc::default(),
            audio_error: None,
            gain: options.gain.unwrap_or(1.0),
            level: 0.0,
            held: (0.0, now),
            clipped: None,
            heard: AudioFrame::default(),
            predictor: BeatPredictor::default(),
            sections: SectionDetector::default(),
            section_count: 0,
            lead_ms: BEAT_LEAD * 1000.0,
            idle: 0.0,
            idle_since: now,
            output_on: false,
            output_display: displays.len().saturating_sub(1),
            output_rect: std::cell::Cell::new(None),
            output_px: std::cell::Cell::new(None),
            undo: Vec::new(),
            redo: Vec::new(),
            undo_base: serde_json::Value::Null,
            undo_looked: now,
            clip_thumbs_wanted: std::cell::RefCell::new(Vec::new()),
            displays,
            message: None,
            state_path,
            saved_at: now,
            start: now,
            last: now,
            frames: 0,
            fps: 0.0,
            compile: None,
            options,
        };
        app.led = led_defaults(&app.midi);
        app.apc = app.midi.has(apc::NAME);
        let mut wanted_source = None;
        if !app.load_state(&mut wanted_source) {
            for name in ["Layer 1", "Layer 2", "Layer 3"] {
                let layer = app.make_layer(name.into())?;
                app.layers.push(layer);
            }
        }
        // Listen to the device used last time, else to what the computer is playing.
        let source = wanted_source
            .and_then(|label: String| app.sources.iter().position(|source| source.label() == label))
            .or_else(|| app.sources.iter().position(|source| source.loopback))
            .or((!app.sources.is_empty()).then_some(0));
        if let Some(source) = source {
            app.listen(source);
        }
        if let Some(id) = app.options.live.clone() {
            match app.index_of.get(&id).copied() {
                Some(entry) => {
                    app.place(1, 0, vec![Source::Shader(entry)]);
                    app.launch(1, 0);
                }
                None => eprintln!("no shader {id} in the library"),
            }
        }
        if app.options.fill > 0 {
            let sources = app.bank.iter().take(app.options.fill).map(|index| Source::Shader(*index)).collect();
            app.place(1, 0, sources);
            app.launch(1, 0);
        }
        if app.options.fx > 0 {
            // `--fx 7,8` chains effects 7 and 8.
            let kind = app.options.fx.min(gfx::EFFECTS.len() - 1);
            app.layers[1].effects.push(Effect { kind, amount: 0.7, bypass: false });
        }
        if let Some(clip) = app.options.clip.clone() {
            app.place(0, 0, vec![Source::Clip(clip)]);
            app.launch(0, 0);
        }
        if app.options.sequence {
            app.layers[1].auto = 1;
            for cell in app.layers[1].cells.iter_mut().flatten() {
                cell.seconds = 1.0;
            }
        }
        if app.options.autoplay > 0 {
            app.checked = app.bank.iter().take(app.options.autoplay).copied().collect();
            app.auto_seconds = 1.0;
            app.start_autoplay();
        }
        if let Some(text) = app.options.open.as_ref().and_then(|path| std::fs::read_to_string(path).ok()) {
            if let Ok(saved) = serde_json::from_str(&text) {
                app.apply(Action::OpenComposition(saved));
            }
        }
        app.tab = app.options.tab.min(2);
        app.assets_tiles = app.options.tiles;
        app.grid_x = app.options.scroll;
        if let Some(wanted) = app.options.download.clone() {
            app.download_text = wanted;
            app.start_download();
        }
        app.detached |= app.options.detach;
        if app.options.thumbs {
            app.thumb_run = Some(0);
        }
        if let Some(entry) = app.options.preview.as_ref().and_then(|id| app.index_of.get(id).copied()) {
            if app.options.look_test {
                let first = app.source_text(entry).ok().and_then(|text| text.lines().find_map(|line| param_of(line).map(|found| found.0.to_owned())));
                let look = Look {
                    zoom: 2.0,
                    rotate: 30.0,
                    speed: 0.5,
                    hue: 0.33,
                    contrast: 1.4,
                    params: first.into_iter().map(|name| (name, 2.0)).collect(),
                    ..Look::default()
                };
                app.looks.insert(entry, look);
            }
            app.apply(Action::Preview(entry));
            app.favorites.insert(entry);
        }
        if app.options.edit_test {
            // What a double-click on the first cell of the second layer asks for.
            if let Some(Source::Shader(entry)) = app.layers[1].cells[0].as_ref().map(|cell| cell.source.clone()) {
                app.apply(Action::Launch(1, 0));
                app.apply(Action::Edit(entry));
            }
        }
        if app.options.cell_fx {
            // Click a cell, then double-click an effect: it lands on that cell, not the layer.
            app.apply(Action::Launch(1, 0));
            app.apply(Action::AddEffectToFocus(7));
        }
        if app.options.move_test {
            // Pick the first and third cells of the second layer and move them to the top layer.
            app.apply(Action::PickCell(1, 0, false));
            app.apply(Action::PickCell(1, 2, false));
            let cells = app.picked.iter().copied().collect();
            app.apply(Action::MoveCells(cells, 2, 1));
        }
        if app.options.midi_test {
            // A knob on the second layer's opacity, a pad on its third cell, and the
            // checked list out through a preset and back.
            app.bindings = vec![((0xB0, 0, 1), Control::Opacity(1)), ((0x90, 0, 5), Control::Cell(1, 2))];
            for message in [[0xB0, 1, 32], [0x90, 5, 100], [0x80, 5, 0]] {
                let _ = app.midi.inject.send(message);
            }
            app.preset_name = "test".into();
            app.save_preset();
            app.stop_autoplay();
            app.checked.clear();
            app.apply(Action::LoadPreset(0));
        }
        if let Some(display) = app.options.output {
            app.output_on = true;
            app.output_display = display.saturating_sub(1).min(app.displays.len().saturating_sub(1));
        }
        if app.options.compile_all {
            app.compile = Some(CompileRun { next: 0, passed: 0, failures: Vec::new(), started: now });
        }
        app.undo_base = app.state();
        Ok(app)
    }
    fn make_layer(&self, name: String) -> Result<Layer> {
        new_layer(&self.gfx, name, self.columns, self.size)
    }
    /// Keep the checked list and how it plays under the name in the preset box.
    fn save_preset(&mut self) {
        let name = self.preset_name.trim().to_owned();
        if name.is_empty() {
            return;
        }
        let preset = Preset {
            name: name.clone(),
            ids: self.checked.iter().map(|entry| self.library[*entry].id.clone()).collect(),
            shuffle: self.auto_shuffle,
            seconds: self.auto_seconds,
            on_sections: self.auto_sections,
            react: self.auto.react,
            opacity: self.auto.opacity,
            blend: self.auto.blend,
        };
        match self.presets.iter_mut().find(|known| known.name == name) {
            Some(known) => *known = preset,
            None => self.presets.push(preset),
        }
    }
    /// Take what MIDI has sent: bind it in learn mode, else move what it is bound to.
    fn handle_midi(&mut self) {
        while let Ok([status, number, value]) = self.midi.incoming.try_recv() {
            let (kind, channel) = (status & 0xF0, status & 0x0F);
            let note_on = kind == 0x90 && value > 0;
            if kind != 0xB0 && !note_on {
                continue;
            }
            let key: MidiKey = (kind, channel, number);
            self.midi_last = Some(format!(
                "{} {number} ch {} = {value}",
                if kind == 0xB0 { "knob" } else { "pad" },
                channel + 1
            ));
            if self.learn {
                if let Some(target) = self.learn_target.take() {
                    self.bindings.retain(|(bound, control)| *bound != key && *control != target);
                    self.bindings.push((key, target));
                    self.lights.clear();
                }
                continue;
            }
            if self.apc && self.apc_in(kind, number, value) {
                continue;
            }
            let bound: Vec<Control> = self.bindings.iter().filter(|(k, _)| *k == key).map(|(_, control)| *control).collect();
            for control in bound {
                self.control(control, f32::from(value) / 127.0, note_on || value >= 64);
            }
        }
    }
    /// The APC Key 25 layout: pads launch the cells under them, the arrows move the pad
    /// grid over the composition, a scene button ejects its layer, "stop all clips"
    /// ejects every layer, and the knobs are five layer opacities, the autoplay
    /// opacity, the beat lead and the FFT gain. True when the message was one of these.
    fn apc_in(&mut self, kind: u8, number: u8, value: u8) -> bool {
        let (columns, rows) = (apc::COLUMNS, apc::ROWS);
        if kind == 0xB0 {
            let Some(knob) = number.checked_sub(apc::KNOB).map(usize::from).filter(|knob| *knob < 8) else { return false };
            let control = match knob {
                5 => Control::AutoOpacity,
                6 => Control::Lead,
                7 => Control::Gain,
                row => Control::Opacity(row + self.pads.1),
            };
            self.control(control, f32::from(value) / 127.0, false);
            return true;
        }
        match number {
            pad if usize::from(pad) < columns * rows => {
                let (layer, column) = (usize::from(pad) / columns + self.pads.1, usize::from(pad) % columns + self.pads.0);
                if self.layers.get(layer).is_some_and(|layer| layer.cells.get(column).is_some_and(Option::is_some)) {
                    self.launch(layer, column);
                }
            }
            apc::LEFT => self.pads.0 = self.pads.0.saturating_sub(columns),
            apc::RIGHT if self.pads.0 + columns < self.columns => self.pads.0 += columns,
            apc::DOWN => self.pads.1 = self.pads.1.saturating_sub(1),
            apc::UP if self.pads.1 + rows < self.layers.len() => self.pads.1 += 1,
            apc::STOP_ALL => (0..self.layers.len()).for_each(|layer| self.eject(layer)),
            scene if (apc::SCENE..apc::SCENE + rows as u8).contains(&scene) => {
                let layer = rows - 1 - usize::from(scene - apc::SCENE) + self.pads.1;
                if layer < self.layers.len() {
                    self.eject(layer);
                }
            }
            apc::RIGHT | apc::UP => {}
            _ => return false,
        }
        true
    }
    /// Where a knob-bound control stands, 0..=1; `None` for the ones pads press.
    fn control_value(&self, control: Control) -> Option<f32> {
        Some(match control {
            Control::Gain => (self.gain / 0.05).ln() / (16.0f32 / 0.05).ln(),
            Control::Lead => self.lead_ms / 250.0,
            Control::Opacity(layer) => self.layers.get(layer)?.opacity,
            Control::FxAmount(layer) => self.layers.get(layer)?.effects.first()?.amount,
            Control::AutoOpacity => self.auto.opacity,
            _ => return None,
        })
    }
    /// The light for a cell: empty, filled or playing.
    fn cell_light(&self, layer: usize, column: usize) -> u8 {
        match self.layers.get(layer) {
            Some(row) if row.cells.get(column).is_some_and(Option::is_some) => {
                if row.active == Some(column) { self.led[2] } else { self.led[1] }
            }
            _ => self.led[0],
        }
    }
    /// Tell the controllers what to light: every bound pad shows the state of what it
    /// is bound to, every bound knob gets its value back (for ringed encoders), and the
    /// APC layout shows the cells under its pads. Only what changed is sent.
    fn light(&mut self) {
        let on = |lit: bool| if lit { self.led[3] } else { self.led[4] };
        let mut wanted: Vec<((u8, u8), u8)> = Vec::new();
        for ((kind, channel, number), control) in &self.bindings {
            let value = match (*control, self.control_value(*control)) {
                (_, Some(value)) => (value.clamp(0.0, 1.0) * 127.0).round() as u8,
                (Control::Cell(layer, column), _) => self.cell_light(layer, column),
                (Control::Eject(layer), _) => on(self.layers.get(layer).is_some_and(|layer| layer.active.is_some())),
                (Control::AutoToggle | Control::AutoNext, _) => on(self.autoplay.is_some()),
                _ => self.led[4],
            };
            wanted.push(((kind | channel, *number), value));
        }
        if self.apc {
            let (columns, rows) = (apc::COLUMNS, apc::ROWS);
            for pad in 0..columns * rows {
                wanted.push(((0x90, pad as u8), self.cell_light(pad / columns + self.pads.1, pad % columns + self.pads.0)));
            }
            for row in 0..rows {
                let live = self.layers.get(row + self.pads.1).is_some_and(|layer| layer.active.is_some());
                wanted.push(((0x90, apc::SCENE + (rows - 1 - row) as u8), u8::from(live)));
            }
            // An arrow is lit while there is more of the composition that way.
            for (arrow, more) in [
                (apc::LEFT, self.pads.0 > 0),
                (apc::RIGHT, self.pads.0 + columns < self.columns),
                (apc::DOWN, self.pads.1 > 0),
                (apc::UP, self.pads.1 + rows < self.layers.len()),
            ] {
                wanted.push(((0x90, arrow), u8::from(more)));
            }
        }
        for ((status, number), value) in wanted {
            if self.lights.insert((status, number), value) != Some(value) {
                self.midi.send([status, number, value]);
            }
        }
    }
    /// Move a bound control: `value` (0..=1) for knobs, `pressed` for pads.
    fn control(&mut self, control: Control, value: f32, pressed: bool) {
        match control {
            Control::Gain => self.gain = 0.05 * (16.0f32 / 0.05).powf(value),
            Control::Lead => self.lead_ms = value * 250.0,
            Control::Opacity(layer) => {
                if let Some(layer) = self.layers.get_mut(layer) {
                    layer.opacity = value;
                }
            }
            Control::FxAmount(layer) => {
                if let Some(first) = self.layers.get_mut(layer).and_then(|layer| layer.effects.first_mut()) {
                    first.amount = value;
                }
            }
            Control::AutoOpacity => self.auto.opacity = value,
            Control::Eject(layer) if pressed && layer < self.layers.len() => self.eject(layer),
            Control::Cell(layer, column) if pressed && layer < self.layers.len() && column < self.columns => {
                self.launch(layer, column)
            }
            Control::AutoToggle if pressed => {
                if self.autoplay.is_some() {
                    self.stop_autoplay();
                } else {
                    self.start_autoplay();
                }
            }
            Control::AutoNext if pressed => self.autoplay_next(),
            _ => {}
        }
    }
    /// The effect chain of a layer, or of the composition for `None`.
    fn chain(&mut self, target: ChainAt) -> Option<&mut Vec<Effect>> {
        match target {
            ChainAt::Cell(layer, column) => self.layers.get_mut(layer)?.cells.get_mut(column)?.as_mut().map(|cell| &mut cell.effects),
            ChainAt::Layer(layer) => self.layers.get_mut(layer).map(|layer| &mut layer.effects),
            ChainAt::Composition => Some(&mut self.master),
        }
    }
    /// The cells an effect goes to when it is double-clicked: the picked ones, else the
    /// one clicked last; none when the layer was highlighted last.
    fn focus_cells(&self) -> Vec<(usize, usize)> {
        let filled = |(layer, column): &(usize, usize)| {
            self.layers.get(*layer).is_some_and(|row| row.cells.get(*column).is_some_and(Option::is_some))
        };
        if self.focus_layer {
            Vec::new()
        } else if !self.picked.is_empty() {
            self.picked.iter().copied().filter(filled).collect()
        } else {
            self.pick_anchor.into_iter().filter(filled).collect()
        }
    }
    /// What a folder holds, folders first, then media files; read once and remembered.
    /// The empty path stands for the computer: its drives.
    fn folder(&mut self, folder: &PathBuf) -> Vec<(PathBuf, bool)> {
        if let Some(known) = self.assets_cache.get(folder) {
            return known.clone();
        }
        let mut found: Vec<(PathBuf, bool)> = if folder.as_os_str().is_empty() {
            (b'A'..=b'Z').map(|letter| PathBuf::from(format!("{}:/", letter as char))).filter(|drive| drive.is_dir()).map(|drive| (drive, true)).collect()
        } else {
            std::fs::read_dir(folder)
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|entry| {
                    let path = entry.path();
                    let name = path.file_name()?.to_string_lossy().into_owned();
                    let is_folder = path.is_dir();
                    let hidden = name.starts_with('.') || name.starts_with('$');
                    (!hidden && (is_folder || is_media(&path))).then_some((path, is_folder))
                })
                .collect()
        };
        found.sort_by_key(|(path, is_folder)| (!*is_folder, path.file_name().map(|name| name.to_string_lossy().to_lowercase())));
        self.assets_cache.insert(folder.clone(), found.clone());
        found
    }
    /// The rows of the tree under `folder`: its entries, and under each opened folder its own.
    fn tree(&mut self, folder: &PathBuf, depth: usize, rows: &mut Vec<(PathBuf, bool, usize)>) {
        for (path, is_folder) in self.folder(folder) {
            rows.push((path.clone(), is_folder, depth));
            if is_folder && depth < 12 && self.assets_open.contains(&path) {
                self.tree(&path, depth + 1, rows);
            }
        }
    }
    /// Take in the thumbnails the thread has finished.
    fn take_thumbs(&mut self, ctx: &egui::Context) {
        while let Ok((path, thumb)) = self.thumb_got.try_recv() {
            let texture = thumb.map(|(rgb, width, height)| {
                let image = egui::ColorImage::from_rgb([width, height], &rgb);
                ctx.load_texture(format!("asset-{}", path.display()), image, egui::TextureOptions::LINEAR)
            });
            self.asset_thumbs.insert(path, texture);
        }
    }
    /// The Assets tab: collections on top, then either a collection's files or the
    /// folder being browsed, as a tree or as thumbnails. Files are picked like shaders
    /// and dragged to the grid.
    fn assets_panel(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        if self.assets_stale {
            self.assets_stale = false;
            self.assets_cache.clear();
            self.assets_picked.clear();
            self.assets_anchor = None;
        }
        self.take_thumbs(ui.ctx());
        let name_of = |path: &PathBuf| match path.file_name() {
            Some(name) => name.to_string_lossy().into_owned(),
            None => path.display().to_string(),
        };
        // Collections: click to open, drag onto the grid to place everything in one, drop files on one to add them.
        ui.strong("Collections");
        for (index, collection) in self.collections.iter().enumerate() {
            let label = format!("{} ({})", collection.name, collection.items.len());
            let response = ui.selectable_label(self.collection_view == Some(index), label).interact(egui::Sense::click_and_drag());
            if response.clicked() {
                self.collection_view = if self.collection_view == Some(index) { None } else { Some(index) };
                self.assets_picked.clear();
            }
            if response.dragged() {
                response.dnd_set_drag_payload(Dragged(collection.items.iter().cloned().map(Source::Clip).collect()));
            }
            if let Some(dropped) = dropped::<Dragged>(&response) {
                let files = dropped.0.iter().filter_map(|source| match source {
                    Source::Clip(path) => Some(path.clone()),
                    Source::Shader(_) => None,
                });
                actions.push(Action::AddToCollection(index, files.collect()));
            }
            if response.dnd_hover_payload::<Dragged>().is_some() {
                ui.painter().rect_stroke(response.rect, 3.0, (2.0, GREEN), egui::StrokeKind::Inside);
            }
            response.context_menu(|ui| {
                if ui.button("Delete collection").clicked() {
                    actions.push(Action::DeleteCollection(index));
                    ui.close_menu();
                }
            });
        }
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.new_collection).hint_text("new collection name").desired_width(170.0));
            if ui.button("Add collection").clicked() && !self.new_collection.trim().is_empty() {
                self.collections.push(Collection { name: self.new_collection.trim().to_owned(), items: Vec::new() });
                self.new_collection.clear();
            }
        });
        ui.separator();
        // What is shown below: a collection's files, or the folder with what is opened under it.
        let viewing = self.collection_view.filter(|index| *index < self.collections.len());
        let mut rows: Vec<(PathBuf, bool, usize)> = Vec::new();
        match viewing {
            Some(index) => rows.extend(self.collections[index].items.iter().map(|path| (path.clone(), false, 0))),
            None => {
                let top = self.assets_dir.clone();
                self.tree(&top, 0, &mut rows);
            }
        }
        match viewing {
            Some(index) => {
                ui.horizontal(|ui| {
                    ui.strong(&self.collections[index].name);
                    if ui.button("Back to folders").clicked() {
                        self.collection_view = None;
                    }
                    ui.checkbox(&mut self.assets_tiles, "thumbnails");
                });
            }
            None => {
                ui.horizontal(|ui| {
                    if ui.button("Up").on_hover_text("The folder above; above a drive, the list of drives").clicked() {
                        self.assets_dir = self.assets_dir.parent().map(PathBuf::from).unwrap_or_default();
                        self.assets_picked.clear();
                    }
                    if ui.button("Drives").clicked() {
                        self.assets_dir = PathBuf::new();
                        self.assets_picked.clear();
                    }
                    if ui.button("Choose…").clicked() {
                        if let Some(folder) = rfd::FileDialog::new().set_directory(&self.assets_dir).pick_folder() {
                            self.assets_dir = folder;
                            self.assets_picked.clear();
                        }
                    }
                    if ui.button("Refresh").clicked() {
                        self.assets_stale = true;
                    }
                    ui.checkbox(&mut self.assets_tiles, "thumbnails");
                });
                let place = if self.assets_dir.as_os_str().is_empty() { "This computer".to_owned() } else { self.assets_dir.display().to_string() };
                ui.add(egui::Label::new(egui::RichText::new(place).weak()).truncate());
            }
        }
        ui.weak(format!("{} picked. Drag to a cell, a layer or a collection; double-click a file to launch it, a folder to go into it.", self.assets_picked.len()));
        let modifiers = ui.input(|input| input.modifiers);
        let picked = self.assets_picked.clone();
        let (mut asked, mut enter, mut toggle, mut wanted) = (None, None, None, Vec::new());
        // What a file or folder does when clicked, dragged or right-clicked; the same in both views.
        let mut handle = |response: egui::Response, place: usize, actions: &mut Vec<Action>| {
            let (path, is_folder, _) = &rows[place];
            let selected = picked.contains(path);
            if *is_folder {
                if response.double_clicked() {
                    enter = Some(path.clone());
                }
                return;
            }
            if response.double_clicked() {
                actions.push(Action::SendClip(path.clone()));
            } else if response.clicked() {
                asked = Some((place, true));
            } else if response.drag_started() && !selected {
                asked = Some((place, false));
            }
            if response.dragged() {
                let carried: Vec<Source> = if selected {
                    rows.iter().filter(|(path, _, _)| picked.contains(path)).map(|(path, _, _)| Source::Clip(path.clone())).collect()
                } else {
                    vec![Source::Clip(path.clone())]
                };
                response.dnd_set_drag_payload(Dragged(carried));
            }
            response.context_menu(|ui| {
                let files: Vec<PathBuf> = if selected { picked.iter().cloned().collect() } else { vec![path.clone()] };
                ui.menu_button("Add to collection", |ui| {
                    for (index, collection) in self.collections.iter().enumerate() {
                        if ui.button(&collection.name).clicked() {
                            actions.push(Action::AddToCollection(index, files.clone()));
                            ui.close_menu();
                        }
                    }
                    if self.collections.is_empty() {
                        ui.weak("no collections yet");
                    }
                });
                if let Some(index) = viewing {
                    if ui.button("Remove from this collection").clicked() {
                        actions.push(Action::RemoveFromCollection(index, files.clone()));
                        ui.close_menu();
                    }
                }
            });
        };
        if self.assets_tiles {
            // Thumbnails: a tile each, made as they scroll into view.
            let tile = egui::vec2(104.0, 78.0);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                    for place in 0..rows.len() {
                        let (path, is_folder, _) = &rows[place];
                        let (rect, response) = ui.allocate_exact_size(tile, egui::Sense::click_and_drag());
                        if ui.is_rect_visible(rect) {
                            let painter = ui.painter_at(rect);
                            let picture = egui::Rect::from_min_size(rect.min, egui::vec2(tile.x, 58.5));
                            painter.rect_filled(rect, 3.0, egui::Color32::from_gray(if *is_folder { 52 } else { 34 }));
                            match self.asset_thumbs.get(path) {
                                Some(Some(texture)) => {
                                    // Fit the picture into the tile, whatever its shape.
                                    let size = texture.size_vec2();
                                    let fit = (picture.width() / size.x).min(picture.height() / size.y);
                                    let shown = egui::Rect::from_center_size(picture.center(), size * fit);
                                    painter.image(texture.id(), shown, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                                }
                                Some(None) => {}
                                None if !*is_folder => wanted.push(path.clone()),
                                None => {
                                    painter.text(picture.center(), egui::Align2::CENTER_CENTER, "folder", egui::FontId::proportional(12.0), egui::Color32::from_gray(170));
                                }
                            }
                            painter.text(
                                rect.left_bottom() + egui::vec2(3.0, -3.0),
                                egui::Align2::LEFT_BOTTOM,
                                name_of(path),
                                egui::FontId::proportional(10.0),
                                egui::Color32::from_gray(220),
                            );
                            let stroke = if picked.contains(path) { (2.0, egui::Color32::from_rgb(70, 140, 255)) } else { (1.0, egui::Color32::from_gray(60)) };
                            painter.rect_stroke(rect, 3.0, stroke, egui::StrokeKind::Inside);
                        }
                        handle(response.on_hover_text(path.display().to_string()), place, actions);
                    }
                });
            });
        } else {
            let row = ui.spacing().interact_size.y.max(ui.text_style_height(&egui::TextStyle::Body) + 4.0);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, row, rows.len(), |ui, shown| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                for place in shown {
                    let (path, is_folder, depth) = &rows[place];
                    ui.horizontal(|ui| {
                        ui.add_space(*depth as f32 * 14.0);
                        if *is_folder {
                            // The triangle opens the folder in place; a double-click on its name goes into it.
                            let open = self.assets_open.contains(path);
                            if ui.small_button(if open { "🔽" } else { "▶" }).clicked() {
                                toggle = Some(path.clone());
                            }
                        } else {
                            ui.add_space(22.0);
                        }
                        let label = if path.file_name().is_none() { path.display().to_string() } else { name_of(path) };
                        let response = ui.selectable_label(picked.contains(path), label).interact(egui::Sense::click_and_drag());
                        if rows.iter().position(|row| !row.1) == Some(place) {
                            self.seen_row.set(Some(response.rect));
                        }
                        handle(response.on_hover_text(path.display().to_string()), place, actions);
                    });
                }
            });
        }
        drop(handle);
        for path in wanted {
            // Asked once; `None` stands for "on its way" until the thread answers.
            if self.thumb_ask.send(path.clone()).is_ok() {
                self.asset_thumbs.insert(path, None);
            }
        }
        if let Some(folder) = toggle {
            if !self.assets_open.remove(&folder) {
                self.assets_open.insert(folder);
            }
        }
        if let Some(folder) = enter {
            self.assets_dir = folder;
            self.assets_picked.clear();
        }
        if let Some((place, clicked)) = asked {
            let path = rows[place].0.clone();
            match (clicked, modifiers.shift, modifiers.command, self.assets_anchor.filter(|anchor| *anchor < rows.len())) {
                (true, true, _, Some(anchor)) => {
                    let run = rows[anchor.min(place)..=anchor.max(place)].iter().filter(|(_, is_folder, _)| !is_folder);
                    self.assets_picked.extend(run.map(|(path, _, _)| path.clone()));
                }
                (true, _, true, _) => {
                    if !self.assets_picked.remove(&path) {
                        self.assets_picked.insert(path);
                    }
                    self.assets_anchor = Some(place);
                }
                _ => {
                    self.assets_picked = BTreeSet::from([path]);
                    self.assets_anchor = Some(place);
                }
            }
        }
    }
    fn close_preview(&mut self) {
        self.pre.active = None;
        if let Some(old) = self.pre.cells[0].take() {
            self.release(old);
        }
    }
    fn thumb_path(&self, entry: usize) -> PathBuf {
        self.root.join("target/desktop/thumbs").join(format!("{}.jpg", self.library[entry].id))
    }
    /// Draw a shader's still picture and keep it as a file, unless that is done already.
    fn render_thumb(&mut self, entry: usize) -> Result<()> {
        let path = self.thumb_path(entry);
        if path.is_file() {
            return Ok(());
        }
        let program = self.build(entry)?;
        let audio = still_audio();
        self.gfx.shader_pass(
            &program,
            &self.thumb_target,
            &gfx::Frame {
                time: 6.0,
                dt: 1.0 / 60.0,
                frame: 360,
                bpm: 128.0,
                tempo: rpi_vj_native::audio::Tempo::free(6.0, 128.0, 0.0),
                audio: &audio,
                flow: &[6.0; 4],
                spec_flow: &[3.0; 32],
                canvas: [1.0, 0.0, 0.0, 0.0],
                zoom: 1.0,
            },
        );
        self.gfx.delete(program);
        std::fs::create_dir_all(path.parent().unwrap_or(&self.root))?;
        image::save_buffer(&path, &self.gfx.read(&self.thumb_target), PREVIEW.0, PREVIEW.1, image::ColorType::Rgb8)?;
        Ok(())
    }
    /// The hover picture that was asked for, and a little of a run over the whole library.
    fn make_thumbs(&mut self, ctx: &egui::Context) {
        if let Some(entry) = self.thumb_wanted.take() {
            let loaded = self.render_thumb(entry).and_then(|()| Ok(image::open(self.thumb_path(entry))?.to_rgb8()));
            match loaded {
                Ok(picture) => {
                    let size = [picture.width() as usize, picture.height() as usize];
                    let image = egui::ColorImage::from_rgb(size, picture.as_raw());
                    let texture = ctx.load_texture(format!("thumb-{entry}"), image, egui::TextureOptions::LINEAR);
                    self.thumbs.insert(entry, texture);
                }
                Err(_) => {
                    self.thumb_failed.insert(entry);
                }
            }
        }
        if let Some(mut at) = self.thumb_run {
            let budget = Instant::now();
            while at < self.library.len() && budget.elapsed() < Duration::from_millis(20) {
                if self.library[at].runnable() && self.render_thumb(at).is_err() {
                    self.thumb_failed.insert(at);
                }
                at += 1;
            }
            self.thumb_run = (at < self.library.len()).then_some(at);
        }
    }
    /// Whether the chosen group holds this shader. Removed shaders show only in "Removed".
    fn in_group(&self, entry: usize) -> bool {
        let removed = self.removed.contains(&entry);
        match self.group {
            2 => removed,
            _ if removed => false,
            0 => true,
            1 => self.favorites.contains(&entry),
            3 => self.review.contains(&entry),
            group => self.groups.get(group - 4).is_some_and(|(_, members)| members.contains(&entry)),
        }
    }
    /// Empty the composition: every cell, layer and player.
    fn clear_composition(&mut self) {
        for layer in std::mem::take(&mut self.layers) {
            for cell in layer.cells.into_iter().flatten() {
                self.release(cell);
            }
            self.gfx.discard(layer.target);
            self.gfx.discard(layer.from);
        }
    }
    fn random(&mut self) -> usize {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed >> 16) as usize
    }
    /// Begin playing the checked shaders, in library order or shuffled.
    fn start_autoplay(&mut self) {
        let mut order: Vec<usize> = self.checked.iter().copied().filter(|entry| self.library[*entry].runnable()).collect();
        if order.is_empty() {
            return;
        }
        if self.auto_shuffle {
            self.shuffle(&mut order);
        }
        // One before the start, so the first change lands on the first shader.
        self.autoplay = Some(AutoPlay { at: order.len() - 1, order, ready: None });
        self.auto.active = None;
    }
    fn shuffle(&mut self, order: &mut [usize]) {
        for last in (1..order.len()).rev() {
            let other = self.random() % (last + 1);
            order.swap(last, other);
        }
    }
    fn stop_autoplay(&mut self) {
        if let Some((_, program)) = self.autoplay.take().and_then(|play| play.ready) {
            self.gfx.delete(program);
        }
        self.auto.active = None;
        if let Some(old) = self.auto.cells[0].take() {
            self.release(old);
        }
    }
    /// Put the next checked shader on the library layer. One that does not compile is
    /// passed over.
    fn autoplay_next(&mut self) {
        let Some(mut play) = self.autoplay.take() else { return };
        for _ in 0..play.order.len() {
            play.at = (play.at + 1) % play.order.len();
            if play.at == 0 && self.auto_shuffle {
                self.shuffle(&mut play.order);
            }
            let entry = play.order[play.at];
            let built = match play.ready.take() {
                Some((ready, program)) if ready == entry => Ok(program),
                other => {
                    if let Some((_, program)) = other {
                        self.gfx.delete(program);
                    }
                    self.build(entry)
                }
            };
            match built {
                Ok(program) => {
                    let mut cell = Cell::new(Source::Shader(entry));
                    (cell.program, cell.prepared) = (Some(program), true);
                    begin_transition(&self.gfx, &mut self.auto);
                    if let Some(old) = self.auto.cells[0].replace(cell) {
                        self.release(old);
                    }
                    self.auto.active = Some(0);
                    self.auto.launched = Instant::now();
                    self.auto_changes += 1;
                    break;
                }
                Err(error) => self.message = Some(format!("{}: {error:#}", self.library[entry].title())),
            }
        }
        self.autoplay = Some(play);
    }
    /// Compile the shader after the current one, a moment after a change.
    fn autoplay_ahead(&mut self) {
        let Some(play) = &self.autoplay else { return };
        if play.ready.is_some() || play.order.len() < 2 || self.auto.launched.elapsed() < Duration::from_secs(1) {
            return;
        }
        let at = (play.at + 1) % play.order.len();
        // A shuffle redeals at the end of a round, so the next one is not known there.
        if at == 0 && self.auto_shuffle {
            return;
        }
        let entry = play.order[at];
        let ready = self.build(entry).ok().map(|program| (entry, program));
        if let Some(play) = &mut self.autoplay {
            play.ready = ready;
        }
    }
    /// Where a layer on Sequence or Shuffle goes next, if its live cell has had its time.
    fn layer_next(&mut self, index: usize, new_section: bool) -> Option<usize> {
        let layer = &self.layers[index];
        let active = layer.active?;
        let cell = layer.cells[active].as_ref()?;
        let elapsed = layer.launched.elapsed().as_secs_f32();
        let due = elapsed >= cell.seconds || (layer.on_sections && new_section && elapsed > SECTION_MIN);
        let filled: Vec<usize> = (0..layer.cells.len()).filter(|column| layer.cells[*column].is_some()).collect();
        if layer.auto == 0 || !due || filled.len() < 2 {
            return None;
        }
        if layer.auto == 1 {
            return filled.iter().copied().find(|column| *column > active).or(filled.first().copied());
        }
        // Shuffle: every cell once before any comes round again.
        let mut fresh: Vec<usize> = filled.iter().copied().filter(|column| !layer.played.contains(column)).collect();
        if fresh.is_empty() {
            self.layers[index].played.clear();
            fresh = filled.into_iter().filter(|column| *column != active).collect();
        }
        let pick = self.random() % fresh.len();
        Some(fresh[pick])
    }
    fn listen(&mut self, source: usize) {
        self.capture = None;
        self.audio_error = None;
        if let Ok(mut frame) = self.shared.frame.lock() {
            *frame = AudioFrame::default();
        }
        match audio_in::start(&self.sources[source], Arc::clone(&self.shared)) {
            Ok(capture) => self.capture = Some(capture),
            Err(error) => self.audio_error = Some(format!("{error:#}")),
        }
        self.source = Some(source);
    }
    /// Where an edited copy of a shader's code is kept; the library file is never changed.
    fn edited_path(&self, entry: usize) -> PathBuf {
        self.root.join("target/desktop/user-shaders").join(format!("{}.fs", self.library[entry].id))
    }
    /// A shader's code: the edited copy if there is one, else the library file.
    fn source_text(&self, entry: usize) -> Result<String> {
        let edited = self.edited_path(entry);
        let path = if edited.is_file() { &edited } else { &self.library[entry].path };
        Ok(std::fs::read_to_string(path)?)
    }
    fn build(&self, entry: usize) -> Result<gfx::Program> {
        self.build_text(entry, &self.source_text(entry)?)
    }
    /// Compile `text` as this shader, with its look's numbers written in.
    fn build_text(&self, entry: usize, text: &str) -> Result<gfx::Program> {
        let params = self.looks.get(&entry).map(|look| look.params.as_slice()).unwrap_or(&[]);
        let entry = &self.library[entry];
        let channels = self.channels.get(&entry.id).map(Vec::as_slice).unwrap_or(&[]);
        let kinds: Vec<_> = channels.iter().map(|channel| (channel.channel, channel.kind)).collect();
        let prepared = shader::prepare_with(&with_params(text, params), &kinds)?;
        // The engine's canvas turns and shifts the picture; here it zooms it as well.
        let source = prepared
            .source
            .replacen("vec4 rpivj_frag() {", "uniform float u_canvas_zoom;\nvec4 rpivj_frag() {", 1)
            .replacen("(gl_FragCoord.xy - c)", "((gl_FragCoord.xy - c) / u_canvas_zoom)", 1);
        self.gfx.program(&source, &prepared.defaults, channels)
    }
    /// Have every cell that holds this shader compile again, after its code or numbers changed.
    fn refresh_cells(&mut self, entry: usize) {
        let (mut programs, mut pictures) = (Vec::new(), Vec::new());
        for layer in &mut self.layers {
            for cell in layer.cells.iter_mut().flatten().filter(|cell| cell.source == Source::Shader(entry)) {
                cell.prepared = false;
                cell.error = None;
                programs.extend(cell.program.take());
                pictures.extend(cell.thumb.take().map(|(target, _)| target));
            }
        }
        programs.into_iter().for_each(|program| self.gfx.delete(program));
        pictures.into_iter().for_each(|target| self.gfx.discard(target));
    }
    fn previewed(&self) -> Option<usize> {
        playing(&self.pre)
    }
    /// Begin downloading the ids in the box that the library does not have yet.
    fn start_download(&mut self) {
        let ids: Vec<String> = shadertoy_ids(&self.download_text).into_iter().filter(|id| !self.index_of.contains_key(id)).collect();
        if ids.is_empty() {
            self.message = Some("nothing to download: no new Shadertoy id in the box".into());
            return;
        }
        let log = Arc::new(std::sync::Mutex::new(vec![format!("{} to download: {}", ids.len(), ids.join(" "))]));
        let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (root, wanted, said, flag) = (self.root.clone(), ids.clone(), Arc::clone(&log), Arc::clone(&done));
        std::thread::spawn(move || {
            run_download(root, wanted, said);
            flag.store(true, Ordering::Relaxed);
        });
        self.download = Some(Download { log, done, ids });
    }
    /// Take a finished download into the library: new shaders go on the end, so every
    /// place that knows a shader by its position still holds, and into "to review".
    fn finish_download(&mut self) {
        let Some(job) = self.download.as_ref().filter(|job| job.done.swap(false, Ordering::Relaxed)) else { return };
        let (log, wanted) = (Arc::clone(&job.log), job.ids.clone());
        let fresh = library::load(&self.root).unwrap_or_default();
        let mut lines = Vec::new();
        for entry in fresh {
            if self.index_of.contains_key(&entry.id) || !wanted.contains(&entry.id) {
                continue;
            }
            let index = self.library.len();
            self.index_of.insert(entry.id.clone(), index);
            self.library.push(entry);
            self.review.insert(index);
        }
        self.channels = library::channels(&self.root);
        self.groups = library::groups(&self.library);
        for id in &wanted {
            lines.push(match self.index_of.get(id).copied() {
                Some(index) if !self.library[index].runnable() => format!("{id}: in the library, but multipass, so it cannot run"),
                Some(index) => match self.build(index) {
                    Ok(program) => {
                        self.gfx.delete(program);
                        format!("{id}: {} compiles; waiting for review", self.library[index].title())
                    }
                    Err(error) => format!("{id}: does not compile: {}", format!("{error:#}").lines().next().unwrap_or_default()),
                },
                None => format!("{id}: did not arrive"),
            });
        }
        lines.push("Done.".into());
        if let Ok(mut said) = log.lock() {
            said.extend(lines);
        }
        if !self.review.is_empty() {
            (self.group, self.view) = (3, View::All);
        }
    }
    /// The download box: links or ids in, the tools' own words out.
    fn download_window(&mut self, ctx: &egui::Context) {
        if !self.download_open {
            return;
        }
        let mut open = true;
        let running = self.download.as_ref().is_some_and(|job| !job.log.lock().is_ok_and(|lines| lines.last().is_some_and(|line| line == "Done." || line.ends_with("failed"))));
        egui::Window::new("Download from Shadertoy").open(&mut open).default_width(460.0).show(ctx, |ui| {
            ui.label("Shadertoy links or ids, one per line or separated by spaces:");
            ui.add(egui::TextEdit::multiline(&mut self.download_text).desired_rows(5).desired_width(f32::INFINITY).hint_text("https://www.shadertoy.com/view/XsXXDn"));
            ui.horizontal(|ui| {
                if ui.add_enabled(!running, egui::Button::new("Download")).clicked() {
                    self.start_download();
                }
                if running {
                    ui.spinner();
                    ui.weak("working; the browser window it opens closes by itself");
                }
            });
            ui.weak("New shaders arrive in the group \"New, to review\". Shadertoy is asked slowly, about 12 seconds a shader.");
            if let Some(job) = &self.download {
                let lines = job.log.lock().map(|lines| lines.clone()).unwrap_or_default();
                egui::ScrollArea::vertical().max_height(220.0).stick_to_bottom(true).show(ui, |ui| {
                    for line in &lines[lines.len().saturating_sub(200)..] {
                        ui.monospace(line);
                    }
                });
            }
        });
        self.download_open = open;
    }
    /// Under the preview: what can be changed about the shader being tried out. The
    /// changes stay with the shader and apply wherever it plays.
    fn editor(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        // The shader a cell was double-clicked for, else the one in the preview.
        let Some(entry) = self.editing.or(self.previewed()) else { return };
        if self.declared.as_ref().map(|(known, _)| *known) != Some(entry) {
            let text = self.source_text(entry).unwrap_or_default();
            let mut found: Vec<(String, f32, bool)> = Vec::new();
            for (name, value, whole) in text.lines().filter_map(param_of) {
                if found.len() < 16 && !found.iter().any(|(known, _, _)| known == name) {
                    found.push((name.to_owned(), value, whole));
                }
            }
            self.declared = Some((entry, found));
        }
        let before = self.looks.get(&entry).cloned().unwrap_or_default();
        let mut look = before.clone();
        let edited = self.edited_path(entry).is_file();
        ui.separator();
        ui.horizontal(|ui| {
            if ui.small_button(if self.editor_open { "🔽" } else { "▶" }).on_hover_text("Fold the editor away").clicked() {
                self.editor_open = !self.editor_open;
            }
            ui.strong("Shader editor");
            if self.editing == Some(entry) && ui.small_button("x").on_hover_text("Close the editor").clicked() {
                self.editing = None;
            }
            ui.add(egui::Label::new(self.library[entry].title()).truncate());
            if self.editor_open && ui.small_button("Reset").on_hover_text("Back to the shader as it came").clicked() {
                look = Look::default();
            }
            if edited {
                ui.weak("edited code in use");
            }
        });
        if !self.editor_open {
            return;
        }
        ui.weak("Double-click a slider to put it back.");
        ui.spacing_mut().slider_width = 130.0;
        egui::CollapsingHeader::new("Transform").default_open(true).show(ui, |ui| {
            let moved = ui.add(egui::Slider::new(&mut look.zoom, 0.2..=5.0).logarithmic(true).text("zoom"));
            if double_clicked(ui, &moved) {
                look.zoom = 1.0;
            }
            let moved = ui.add(egui::Slider::new(&mut look.rotate, -180.0..=180.0).suffix("°").text("turn"));
            if double_clicked(ui, &moved) {
                look.rotate = 0.0;
            }
            let moved = ui.add(egui::Slider::new(&mut look.pan[0], -1.0..=1.0).text("across"));
            if double_clicked(ui, &moved) {
                look.pan[0] = 0.0;
            }
            let moved = ui.add(egui::Slider::new(&mut look.pan[1], -1.0..=1.0).text("up"));
            if double_clicked(ui, &moved) {
                look.pan[1] = 0.0;
            }
        });
        egui::CollapsingHeader::new("Motion").default_open(true).show(ui, |ui| {
            let moved = ui.add(egui::Slider::new(&mut look.speed, -2.0..=4.0).text("speed"));
            if double_clicked(ui, &moved) {
                look.speed = 1.0;
            }
            let moved = ui.add(egui::Slider::new(&mut look.react, 0.0..=2.0).text("follows the music"));
            if double_clicked(ui, &moved) {
                look.react = 1.0;
            }
            ui.horizontal(|ui| {
                let chosen = look.react_mode.map_or("as the layer", |mode| REACT_MODES[usize::from(mode)]);
                egui::ComboBox::from_id_salt("look-react").width(110.0).selected_text(chosen).show_ui(ui, |ui| {
                    ui.selectable_value(&mut look.react_mode, None, "as the layer");
                    for (mode, name) in REACT_MODES.iter().enumerate() {
                        ui.selectable_value(&mut look.react_mode, Some(mode as u8), *name);
                    }
                });
                ui.label("react mode").on_hover_text("Set here, it wins over the cell's, the layer's and the library autoplay's");
            });
        });
        egui::CollapsingHeader::new("Colour").default_open(true).show(ui, |ui| {
            let moved = ui.add(egui::Slider::new(&mut look.hue, 0.0..=1.0).text("hue"));
            if double_clicked(ui, &moved) {
                look.hue = 0.0;
            }
            let moved = ui.add(egui::Slider::new(&mut look.saturation, 0.0..=2.0).text("saturation"));
            if double_clicked(ui, &moved) {
                look.saturation = 1.0;
            }
            let moved = ui.add(egui::Slider::new(&mut look.brightness, 0.0..=2.0).text("brightness"));
            if double_clicked(ui, &moved) {
                look.brightness = 1.0;
            }
            let moved = ui.add(egui::Slider::new(&mut look.contrast, 0.0..=2.0).text("contrast"));
            if double_clicked(ui, &moved) {
                look.contrast = 1.0;
            }
            let moved = ui.add(egui::Slider::new(&mut look.gamma, 0.3..=3.0).logarithmic(true).text("gamma"));
            if double_clicked(ui, &moved) {
                look.gamma = 1.0;
            }
            ui.checkbox(&mut look.invert, "invert");
        });
        egui::CollapsingHeader::new("The shader's own numbers").default_open(true).show(ui, |ui| {
            let declared = self.declared.as_ref().map(|(_, found)| found.as_slice()).unwrap_or(&[]);
            if declared.is_empty() {
                ui.weak("This shader declares no numbers up front. Its shapes are in its code.");
            }
            for (name, original, whole) in declared {
                let mut value = look.params.iter().find(|(known, _)| known == name).map_or(*original, |(_, value)| *value);
                let reach = if *original == 0.0 { 1.0 } else { original.abs() * 3.0 };
                let range = if *original > 0.0 { 0.0..=reach } else { -reach..=reach };
                let mut slider = egui::Slider::new(&mut value, range).text(name.as_str());
                if *whole {
                    slider = slider.step_by(1.0);
                }
                let moved = ui.add(slider);
                if double_clicked(ui, &moved) {
                    look.params.retain(|(known, _)| known != name);
                } else if moved.changed() {
                    look.params.retain(|(known, _)| known != name);
                    if value != *original {
                        look.params.push((name.clone(), value));
                    }
                }
            }
        });
        // The code itself, for what sliders cannot reach: shapes, sprites, everything.
        let (mut apply, mut revert, mut close, mut open) = (false, false, false, false);
        match &mut self.code {
            Some((shown, text, error)) if *shown == entry => {
                egui::ScrollArea::vertical().id_salt("code").max_height(300.0).show(ui, |ui| {
                    ui.add(egui::TextEdit::multiline(text).code_editor().desired_rows(14).desired_width(f32::INFINITY));
                });
                ui.horizontal(|ui| {
                    apply = ui.button("Apply").on_hover_text("Compile, and keep as an edited copy if it compiles").clicked();
                    revert = ui.add_enabled(edited, egui::Button::new("Back to the original")).clicked();
                    close = ui.button("Close code").clicked();
                });
                if let Some(error) = error {
                    ui.colored_label(RED, error.as_str());
                }
            }
            _ => open = ui.button("Edit code").clicked(),
        }
        if open {
            self.code = Some((entry, self.source_text(entry).unwrap_or_default(), None));
        }
        if apply {
            let text = self.code.as_ref().map(|(_, text, _)| text.clone()).unwrap_or_default();
            let kept = self.build_text(entry, &text).and_then(|program| {
                self.gfx.delete(program);
                let path = self.edited_path(entry);
                std::fs::create_dir_all(path.parent().unwrap_or(&self.root))?;
                Ok(std::fs::write(path, &text)?)
            });
            let failed = kept.err().map(|error| format!("{error:#}"));
            let compiled = failed.is_none();
            if let Some((_, _, error)) = &mut self.code {
                *error = failed;
            }
            if compiled {
                self.declared = None;
                self.refresh_cells(entry);
                if self.previewed() == Some(entry) {
                    actions.push(Action::Preview(entry));
                }
            }
        }
        if revert {
            let _ = std::fs::remove_file(self.edited_path(entry));
            self.code = Some((entry, self.source_text(entry).unwrap_or_default(), None));
            self.declared = None;
            self.refresh_cells(entry);
            actions.push(Action::Preview(entry));
        }
        if close {
            self.code = None;
        }
        if look != before {
            if look.params != before.params {
                // Numbers are written into the code, so it compiles again; soon, not on every step of a drag.
                self.rebuild_at = Some(Instant::now());
            }
            if look == Look::default() {
                self.looks.remove(&entry);
            } else {
                self.looks.insert(entry, look);
            }
        }
    }
    fn source_name(&self, source: &Source) -> String {
        match source {
            Source::Shader(entry) => self.library[*entry].title().to_owned(),
            Source::Clip(clip) => clip.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default(),
        }
    }
    fn release(&self, cell: Cell) {
        if let Some(program) = cell.program {
            self.gfx.delete(program);
        }
        if let Some((target, _)) = cell.thumb {
            self.gfx.discard(target);
        }
    }
    /// Compile a shader cell and draw its still picture. Clips need nothing.
    fn prepare(&mut self, layer: usize, column: usize, frame: &mut eframe::Frame) {
        let Some(Source::Shader(entry)) = self.layers[layer].cells[column].as_ref().map(|cell| cell.source.clone()) else {
            if let Some(cell) = &mut self.layers[layer].cells[column] {
                cell.prepared = true;
            }
            return;
        };
        let built = self.build(entry).and_then(|program| Ok((program, self.gfx.target(THUMB)?)));
        let Some(cell) = &mut self.layers[layer].cells[column] else { return };
        cell.prepared = true;
        match built {
            Ok((program, target)) => {
                let audio = still_audio();
                self.gfx.shader_pass(
                    &program,
                    &target,
                    &gfx::Frame {
                        time: 6.0,
                        dt: 1.0 / 60.0,
                        frame: 360,
                        bpm: 128.0,
                        tempo: rpi_vj_native::audio::Tempo::free(6.0, 128.0, 0.0),
                        audio: &audio,
                        flow: &[6.0; 4],
                        spec_flow: &[3.0; 32],
                        canvas: [1.0, 0.0, 0.0, 0.0],
                        zoom: 1.0,
                    },
                );
                let texture = frame.register_native_glow_texture(target.texture);
                cell.program = Some(program);
                cell.thumb = Some((target, texture));
            }
            Err(error) => cell.error = Some(format!("{error:#}")),
        }
    }
    /// Put `sources` into a layer from `column` on, one per cell, adding columns as needed.
    fn place(&mut self, layer: usize, column: usize, sources: Vec<Source>) {
        if layer >= self.layers.len() {
            return;
        }
        while self.columns < column + sources.len() {
            self.add_column();
        }
        for (offset, source) in sources.into_iter().enumerate() {
            let at = column + offset;
            if self.layers[layer].active == Some(at) {
                self.eject(layer);
            }
            if let Some(old) = self.layers[layer].cells[at].replace(Cell::new(source)) {
                self.release(old);
            }
        }
    }
    /// Lift cells out of the composition and set them down in one layer from `column`
    /// on, in their order, keeping their programs, pictures, times and react modes.
    fn move_cells(&mut self, mut cells: Vec<(usize, usize)>, layer: usize, column: usize) {
        cells.sort_unstable();
        cells.dedup();
        cells.retain(|(from, at)| self.layers.get(*from).is_some_and(|row| row.cells.get(*at).is_some_and(Option::is_some)));
        if layer >= self.layers.len() || cells.is_empty() {
            return;
        }
        let mut lifted = Vec::new();
        for (from, at) in &cells {
            if self.layers[*from].active == Some(*at) {
                self.eject(*from);
            }
            lifted.extend(self.layers[*from].cells[*at].take());
        }
        while self.columns < column + lifted.len() {
            self.add_column();
        }
        self.picked.clear();
        for (offset, cell) in lifted.into_iter().enumerate() {
            let at = column + offset;
            if self.layers[layer].active == Some(at) {
                self.eject(layer);
            }
            if let Some(old) = self.layers[layer].cells[at].replace(cell) {
                self.release(old);
            }
            self.picked.insert((layer, at));
        }
        self.selected_layer = layer;
    }
    fn add_column(&mut self) {
        self.columns += 1;
        for layer in &mut self.layers {
            layer.cells.push(None);
        }
    }
    fn clear_cell(&mut self, layer: usize, column: usize) {
        if self.layers[layer].active == Some(column) {
            self.eject(layer);
        }
        if let Some(old) = self.layers[layer].cells[column].take() {
            self.release(old);
        }
    }
    fn eject(&mut self, layer: usize) {
        let layer = &mut self.layers[layer];
        layer.active = None;
        if let Some(player) = &mut layer.player {
            let _ = player.stop();
        }
    }
    fn launch(&mut self, layer: usize, column: usize) {
        let Some(source) = self.layers[layer].cells[column].as_ref().map(|cell| cell.source.clone()) else { return };
        let layer = &mut self.layers[layer];
        layer.error = None;
        begin_transition(&self.gfx, layer);
        match source {
            Source::Shader(_) => {
                if let Some(player) = &mut layer.player {
                    let _ = player.stop();
                }
            }
            Source::Clip(clip) => {
                if layer.player.is_none() {
                    match mpv::Player::new() {
                        Ok(player) => layer.player = Some(player),
                        Err(error) => layer.error = Some(format!("{error:#}")),
                    }
                }
                if let Some(player) = &mut layer.player {
                    if let Err(error) = player.load(&clip) {
                        layer.error = Some(format!("{error:#}"));
                    }
                }
            }
        }
        layer.active = Some(column);
        layer.launched = Instant::now();
        layer.played.push(column);
    }
    /// Put a library shader on the selected layer's first free cell and launch it.
    fn send(&mut self, entry: usize) {
        let layer = self.selected_layer.min(self.layers.len().saturating_sub(1));
        let column = self.layers[layer].cells.iter().position(Option::is_none).unwrap_or(self.columns);
        self.place(layer, column, vec![Source::Shader(entry)]);
        self.launch(layer, column);
    }
    fn resize(&mut self, size: usize) -> Result<()> {
        let fresh = |old: &mut gfx::Target, gfx: &gfx::Gfx| -> Result<()> {
            let made = gfx.target(RENDER_SIZES[size])?;
            gfx.discard(std::mem::replace(old, made));
            Ok(())
        };
        for target in self.stack.iter_mut().chain(&mut self.scratch).chain([&mut self.out_target, &mut self.graded, &mut self.pre_out, &mut self.crossing]) {
            fresh(target, &self.gfx)?;
        }
        for layer in self.layers.iter_mut().chain([&mut self.auto, &mut self.pre]) {
            fresh(&mut layer.target, &self.gfx)?;
            fresh(&mut layer.from, &self.gfx)?;
        }
        self.out_texture = None;
        self.pre_texture = None;
        self.size = size;
        Ok(())
    }
    fn apply(&mut self, action: Action) {
        match action {
            Action::Launch(layer, column) => {
                self.picked.clear();
                self.pick_anchor = Some((layer, column));
                self.focus_layer = false;
                self.launch(layer, column);
            }
            Action::PickCell(layer, column, run) => {
                self.selected_layer = layer;
                self.focus_layer = false;
                match self.pick_anchor.filter(|anchor| run && anchor.0 == layer) {
                    Some((_, from)) => {
                        let filled = (from.min(column)..=from.max(column)).filter(|at| self.layers[layer].cells[*at].is_some());
                        self.picked.extend(filled.map(|at| (layer, at)));
                    }
                    None => {
                        if !self.picked.remove(&(layer, column)) {
                            self.picked.insert((layer, column));
                        }
                        self.pick_anchor = Some((layer, column));
                    }
                }
            }
            Action::MoveCells(cells, layer, column) => self.move_cells(cells, layer, column),
            Action::ClearCells(cells) => {
                for (layer, column) in cells {
                    if layer < self.layers.len() && column < self.columns {
                        self.clear_cell(layer, column);
                    }
                }
                self.picked.clear();
            }
            Action::Eject(layer) => self.eject(layer),
            Action::Place(layer, column, sources) => self.place(layer, column, sources),
            Action::ClearCell(layer, column) => self.clear_cell(layer, column),
            Action::ClearLayer(layer) => (0..self.columns).for_each(|column| self.clear_cell(layer, column)),
            Action::AddColumn => self.add_column(),
            Action::AddLayer => match self.make_layer(format!("Layer {}", self.layers.len() + 1)) {
                Ok(layer) => self.layers.push(layer),
                Err(error) => self.message = Some(format!("{error:#}")),
            },
            Action::RemoveLayer(layer) => {
                if self.layers.len() > 1 {
                    (0..self.columns).for_each(|column| self.clear_cell(layer, column));
                    self.picked.clear();
                    let removed = self.layers.remove(layer);
                    self.gfx.discard(removed.target);
                    self.gfx.discard(removed.from);
                    self.selected_layer = self.selected_layer.min(self.layers.len() - 1);
                }
            }
            Action::Send(entry) => self.send(entry),
            Action::AddEffect(target, kind) => {
                if let Some(chain) = self.chain(target) {
                    chain.push(Effect { kind, amount: 0.5, bypass: false });
                }
            }
            Action::RemoveEffect(target, at) => {
                if let Some(chain) = self.chain(target).filter(|chain| at < chain.len()) {
                    chain.remove(at);
                }
            }
            Action::MoveEffect(target, at, up) => {
                if let Some(chain) = self.chain(target) {
                    let to = if up { at.wrapping_sub(1) } else { at + 1 };
                    if at < chain.len() && to < chain.len() {
                        chain.swap(at, to);
                    }
                }
            }
            Action::Preview(entry) => match self.build(entry) {
                Ok(program) => {
                    let mut cell = Cell::new(Source::Shader(entry));
                    (cell.program, cell.prepared) = (Some(program), true);
                    if let Some(old) = self.pre.cells[0].replace(cell) {
                        self.release(old);
                    }
                    self.pre.active = Some(0);
                    // The editor follows the preview, unless this is the shader it already shows.
                    if self.editing != Some(entry) {
                        self.editing = None;
                    }
                }
                Err(error) => self.message = Some(format!("{}: {error:#}", self.library[entry].title())),
            },
            Action::Favorite(entries, on) => {
                for entry in entries {
                    if on {
                        self.favorites.insert(entry);
                    } else {
                        self.favorites.remove(&entry);
                    }
                }
            }
            Action::AddEffectToFocus(kind) => {
                let cells = self.focus_cells();
                let targets: Vec<ChainAt> = if cells.is_empty() {
                    vec![ChainAt::Layer(self.selected_layer)]
                } else {
                    cells.into_iter().map(|(layer, column)| ChainAt::Cell(layer, column)).collect()
                };
                for target in targets {
                    if let Some(chain) = self.chain(target) {
                        chain.push(Effect { kind, amount: 0.5, bypass: false });
                    }
                }
            }
            Action::Edit(entry) => {
                self.editing = Some(entry);
                self.editor_open = true;
            }
            Action::Undo => self.step_history(true),
            Action::Redo => self.step_history(false),
            Action::SendClip(clip) => {
                let layer = self.selected_layer.min(self.layers.len().saturating_sub(1));
                let column = self.layers[layer].cells.iter().position(Option::is_none).unwrap_or(self.columns);
                self.place(layer, column, vec![Source::Clip(clip)]);
                self.launch(layer, column);
            }
            Action::AddToCollection(index, files) => {
                if let Some(collection) = self.collections.get_mut(index) {
                    for file in files {
                        if !collection.items.contains(&file) {
                            collection.items.push(file);
                        }
                    }
                }
            }
            Action::RemoveFromCollection(index, files) => {
                if let Some(collection) = self.collections.get_mut(index) {
                    collection.items.retain(|item| !files.contains(item));
                }
                self.assets_picked.clear();
            }
            Action::DeleteCollection(index) => {
                if index < self.collections.len() {
                    self.collections.remove(index);
                }
                self.collection_view = None;
            }
            Action::Reviewed(entries) => {
                for entry in entries {
                    self.review.remove(&entry);
                }
            }
            Action::Remove(entries, on) => {
                for entry in entries {
                    if on {
                        self.removed.insert(entry);
                        self.selection.remove(&entry);
                        self.checked.remove(&entry);
                    } else {
                        self.removed.remove(&entry);
                    }
                }
            }
            Action::SetSeconds(layer, column, seconds) => {
                if let Some(cell) = &mut self.layers[layer].cells[column] {
                    cell.seconds = seconds.clamp(1.0, 36000.0);
                }
            }
            Action::SetReact(layer, column, react) => {
                if let Some(cell) = &mut self.layers[layer].cells[column] {
                    cell.react = react;
                }
            }
            Action::AddToFolder(folder, entries) => {
                for entry in entries {
                    let id = &self.library[entry].id;
                    if !self.folders[folder].ids.contains(id) {
                        self.folders[folder].ids.push(id.clone());
                    }
                }
            }
            Action::RemoveFromFolder(folder, entries) => {
                let gone: Vec<&String> = entries.iter().map(|entry| &self.library[*entry].id).collect();
                self.folders[folder].ids.retain(|id| !gone.contains(&id));
            }
            Action::DeleteFolder(folder) => {
                self.folders.remove(folder);
                self.view = View::All;
            }
            // Handled where the actions are gone through, with the painter at hand.
            Action::Spot(..) => {}
            Action::LoadPreset(index) => {
                let Some(preset) = self.presets.get(index) else { return };
                self.checked = preset.ids.iter().filter_map(|id| self.index_of.get(id).copied()).collect();
                self.auto_shuffle = preset.shuffle;
                self.auto_seconds = preset.seconds;
                self.auto_sections = preset.on_sections;
                self.auto.react = preset.react;
                self.auto.opacity = preset.opacity;
                self.auto.blend = preset.blend;
                self.preset_name = preset.name.clone();
                // Recalled to be played: start at once.
                self.stop_autoplay();
                self.start_autoplay();
            }
            Action::DeletePreset(index) => {
                if index < self.presets.len() {
                    self.presets.remove(index);
                }
            }
            Action::OpenComposition(saved) => {
                let before = (std::mem::take(&mut self.layers), self.columns);
                if self.load_layers(&saved) {
                    let opened = std::mem::replace(&mut self.layers, before.0);
                    self.clear_composition();
                    self.picked.clear();
                    self.layers = opened;
                    self.selected_layer = self.selected_layer.min(self.layers.len() - 1);
                } else {
                    (self.layers, self.columns) = before;
                    self.message = Some("that file holds no composition".into());
                }
            }
        }
    }
    /// One frame of sound and picture: the Pi's chain up to the predictor, then each
    /// layer's own react mode, then the stack.
    fn tick(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f32().min(0.1);
        self.last = now;
        self.frames += 1;
        if dt > 0.0 {
            self.fps += (1.0 / dt - self.fps) * 0.05;
        }
        self.shared.gain.store(self.gain.to_bits(), Ordering::Relaxed);
        if let Some(error) = self.shared.error.lock().ok().and_then(|mut slot| slot.take()) {
            self.audio_error = Some(error);
            self.capture = None;
        }
        let latest = self.shared.frame.lock().map(|frame| frame.clone()).unwrap_or_default();
        self.heard = if self.capture.is_some() { latest } else { AudioFrame::default() };
        let raw = f32::from_bits(self.shared.peak_raw.swap(0, Ordering::Relaxed));
        let peak = f32::from_bits(self.shared.peak.swap(0, Ordering::Relaxed));
        self.level = peak.max(self.level * (-dt / 0.3).exp());
        if peak >= self.held.0 || self.held.1.elapsed() > Duration::from_millis(1500) {
            self.held = (self.level, now);
        }
        if raw > 0.98 || peak > 1.0 {
            self.clipped = Some(now);
        }
        let heard = &self.heard;
        let new_section = self.sections.tick(dt, heard);
        self.section_count += u32::from(new_section);
        let quiet = heard.volume < 0.01 && heard.bass < 0.01 && heard.mid < 0.01;
        if self.idle == 0.0 {
            self.idle_since = now;
        }
        self.idle = (self.idle + if quiet { dt * 0.4 } else { -dt * 4.0 }).clamp(0.0, 1.0);
        let timed = self.predictor.apply(dt, self.lead_ms / 1000.0, heard);
        let moving = idle_audio(&timed, self.idle_since.elapsed().as_secs_f32(), self.idle);
        let found = self.predictor.bpm();
        let bpm = if found > 0.0 { found } else { 128.0 };
        let (time, frame) = (self.start.elapsed().as_secs_f32(), self.frames as i32);
        for layer in self.layers.iter_mut().chain([&mut self.auto, &mut self.pre]) {
            layer.drawn = false;
            if new_section {
                layer.reactor.new_section();
            }
            let Some(cell) = layer.active.and_then(|column| layer.cells[column].as_ref()) else { continue };
            match (&cell.source, &cell.program) {
                (Source::Shader(entry), Some(program)) => {
                    let look = self.looks.get(entry);
                    // The shader's own mode first, then the cell's, then the layer's.
                    let react = look.and_then(|look| look.react_mode).or(cell.react).unwrap_or(layer.react);
                    let audio = scaled(&layer.reactor.apply(react, dt, &moving), look.map_or(1.0, |look| look.react));
                    layer.flash = layer.reactor.flash(react);
                    layer.time += dt * look.map_or(1.0, |look| look.speed);
                    for (clock, energy) in layer.flow.iter_mut().zip([audio.volume, audio.bass, audio.mid, audio.treble]) {
                        *clock += dt * (FLOW_RATE.0 + energy * FLOW_RATE.1);
                    }
                    for (clock, energy) in layer.spec_flow.iter_mut().zip(gfx::spectrum_bands(&audio)) {
                        *clock += dt * (SPEC_FLOW_RATE.0 + energy * SPEC_FLOW_RATE.1);
                    }
                    let tempo = layer.clock.tick(dt, bpm, &audio);
                    self.gfx.shader_pass(
                        program,
                        &layer.target,
                        &gfx::Frame {
                            time: layer.time,
                            dt,
                            frame,
                            bpm,
                            tempo,
                            audio: &audio,
                            flow: &layer.flow,
                            spec_flow: &layer.spec_flow,
                            canvas: look.map_or([1.0, 0.0, 0.0, 0.0], Look::canvas),
                            zoom: look.map_or(1.0, |look| look.zoom),
                        },
                    );
                    layer.drawn = true;
                }
                (Source::Clip(_), _) => {
                    layer.flash = [1.0, 0.0, 0.0, 0.0];
                    if let Some(player) = &mut layer.player {
                        let played = player.poll().and_then(|()| player.render(&self.gfx.gl, &layer.target));
                        match played {
                            Ok(()) => layer.drawn = true,
                            Err(error) => layer.error = Some(format!("{error:#}")),
                        }
                    }
                }
                // A shader still waiting for its program: next frame.
                (Source::Shader(_), None) => {}
            }
        }
        // The stack, bottom layer first, each laid over what is below it.
        let (mut below, mut next) = (None, 0);
        for layer in self.layers.iter().chain([&self.auto]).filter(|layer| layer.drawn && !layer.bypass) {
            // Colours, the cell's own effects, the change from the cell before, the layer's effects.
            let picture = graded(&self.gfx, &self.looks, layer, &self.graded);
            let own = layer.active.and_then(|column| layer.cells[column].as_ref()).map(|cell| cell.effects.as_slice()).unwrap_or(&[]);
            let (mut picture, first) = run_effects(&self.gfx, &self.scratch, picture, own, 0, time, moving.onset, moving.bass);
            let since = layer.switched.elapsed().as_secs_f32();
            if layer.transition > 0 && since < layer.transition_seconds {
                self.gfx.transition(&self.crossing, layer.from.texture, picture, layer.transition as i32, since / layer.transition_seconds);
                picture = self.crossing.texture;
            }
            let (picture, _) = run_effects(&self.gfx, &self.scratch, picture, &layer.effects, first, time, moving.onset, moving.bass);
            self.gfx.mix(
                &self.stack[next],
                &gfx::Mix {
                    base: below,
                    top: Some(picture),
                    opacity: layer.opacity,
                    mode: layer.blend as i32,
                    fx: 0,
                    fx_amount: 0.0,
                    flash: layer.flash,
                },
            );
            below = Some(self.stack[next].texture);
            next ^= 1;
        }
        match below {
            Some(picture) => {
                let (picture, _) = run_effects(&self.gfx, &self.scratch, picture, &self.master, 0, time, moving.onset, moving.bass);
                self.gfx.mix(&self.out_target, &gfx::Mix::copy(picture));
            }
            None => self.gfx.clear(&self.out_target),
        }
        // The preview is not part of the output; it gets its shader's colours and a
        // picture of its own.
        if self.pre.drawn {
            let picture = graded(&self.gfx, &self.looks, &self.pre, &self.graded);
            self.gfx.mix(&self.pre_out, &gfx::Mix::copy(picture));
        }
        // Layers on Sequence or Shuffle, and the library's checked list, move on.
        for index in 0..self.layers.len() {
            if let Some(column) = self.layer_next(index, new_section) {
                self.launch(index, column);
            }
        }
        if self.autoplay.is_some() {
            let elapsed = self.auto.launched.elapsed().as_secs_f32();
            let due = elapsed >= self.auto_seconds || (self.auto_sections && new_section && elapsed > SECTION_MIN);
            if self.auto.active.is_none() || due {
                self.autoplay_next();
            } else {
                self.autoplay_ahead();
            }
        }
    }
    /// A few shaders per frame, so the window stays alive while the whole library compiles.
    fn compile_some(&mut self, ctx: &egui::Context) {
        let Some(mut run) = self.compile.take() else { return };
        let budget = Instant::now();
        while run.next < self.library.len() && budget.elapsed() < Duration::from_millis(30) {
            let entry = run.next;
            run.next += 1;
            if !self.library[entry].runnable() {
                continue;
            }
            match self.build(entry) {
                Ok(program) => {
                    self.gfx.delete(program);
                    run.passed += 1;
                }
                Err(error) => run.failures.push((self.library[entry].id.clone(), format!("{error:#}"))),
            }
        }
        if run.next < self.library.len() {
            self.compile = Some(run);
            return;
        }
        let report = serde_json::json!({
            "dialect": self.gfx.dialect(),
            "compiled": run.passed,
            "failed": run.failures.len(),
            "seconds": run.started.elapsed().as_secs_f32(),
            "failures": run.failures.iter().map(|(id, error)| serde_json::json!({"id": id, "error": error})).collect::<Vec<_>>(),
        });
        let path = self.root.join("target/desktop/compile-report.json");
        let written = std::fs::create_dir_all(path.parent().unwrap())
            .and_then(|()| std::fs::write(&path, serde_json::to_vec_pretty(&report).unwrap_or_default()));
        println!(
            "compiled {} of {} runnable shaders as {} in {:.1} s; {} failed; report {} ({written:?})",
            run.passed,
            run.passed + run.failures.len(),
            self.gfx.dialect(),
            run.started.elapsed().as_secs_f32(),
            run.failures.len(),
            path.display()
        );
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
    /// The composition, folders and settings as they are kept between runs.
    fn state(&self) -> serde_json::Value {
        let cell = |cell: &Option<Cell>| match cell.as_ref().map(|cell| &cell.source) {
            Some(Source::Shader(entry)) => serde_json::json!({ "shader": self.library[*entry].id }),
            Some(Source::Clip(clip)) => serde_json::json!({ "clip": clip }),
            None => serde_json::Value::Null,
        };
        let cell = |saved: &Option<Cell>| {
            let mut value = cell(saved);
            if let (Some(kept), Some(map)) = (saved, value.as_object_mut()) {
                map.insert("seconds".into(), kept.seconds.into());
                map.insert("react".into(), kept.react.into());
                map.insert("effects".into(), effects_json(&kept.effects));
            }
            value
        };
        let layers: Vec<_> = self
            .layers
            .iter()
            .map(|layer| {
                serde_json::json!({
                    "name": layer.name,
                    "active": layer.active,
                    "opacity": layer.opacity,
                    "blend": layer.blend,
                    "effects": effects_json(&layer.effects),
                    "react": layer.react,
                    "auto": layer.auto,
                    "on_sections": layer.on_sections,
                    "transition": gfx::TRANSITIONS[layer.transition],
                    "transition_seconds": layer.transition_seconds,
                    "cells": layer.cells.iter().map(cell).collect::<Vec<_>>(),
                })
            })
            .collect();
        let folders: Vec<_> =
            self.folders.iter().map(|folder| serde_json::json!({ "name": folder.name, "ids": folder.ids })).collect();
        serde_json::json!({
            "version": 1,
            "columns": self.columns,
            "layers": layers,
            "folders": folders,
            "presets": self.presets.iter().map(|preset| serde_json::json!({
                "name": preset.name,
                "ids": preset.ids,
                "shuffle": preset.shuffle,
                "seconds": preset.seconds,
                "on_sections": preset.on_sections,
                "react": preset.react,
                "opacity": preset.opacity,
                "blend": preset.blend,
            })).collect::<Vec<_>>(),
            "favorites": self.favorites.iter().map(|entry| self.library[*entry].id.as_str()).collect::<Vec<_>>(),
            "removed": self.removed.iter().map(|entry| self.library[*entry].id.as_str()).collect::<Vec<_>>(),
            "master": effects_json(&self.master),
            "looks": self.looks.iter().map(|(entry, look)| (self.library[*entry].id.clone(), look.json())).collect::<serde_json::Map<_, _>>(),
            "collections": self.collections.iter().map(|collection| serde_json::json!({ "name": collection.name, "items": collection.items })).collect::<Vec<_>>(),
            "assets_dir": self.assets_dir,
            "review": self.review.iter().map(|entry| self.library[*entry].id.as_str()).collect::<Vec<_>>(),
            "detached": self.detached,
            "led": self.led,
            "apc": self.apc,
            "midi": self.bindings.iter().map(|((kind, channel, number), control)| serde_json::json!({
                "kind": kind,
                "channel": channel,
                "number": number,
                "control": control.name(),
            })).collect::<Vec<_>>(),
            "gain": self.gain,
            "lead_ms": self.lead_ms,
            "size": self.size,
            "output_display": self.output_display,
            "checked": self.checked.iter().map(|entry| self.library[*entry].id.as_str()).collect::<Vec<_>>(),
            "auto": {
                "shuffle": self.auto_shuffle,
                "seconds": self.auto_seconds,
                "on_sections": self.auto_sections,
                "react": self.auto.react,
                "opacity": self.auto.opacity,
                "blend": self.auto.blend,
            },
            "audio": self.source.map(|index| self.sources[index].label()),
        })
    }
    fn save_state(&mut self) {
        self.saved_at = Instant::now();
        let Some(path) = &self.state_path else { return };
        let written = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(path, serde_json::to_vec_pretty(&self.state()).unwrap_or_default()));
        if let Err(error) = written {
            self.message = Some(format!("could not save to {}: {error}", path.display()));
        }
    }
    /// Build the layers a saved state or composition file describes; false, with nothing
    /// changed that matters, when it describes none.
    fn load_layers(&mut self, state: &serde_json::Value) -> bool {
        let layers = state["layers"].as_array().cloned().unwrap_or_default();
        if layers.is_empty() {
            return false;
        }
        let number = |value: &serde_json::Value, otherwise: f32| value.as_f64().map_or(otherwise, |v| v as f32);
        let widest = layers.iter().map(|layer| layer["cells"].as_array().map_or(0, Vec::len)).max().unwrap_or(0);
        self.columns = (state["columns"].as_u64().unwrap_or(8) as usize).max(widest).clamp(1, 256);
        for saved in &layers {
            let Ok(mut layer) = self.make_layer(saved["name"].as_str().unwrap_or("Layer").to_owned()) else { return false };
            layer.opacity = number(&saved["opacity"], 1.0).clamp(0.0, 1.0);
            layer.blend = (saved["blend"].as_u64().unwrap_or(0) as usize).min(gfx::BLENDS.len() - 1);
            layer.effects = effects_from(&saved["effects"]);
            // A file from before effect chains held one effect by number.
            if let Some(kind) = saved["fx"].as_u64().map(|kind| kind as usize).filter(|kind| (1..8).contains(kind)) {
                layer.effects.push(Effect { kind, amount: number(&saved["fx_amount"], 0.5).clamp(0.0, 1.0), bypass: false });
            }
            let react = |value: &serde_json::Value| value.as_u64().map(|mode| (mode as u8).min(REACT_MODES.len() as u8 - 1));
            layer.react = react(&saved["react"]).unwrap_or(0);
            layer.auto = (saved["auto"].as_u64().unwrap_or(0) as usize).min(AUTO_MODES.len() - 1);
            layer.on_sections = saved["on_sections"].as_bool().unwrap_or(false);
            if let Some(kind) = gfx::TRANSITIONS.iter().position(|name| Some(*name) == saved["transition"].as_str()) {
                layer.transition = kind;
            }
            layer.transition_seconds = number(&saved["transition_seconds"], 0.5).clamp(0.05, 30.0);
            for (column, cell) in saved["cells"].as_array().into_iter().flatten().enumerate() {
                let source = match (cell["shader"].as_str(), cell["clip"].as_str()) {
                    (Some(id), _) => self.index_of.get(id).map(|entry| Source::Shader(*entry)),
                    (_, Some(clip)) => Some(Source::Clip(clip.into())),
                    _ => None,
                };
                layer.cells[column] = source.map(|source| {
                    let mut kept = Cell::new(source);
                    kept.seconds = number(&cell["seconds"], 20.0).clamp(1.0, 36000.0);
                    kept.react = react(&cell["react"]);
                    kept.effects = effects_from(&cell["effects"]);
                    kept
                });
            }
            self.layers.push(layer);
        }
        true
    }
    /// Notice changes for undo: a couple of times a second, and not while a button is
    /// held, so a slider being dragged is one step, not hundreds.
    fn track_undo(&mut self, ctx: &egui::Context) {
        if self.undo_looked.elapsed() < Duration::from_millis(400) || ctx.input(|input| input.pointer.any_down()) {
            return;
        }
        self.undo_looked = Instant::now();
        let now = self.state();
        if undo_view(&now) == undo_view(&self.undo_base) {
            self.undo_base = now;
            return;
        }
        self.undo.push(std::mem::replace(&mut self.undo_base, now));
        self.redo.clear();
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
    }
    /// One step back, or forward again.
    fn step_history(&mut self, back: bool) {
        // A change not noticed yet is a step of its own.
        let now = self.state();
        if undo_view(&now) != undo_view(&self.undo_base) {
            self.undo.push(std::mem::replace(&mut self.undo_base, now.clone()));
            self.redo.clear();
        }
        let Some(target) = (if back { self.undo.pop() } else { self.redo.pop() }) else { return };
        if back {
            self.redo.push(now);
        } else {
            self.undo.push(now);
        }
        self.restore(&target);
        self.undo_base = self.state();
        self.undo_looked = Instant::now();
    }
    /// Put the composition, effects, looks, folders, collections and presets back to a
    /// saved state. This computer's own settings stay as they are, and the cells that
    /// were playing in that state play again.
    fn restore(&mut self, saved: &serde_json::Value) {
        let kept = (self.gain, self.lead_ms, self.output_display, self.detached, self.led, self.apc, std::mem::take(&mut self.bindings), self.assets_dir.clone());
        self.clear_composition();
        self.picked.clear();
        self.folders.clear();
        self.presets.clear();
        self.collections.clear();
        self.looks.clear();
        let mut audio = None;
        if !self.apply_state(saved, &mut audio) && self.layers.is_empty() {
            if let Ok(layer) = self.make_layer("Layer 1".into()) {
                self.layers.push(layer);
            }
        }
        (self.gain, self.lead_ms, self.output_display, self.detached, self.led, self.apc, self.bindings, self.assets_dir) = kept;
        self.selected_layer = self.selected_layer.min(self.layers.len().saturating_sub(1));
        if matches!(self.view, View::Folder(folder) if folder >= self.folders.len()) {
            self.view = View::All;
        }
        self.collection_view = self.collection_view.filter(|index| *index < self.collections.len());
        for (index, layer) in saved["layers"].as_array().into_iter().flatten().enumerate() {
            let live = layer["active"].as_u64().map(|column| column as usize);
            if let Some(column) = live.filter(|column| self.layers.get(index).is_some_and(|row| row.cells.get(*column).is_some_and(Option::is_some))) {
                self.launch(index, column);
            }
        }
        self.lights.clear();
    }
    /// Bring back what `save_state` wrote; false when there is nothing usable.
    fn load_state(&mut self, audio: &mut Option<String>) -> bool {
        let Some(text) = self.state_path.as_ref().and_then(|path| std::fs::read_to_string(path).ok()) else {
            return false;
        };
        let Ok(state) = serde_json::from_str::<serde_json::Value>(&text) else { return false };
        self.apply_state(&state, audio)
    }
    /// Take everything a saved state holds; false when it holds no composition.
    fn apply_state(&mut self, state: &serde_json::Value, audio: &mut Option<String>) -> bool {
        if !self.load_layers(state) {
            return false;
        }
        let number = |value: &serde_json::Value, otherwise: f32| value.as_f64().map_or(otherwise, |v| v as f32);
        for folder in state["folders"].as_array().into_iter().flatten() {
            self.folders.push(Folder {
                name: folder["name"].as_str().unwrap_or("Folder").to_owned(),
                ids: folder["ids"].as_array().into_iter().flatten().filter_map(|id| id.as_str().map(str::to_owned)).collect(),
            });
        }
        let whole = |value: &serde_json::Value| value.as_u64().unwrap_or(0);
        for preset in state["presets"].as_array().into_iter().flatten() {
            self.presets.push(Preset {
                name: preset["name"].as_str().unwrap_or("preset").to_owned(),
                ids: preset["ids"].as_array().into_iter().flatten().filter_map(|id| id.as_str().map(str::to_owned)).collect(),
                shuffle: preset["shuffle"].as_bool().unwrap_or(false),
                seconds: number(&preset["seconds"], 20.0).clamp(1.0, 36000.0),
                on_sections: preset["on_sections"].as_bool().unwrap_or(true),
                react: (whole(&preset["react"]) as u8).min(REACT_MODES.len() as u8 - 1),
                opacity: number(&preset["opacity"], 1.0).clamp(0.0, 1.0),
                blend: (whole(&preset["blend"]) as usize).min(gfx::BLENDS.len() - 1),
            });
        }
        let entries = |value: &serde_json::Value| -> BTreeSet<usize> {
            value.as_array().into_iter().flatten().filter_map(|id| self.index_of.get(id.as_str()?).copied()).collect()
        };
        (self.favorites, self.removed) = (entries(&state["favorites"]), entries(&state["removed"]));
        self.master = effects_from(&state["master"]);
        self.review = entries(&state["review"]);
        for saved in state["collections"].as_array().into_iter().flatten() {
            self.collections.push(Collection {
                name: saved["name"].as_str().unwrap_or("Collection").to_owned(),
                items: saved["items"].as_array().into_iter().flatten().filter_map(|item| item.as_str().map(PathBuf::from)).collect(),
            });
        }
        if let Some(folder) = state["assets_dir"].as_str().map(PathBuf::from).filter(|folder| folder.is_dir()) {
            self.assets_dir = folder;
        }
        for (id, saved) in state["looks"].as_object().into_iter().flatten() {
            if let Some(entry) = self.index_of.get(id) {
                self.looks.insert(*entry, Look::from(saved));
            }
        }
        self.detached = state["detached"].as_bool().unwrap_or(false);
        if let Some(led) = state["led"].as_array().filter(|led| led.len() == 5) {
            self.led = std::array::from_fn(|index| (whole(&led[index]) as u8).min(127));
        }
        self.apc = state["apc"].as_bool().unwrap_or(self.apc) && self.midi.has(apc::NAME);
        for binding in state["midi"].as_array().into_iter().flatten() {
            if let Some(control) = binding["control"].as_str().and_then(Control::parse) {
                let key = (whole(&binding["kind"]) as u8, whole(&binding["channel"]) as u8, whole(&binding["number"]) as u8);
                self.bindings.push((key, control));
            }
        }
        let checked = state["checked"].as_array().into_iter().flatten();
        self.checked = checked.filter_map(|id| self.index_of.get(id.as_str()?).copied()).collect();
        let auto = &state["auto"];
        self.auto_shuffle = auto["shuffle"].as_bool().unwrap_or(false);
        self.auto_seconds = number(&auto["seconds"], 20.0).clamp(1.0, 36000.0);
        self.auto_sections = auto["on_sections"].as_bool().unwrap_or(true);
        self.auto.react = (auto["react"].as_u64().unwrap_or(0) as u8).min(REACT_MODES.len() as u8 - 1);
        self.auto.opacity = number(&auto["opacity"], 1.0).clamp(0.0, 1.0);
        self.auto.blend = (auto["blend"].as_u64().unwrap_or(2) as usize).min(gfx::BLENDS.len() - 1);
        self.gain = number(&state["gain"], 1.0).clamp(0.05, 16.0);
        self.lead_ms = number(&state["lead_ms"], BEAT_LEAD * 1000.0).clamp(0.0, 250.0);
        let size = (state["size"].as_u64().unwrap_or(1) as usize).min(RENDER_SIZES.len() - 1);
        if size != self.size && self.resize(size).is_err() {
            return false;
        }
        self.output_display =
            (state["output_display"].as_u64().unwrap_or(0) as usize).min(self.displays.len().saturating_sub(1));
        *audio = state["audio"].as_str().map(str::to_owned);
        self.selected_layer = self.selected_layer.min(self.layers.len() - 1);
        true
    }
    fn top_bar(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let learn = self.learn;
        ui.horizontal_wrapped(|ui| {
            ui.label("Audio in");
            let current = self.source.map(|index| self.sources[index].label()).unwrap_or_else(|| "none".into());
            let mut chosen = None;
            egui::ComboBox::from_id_salt("audio-source").width(260.0).selected_text(current).show_ui(ui, |ui| {
                for (index, source) in self.sources.iter().enumerate() {
                    if ui.selectable_label(self.source == Some(index), source.label()).clicked() {
                        chosen = Some(index);
                    }
                }
            });
            if let Some(index) = chosen {
                self.listen(index);
            }
            let gain = ui.add(egui::Slider::new(&mut self.gain, 0.05..=16.0).logarithmic(true).text("FFT gain"));
            spot(actions, learn, &gain, Control::Gain);
            let db = |sample: f32| 20.0 * sample.max(1e-4).log10();
            let place = |sample: f32| ((db(sample) + 48.0) / 48.0).clamp(0.0, 1.0);
            let clipping = self.clipped.is_some_and(|at| at.elapsed() < Duration::from_secs(2));
            let color = if clipping {
                egui::Color32::from_rgb(235, 40, 40)
            } else if db(self.held.0) < -18.0 {
                egui::Color32::from_rgb(70, 120, 240)
            } else if db(self.held.0) < -6.0 {
                GREEN
            } else {
                egui::Color32::from_rgb(240, 200, 60)
            };
            ui.add(
                egui::ProgressBar::new(place(self.level))
                    .desired_width(150.0)
                    .fill(color)
                    .text(format!("{:.0} dB{}", db(self.held.0), if clipping { " CLIP" } else { "" })),
            );
            ui.separator();
            let tracked = if self.heard.beat_period > 0.0 { 60.0 / self.heard.beat_period } else { 0.0 };
            ui.label(format!(
                "Tempo {:.1}  sure {:.0}%  {}",
                tracked,
                self.heard.beat_sure * 100.0,
                if self.predictor.bpm() > 0.0 { "predicting" } else { "following" }
            ));
            let lead = ui.add(egui::Slider::new(&mut self.lead_ms, 0.0..=250.0).suffix(" ms").text("beat lead"));
            spot(actions, learn, &lead, Control::Lead);
            ui.separator();
            ui.checkbox(&mut self.output_on, "Project on");
            let label = self.displays.get(self.output_display).map_or("no monitor", |display| display.label.as_str());
            egui::ComboBox::from_id_salt("monitor").selected_text(label).show_ui(ui, |ui| {
                for (index, display) in self.displays.iter().enumerate() {
                    ui.selectable_value(&mut self.output_display, index, &display.label);
                }
            });
            let mut size = self.size;
            egui::ComboBox::from_id_salt("size").selected_text(format!("{}p", RENDER_SIZES[size].1)).show_ui(ui, |ui| {
                for (index, (_, height)) in RENDER_SIZES.iter().enumerate() {
                    ui.selectable_value(&mut size, index, format!("{height}p"));
                }
            });
            if size != self.size {
                if let Err(error) = self.resize(size) {
                    self.message = Some(format!("{error:#}"));
                }
            }
            ui.label(format!("{:.0} fps", self.fps));
            ui.separator();
            if ui.toggle_value(&mut self.learn, "MIDI learn").changed() {
                self.learn_target = None;
            }
            ui.menu_button("MIDI", |ui| {
                ui.strong("In");
                for name in &self.midi.names {
                    ui.label(name);
                }
                ui.strong("Out (lights)");
                for name in &self.midi.output_names {
                    ui.label(name);
                }
                if self.midi.names.is_empty() && self.midi.output_names.is_empty() {
                    ui.weak("no device; connect one and restart");
                }
                ui.separator();
                ui.strong("Light values");
                for (index, label) in ["empty cell", "filled cell", "playing cell", "button on", "button off"].into_iter().enumerate() {
                    ui.horizontal(|ui| {
                        if ui.add(egui::DragValue::new(&mut self.led[index]).range(0..=127)).changed() {
                            self.lights.clear();
                        }
                        ui.label(label);
                    });
                }
                if ui.button("Reset to this controller's colours").clicked() {
                    self.led = led_defaults(&self.midi);
                    self.lights.clear();
                }
                ui.separator();
                if ui.add_enabled(self.midi.has(apc::NAME), egui::Checkbox::new(&mut self.apc, "APC Key 25 layout")).changed() {
                    self.lights.clear();
                }
                if self.apc {
                    ui.weak(format!(
                        "pads show columns {}-{}, layers {}-{}; arrows move them",
                        self.pads.0 + 1,
                        self.pads.0 + apc::COLUMNS,
                        self.pads.1 + 1,
                        self.pads.1 + apc::ROWS
                    ));
                }
                ui.separator();
                if ui.button("Send all lights again").clicked() {
                    self.lights.clear();
                }
                if ui.button(format!("Forget all {} bindings", self.bindings.len())).clicked() {
                    self.bindings.clear();
                    self.lights.clear();
                }
            });
            let devices = match self.midi.names.len() {
                0 => "no MIDI device".to_owned(),
                1 => self.midi.names[0].clone(),
                count => format!("{count} MIDI devices"),
            };
            ui.weak(format!("{devices}{}", self.midi_last.as_ref().map(|last| format!("  ·  {last}")).unwrap_or_default()));
        });
        if self.learn {
            ui.colored_label(
                egui::Color32::from_rgb(240, 200, 60),
                match self.learn_target {
                    Some(_) => "Now move the knob or press the pad for it.",
                    None => "Click a framed control or cell, then move a knob or press a pad. Right-click a blue one to forget it.",
                },
            );
        }
        if let Some(error) = &self.audio_error {
            ui.colored_label(RED, format!("Audio: {error}"));
        }
    }
    /// The library entries the current folder and search show, in display order.
    fn shown(&self) -> Vec<usize> {
        let needle = self.filter.to_lowercase();
        let pool: Vec<usize> = match self.view {
            View::All => (0..self.library.len()).collect(),
            View::PiBank => self.bank.clone(),
            View::Folder(folder) => self.folders[folder].ids.iter().filter_map(|id| self.index_of.get(id).copied()).collect(),
        };
        pool.into_iter()
            .filter(|index| {
                let entry = &self.library[*index];
                (self.show_unrunnable || entry.runnable())
                    && self.in_group(*index)
                    && (needle.is_empty()
                        || entry.name.to_lowercase().contains(&needle)
                        || entry.author.to_lowercase().contains(&needle)
                        || entry.id.to_lowercase().contains(&needle))
            })
            .collect()
    }
    fn library_panel(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.tab, 0, egui::RichText::new("Shaders").heading());
            ui.selectable_value(&mut self.tab, 1, egui::RichText::new("Effects").heading());
            ui.selectable_value(&mut self.tab, 2, egui::RichText::new("Assets").heading());
        });
        if self.tab == 1 {
            self.effects_panel(ui, actions);
            return;
        }
        if self.tab == 2 {
            self.assets_panel(ui, actions);
            return;
        }
        // Folders: pick one to look into, or drop shaders on one to add them.
        let runnable = self.library.iter().filter(|entry| entry.runnable()).count();
        let mut rows = vec![(View::All, format!("All shaders ({runnable})")), (View::PiBank, format!("On the Pi ({})", self.bank.len()))];
        rows.extend(self.folders.iter().enumerate().map(|(index, folder)| (View::Folder(index), format!("{} ({})", folder.name, folder.ids.len()))));
        for (view, label) in rows {
            let response = ui.selectable_label(self.view == view, label);
            if response.clicked() {
                self.view = view;
                self.selection.clear();
            }
            if let View::Folder(folder) = view {
                if let Some(dropped) = dropped::<Dragged>(&response) {
                    let entries = dropped.0.iter().filter_map(|source| match source {
                        Source::Shader(entry) => Some(*entry),
                        Source::Clip(_) => None,
                    });
                    actions.push(Action::AddToFolder(folder, entries.collect()));
                }
                if response.dnd_hover_payload::<Dragged>().is_some() {
                    ui.painter().rect_stroke(response.rect, 3.0, (2.0, GREEN), egui::StrokeKind::Inside);
                }
                response.context_menu(|ui| {
                    if ui.button("Delete folder").clicked() {
                        actions.push(Action::DeleteFolder(folder));
                        ui.close_menu();
                    }
                });
            }
        }
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.new_folder).hint_text("new folder name").desired_width(170.0));
            if ui.button("Add folder").clicked() && !self.new_folder.trim().is_empty() {
                self.folders.push(Folder { name: self.new_folder.trim().to_owned(), ids: Vec::new() });
                self.new_folder.clear();
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("search name, author or id").desired_width(200.0));
            ui.checkbox(&mut self.show_unrunnable, "multipass");
        });
        // Groups worked out from tags, code and author; favourites and removed ones too.
        ui.horizontal(|ui| {
            let count = |members: usize| format!("({members})");
            let name = |group: usize| match group {
                0 => "All groups".to_owned(),
                1 => format!("Favourites {}", count(self.favorites.len())),
                2 => format!("Removed {}", count(self.removed.len())),
                3 => format!("New, to review {}", count(self.review.len())),
                group => format!("{} {}", self.groups[group - 4].0, count(self.groups[group - 4].1.len())),
            };
            let mut group = self.group;
            egui::ComboBox::from_id_salt("group").width(190.0).height(500.0).selected_text(name(group)).show_ui(ui, |ui| {
                for choice in 0..self.groups.len() + 4 {
                    ui.selectable_value(&mut group, choice, name(choice));
                }
            });
            self.group = group;
            match self.thumb_run {
                Some(at) => {
                    ui.weak(format!("pictures {at} of {}", self.library.len()));
                }
                None => {
                    if ui.button("Make all pictures").on_hover_text("Draw every shader's hover picture now, once").clicked() {
                        self.thumb_run = Some(0);
                    }
                }
            }
            ui.toggle_value(&mut self.download_open, "Download…").on_hover_text("Get shaders from Shadertoy by link or id");
        });
        let shown = self.shown();
        ui.weak(format!("{} shown, {} selected. Drag to a layer or a folder; double-click to preview.", shown.len(), self.selection.len()));
        // The checked shaders play by themselves, on a layer of their own above the composition.
        ui.horizontal(|ui| {
            let mut all = !shown.is_empty() && shown.iter().all(|entry| self.checked.contains(entry));
            if ui.checkbox(&mut all, format!("all  ({} checked)", self.checked.len())).changed() {
                for entry in &shown {
                    if all && self.library[*entry].runnable() {
                        self.checked.insert(*entry);
                    } else {
                        self.checked.remove(entry);
                    }
                }
            }
            let learn = self.learn;
            let next = ui.add_enabled(self.autoplay.is_some(), egui::Button::new("Next"));
            spot(actions, learn, &next, Control::AutoNext);
            if next.clicked() && !learn {
                self.autoplay_next();
            }
            ui.checkbox(&mut self.auto_shuffle, "shuffle");
        });
        ui.horizontal(|ui| {
            // Play: the ticked shaders one after another, with the mode and time beside it.
            let learn = self.learn;
            let playing = self.autoplay.is_some();
            let toggle = ui
                .add_enabled(playing || !self.checked.is_empty(), egui::Button::new(if playing { "⏹" } else { "▶" }))
                .on_hover_text(if playing { "Stop the autoplay" } else { "Play the ticked shaders one after another" });
            spot(actions, learn, &toggle, Control::AutoToggle);
            if toggle.clicked() && !learn {
                if playing {
                    self.stop_autoplay();
                } else {
                    self.start_autoplay();
                }
            }
            egui::ComboBox::from_id_salt("auto-react").width(66.0).selected_text(REACT_MODES[usize::from(self.auto.react)]).show_ui(ui, |ui| {
                for (mode, name) in REACT_MODES.iter().enumerate() {
                    ui.selectable_value(&mut self.auto.react, mode as u8, *name);
                }
            });
            ui.add(egui::DragValue::new(&mut self.auto_seconds).range(1.0..=36000.0).suffix(" s each"));
            ui.checkbox(&mut self.auto_sections, "and on song sections");
        });
        let playing = self.autoplay.as_ref().map(|play| play.order[play.at]).filter(|_| self.auto.active.is_some());
        if let Some(play) = &self.autoplay {
            ui.horizontal(|ui| {
                ui.spacing_mut().slider_width = 70.0;
                let opacity = ui.add(egui::Slider::new(&mut self.auto.opacity, 0.0..=1.0).show_value(false)).on_hover_text("Opacity");
                spot(actions, self.learn, &opacity, Control::AutoOpacity);
                egui::ComboBox::from_id_salt("auto-blend").width(84.0).selected_text(gfx::BLENDS[self.auto.blend]).show_ui(ui, |ui| {
                    for (mode, name) in gfx::BLENDS.iter().enumerate() {
                        ui.selectable_value(&mut self.auto.blend, mode, *name);
                    }
                });
                ui.colored_label(GREEN, format!("{} of {}", play.at + 1, play.order.len()));
            });
        }
        // Presets: the checked list and how it plays, kept under a name and played on recall.
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.preset_name).hint_text("preset name").desired_width(96.0));
            if ui.add_enabled(!self.preset_name.trim().is_empty(), egui::Button::new("Save preset")).clicked() {
                self.save_preset();
            }
            ui.add_enabled_ui(!self.presets.is_empty(), |ui| {
                ui.menu_button("Load preset", |ui| {
                    for (index, preset) in self.presets.iter().enumerate() {
                        if ui.button(format!("{}  ({})", preset.name, preset.ids.len())).clicked() {
                            actions.push(Action::LoadPreset(index));
                            ui.close_menu();
                        }
                    }
                    ui.separator();
                    ui.menu_button("Delete", |ui| {
                        for (index, preset) in self.presets.iter().enumerate() {
                            if ui.button(&preset.name).clicked() {
                                actions.push(Action::DeletePreset(index));
                                ui.close_menu();
                            }
                        }
                    });
                });
            });
        });
        let mut toggled = Vec::new();
        let mut wanted = None;
        let modifiers = ui.input(|input| input.modifiers);
        let selection = self.selection.clone();
        // What a row asked for: (place in `shown`, library index, whether it was a click).
        let mut asked = None;
        let row = ui.spacing().interact_size.y.max(ui.text_style_height(&egui::TextStyle::Body) + 4.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, row, shown.len(), |ui, rows| {
            // One line each: the list is drawn by row height.
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            for place in rows {
              ui.horizontal(|ui| {
                let index = shown[place];
                let entry = &self.library[index];
                let mut check = self.checked.contains(&index);
                if ui.add_enabled(entry.runnable(), egui::Checkbox::without_text(&mut check)).changed() {
                    toggled.push(index);
                }
                let star = if self.favorites.contains(&index) { "★ " } else { "" };
                let mut text = egui::RichText::new(if entry.author.is_empty() {
                    format!("{star}{}", entry.title())
                } else {
                    format!("{star}{}  ·  {}", entry.title(), entry.author)
                });
                if !entry.runnable() {
                    text = text.weak();
                }
                if playing == Some(index) {
                    text = text.color(GREEN).strong();
                }
                let selected = selection.contains(&index);
                let response = ui
                    .add_enabled(entry.runnable(), egui::SelectableLabel::new(selected, text))
                    .interact(egui::Sense::click_and_drag());
                // Hovering shows the shader's picture, drawn the first time it is asked for.
                if response.hovered() && entry.runnable() && !self.thumbs.contains_key(&index) && !self.thumb_failed.contains(&index) {
                    wanted = Some(index);
                }
                let response = response.on_hover_ui(|ui| match self.thumbs.get(&index) {
                    Some(picture) => {
                        ui.image(egui::load::SizedTexture::new(picture.id(), egui::vec2(PREVIEW.0 as f32, PREVIEW.1 as f32)));
                    }
                    None if self.thumb_failed.contains(&index) => {
                        ui.label("no picture: this shader does not compile");
                    }
                    None => {
                        ui.label("drawing its picture…");
                    }
                });
                if response.double_clicked() {
                    actions.push(Action::Preview(index));
                } else if response.clicked() {
                    asked = Some((place, index, true));
                } else if response.drag_started() && !selected {
                    asked = Some((place, index, false));
                }
                if place == 0 {
                    self.seen_row.set(Some(response.rect));
                    if self.options.drag_test && (104..=130).contains(&self.frames) {
                        self.drag_notes.borrow_mut().push(format!(
                            "f{} row dragged={} started={} down_on={} hovered={}",
                            self.frames,
                            response.dragged(),
                            response.drag_started(),
                            response.is_pointer_button_down_on(),
                            response.hovered()
                        ));
                    }
                }
                if response.dragged() {
                    // A selected row carries the selection; a ticked row, everything ticked.
                    let carried: Vec<Source> = if selected && selection.len() > 1 {
                        shown.iter().filter(|entry| selection.contains(entry)).map(|entry| Source::Shader(*entry)).collect()
                    } else if self.checked.contains(&index) {
                        shown.iter().filter(|entry| self.checked.contains(entry)).map(|entry| Source::Shader(*entry)).collect()
                    } else {
                        vec![Source::Shader(index)]
                    };
                    response.dnd_set_drag_payload(Dragged(carried));
                }
                response.context_menu(|ui| {
                    let picked: Vec<usize> = if selected { selection.iter().copied().collect() } else { vec![index] };
                    if ui.button("Preview").clicked() {
                        actions.push(Action::Preview(index));
                        ui.close_menu();
                    }
                    if ui.button("Send to the selected layer").clicked() {
                        actions.push(Action::Send(index));
                        ui.close_menu();
                    }
                    let starred = self.favorites.contains(&index);
                    if ui.button(if starred { "Remove from favourites" } else { "Favourite" }).clicked() {
                        actions.push(Action::Favorite(picked.clone(), !starred));
                        ui.close_menu();
                    }
                    if self.review.contains(&index) && ui.button("Mark as reviewed").clicked() {
                        actions.push(Action::Reviewed(picked.clone()));
                        ui.close_menu();
                    }
                    let gone = self.removed.contains(&index);
                    if ui.button(if gone { "Restore to the library" } else { "Remove from the library" }).clicked() {
                        actions.push(Action::Remove(picked.clone(), !gone));
                        ui.close_menu();
                    }
                    ui.separator();
                    ui.menu_button("Add to folder", |ui| {
                        for (folder, known) in self.folders.iter().enumerate() {
                            if ui.button(&known.name).clicked() {
                                actions.push(Action::AddToFolder(folder, picked.clone()));
                                ui.close_menu();
                            }
                        }
                        if self.folders.is_empty() {
                            ui.weak("no folders yet");
                        }
                    });
                    if let View::Folder(folder) = self.view {
                        if ui.button("Remove from this folder").clicked() {
                            actions.push(Action::RemoveFromFolder(folder, picked.clone()));
                            ui.close_menu();
                        }
                    }
                });
              });
            }
        });
        for index in toggled {
            if !self.checked.remove(&index) {
                self.checked.insert(index);
            }
        }
        if wanted.is_some() {
            self.thumb_wanted = wanted;
        }
        if let Some((place, index, clicked)) = asked {
            let anchor = self.anchor.and_then(|anchor| shown.iter().position(|entry| *entry == anchor));
            match (clicked, modifiers.shift, modifiers.command, anchor) {
                (true, true, _, Some(anchor)) => {
                    let (from, to) = (anchor.min(place), anchor.max(place));
                    self.selection.extend(shown[from..=to].iter().copied());
                }
                (true, _, true, _) => {
                    if !self.selection.remove(&index) {
                        self.selection.insert(index);
                    }
                    self.anchor = Some(index);
                }
                _ => {
                    self.selection = BTreeSet::from([index]);
                    self.anchor = Some(index);
                }
            }
        }
    }
    /// The Effects tab: drag one onto a layer or onto the composition's box on the right,
    /// or double-click to put it on the selected layer.
    fn effects_panel(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.add(egui::TextEdit::singleline(&mut self.fx_filter).hint_text("search effects").desired_width(200.0));
        let aim = match self.focus_cells().len() {
            0 => format!("the layer {}", self.layers[self.selected_layer.min(self.layers.len() - 1)].name),
            1 => "the highlighted cell".to_owned(),
            count => format!("the {count} highlighted cells"),
        };
        ui.weak(format!("Double-click adds to {aim}. Click a layer's name to aim at the layer, a cell to aim at cells. Or drag onto a cell, a layer's controls, or the composition box on the right."));
        let needle = self.fx_filter.to_lowercase();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for heading in ["Colour", "Distort", "Stylize", "Audio"] {
                let listed: Vec<usize> = (1..gfx::EFFECTS.len())
                    .filter(|kind| gfx::EFFECTS[*kind].1 == heading && gfx::EFFECTS[*kind].0.to_lowercase().contains(&needle))
                    .collect();
                if listed.is_empty() {
                    continue;
                }
                ui.add_space(4.0);
                ui.strong(heading);
                for kind in listed {
                    let response = ui.selectable_label(false, gfx::EFFECTS[kind].0).interact(egui::Sense::click_and_drag());
                    if response.double_clicked() {
                        actions.push(Action::AddEffectToFocus(kind));
                    }
                    if response.dragged() {
                        response.dnd_set_drag_payload(DraggedEffect(kind));
                    }
                }
            }
        });
    }
    /// One effect chain's rows: on or off, name, amount, order, remove; and a box to drop on.
    fn chain_rows(&mut self, ui: &mut egui::Ui, target: ChainAt, title: String, actions: &mut Vec<Action>) {
        ui.separator();
        let zone = ui
            .group(|ui| {
                ui.set_width(ui.available_width());
                ui.strong(title);
                let Some(chain) = self.chain(target) else { return };
                if chain.is_empty() {
                    ui.weak("no effects; drop one here");
                }
                let last = chain.len().saturating_sub(1);
                for (at, effect) in chain.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().slider_width = 80.0;
                        let mut on = !effect.bypass;
                        ui.checkbox(&mut on, "").on_hover_text("On");
                        effect.bypass = !on;
                        ui.add_sized([70.0, 18.0], egui::Label::new(gfx::EFFECTS[effect.kind].0).truncate());
                        ui.add(egui::Slider::new(&mut effect.amount, 0.0..=1.0).show_value(false));
                        if ui.add_enabled(at > 0, egui::Button::new("up").small()).clicked() {
                            actions.push(Action::MoveEffect(target, at, true));
                        }
                        if ui.add_enabled(at < last, egui::Button::new("dn").small()).clicked() {
                            actions.push(Action::MoveEffect(target, at, false));
                        }
                        if ui.small_button("x").clicked() {
                            actions.push(Action::RemoveEffect(target, at));
                        }
                    });
                }
            })
            .response;
        if let Some(dropped) = dropped::<DraggedEffect>(&zone) {
            actions.push(Action::AddEffect(target, dropped.0));
        }
        if zone.dnd_hover_payload::<DraggedEffect>().is_some() {
            ui.painter().rect_stroke(zone.rect, 3.0, (2.0, GREEN), egui::StrokeKind::Inside);
        }
    }
    fn layer_header(&mut self, ui: &mut egui::Ui, index: usize, actions: &mut Vec<Action>) {
        let selected = self.selected_layer == index;
        let learn = self.learn;
        let mut select = false;
        let layer = &mut self.layers[index];
        let header = ui.allocate_ui_with_layout(egui::vec2(HEADER_WIDTH, CELL.y), egui::Layout::top_down(egui::Align::LEFT), |ui| {
            ui.set_min_size(egui::vec2(HEADER_WIDTH, CELL.y));
            ui.spacing_mut().slider_width = 74.0;
            ui.spacing_mut().item_spacing.y = 3.0;
            ui.horizontal(|ui| {
                if ui.selectable_label(selected, egui::RichText::new(&layer.name).strong()).clicked() {
                    select = true;
                }
                let eject = ui.small_button("Eject");
                spot(actions, learn, &eject, Control::Eject(index));
                if eject.clicked() && !learn {
                    actions.push(Action::Eject(index));
                }
                ui.checkbox(&mut layer.bypass, "Bypass");
            });
            ui.horizontal(|ui| {
                let opacity = ui.add(egui::Slider::new(&mut layer.opacity, 0.0..=1.0).show_value(false)).on_hover_text("Opacity");
                spot(actions, learn, &opacity, Control::Opacity(index));
                egui::ComboBox::from_id_salt(("blend", index)).width(84.0).selected_text(gfx::BLENDS[layer.blend]).show_ui(ui, |ui| {
                    for (mode, name) in gfx::BLENDS.iter().enumerate() {
                        ui.selectable_value(&mut layer.blend, mode, *name);
                    }
                });
                egui::ComboBox::from_id_salt(("react", index)).width(62.0).selected_text(REACT_MODES[usize::from(layer.react)]).show_ui(ui, |ui| {
                    for (mode, name) in REACT_MODES.iter().enumerate() {
                        ui.selectable_value(&mut layer.react, mode as u8, *name);
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                if let Some(first) = layer.effects.first_mut() {
                    let amount = ui
                        .add(egui::Slider::new(&mut first.amount, 0.0..=1.0).show_value(false))
                        .on_hover_text("Amount of the first effect");
                    spot(actions, learn, &amount, Control::FxAmount(index));
                }
                let names: Vec<&str> = layer.effects.iter().map(|effect| gfx::EFFECTS[effect.kind].0).collect();
                if names.is_empty() {
                    ui.weak("no effects; drop one here");
                } else {
                    ui.label(format!("FX: {}", names.join(", ")));
                }
            });
            ui.horizontal(|ui| {
                // Play: this layer's cells one after another, from the first if none is live.
                let running = layer.active.is_some();
                let play = ui
                    .small_button(if running { "⏹" } else { "▶" })
                    .on_hover_text(if running { "Stop this layer: nothing of it stays in the picture" } else { "Play this layer's cells one after another" });
                if play.clicked() {
                    if running {
                        // Stopped means off, not frozen on its last cell under the other layers.
                        actions.push(Action::Eject(index));
                    } else {
                        if layer.auto == 0 {
                            layer.auto = 1;
                        }
                        if layer.active.is_none() {
                            if let Some(first) = layer.cells.iter().position(Option::is_some) {
                                actions.push(Action::Launch(index, first));
                            }
                        }
                    }
                }
                let before = layer.auto;
                egui::ComboBox::from_id_salt(("auto", index)).width(84.0).selected_text(AUTO_MODES[layer.auto]).show_ui(ui, |ui| {
                    for (mode, name) in AUTO_MODES.iter().enumerate() {
                        ui.selectable_value(&mut layer.auto, mode, *name);
                    }
                });
                ui.checkbox(&mut layer.on_sections, "on sections");
                // Switched on with nothing playing: start at the first cell.
                if before == 0 && layer.auto != 0 && layer.active.is_none() {
                    if let Some(first) = layer.cells.iter().position(Option::is_some) {
                        actions.push(Action::Launch(index, first));
                    }
                }
            });
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt(("transition", index)).width(104.0).selected_text(gfx::TRANSITIONS[layer.transition]).show_ui(ui, |ui| {
                    for (kind, name) in gfx::TRANSITIONS.iter().enumerate() {
                        ui.selectable_value(&mut layer.transition, kind, *name);
                    }
                });
                ui.add_enabled(layer.transition > 0, egui::DragValue::new(&mut layer.transition_seconds).range(0.05..=30.0).speed(0.05).suffix(" s"))
                    .on_hover_text("How long the change from one cell to the next takes");
            });
        })
        .response
        .interact(egui::Sense::click());
        if let Some(dropped) = dropped::<DraggedEffect>(&header) {
            actions.push(Action::AddEffect(ChainAt::Layer(index), dropped.0));
        }
        if let Some(dropped) = dropped::<Dragged>(&header) {
            let free = self.layers[index].cells.iter().position(Option::is_none).unwrap_or(self.columns);
            actions.push(Action::Place(index, free, dropped.0.clone()));
        }
        if header.dnd_hover_payload::<Dragged>().is_some() {
            ui.painter().rect_stroke(header.rect, 3.0, (2.0, egui::Color32::from_rgb(240, 200, 60)), egui::StrokeKind::Inside);
        }
        if header.dnd_hover_payload::<DraggedEffect>().is_some() {
            ui.painter().rect_stroke(header.rect, 3.0, (2.0, GREEN), egui::StrokeKind::Inside);
        }
        header.context_menu(|ui| {
            if ui.button("Clear layer").clicked() {
                actions.push(Action::ClearLayer(index));
                ui.close_menu();
            }
            if ui.button("Remove layer").clicked() {
                actions.push(Action::RemoveLayer(index));
                ui.close_menu();
            }
        });
        if select {
            self.selected_layer = index;
            self.focus_layer = true;
            self.picked.clear();
        }
    }
    fn cell(&self, ui: &mut egui::Ui, layer_index: usize, column: usize, actions: &mut Vec<Action>) {
        let (rect, response) = ui.allocate_exact_size(CELL, egui::Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        let layer = &self.layers[layer_index];
        let cell = layer.cells[column].as_ref();
        painter.rect_filled(rect, 3.0, egui::Color32::from_gray(if cell.is_some() { 40 } else { 24 }));
        if let Some(cell) = cell {
            let picture = egui::Rect::from_min_size(rect.min, egui::vec2(CELL.x, CELL_PICTURE));
            let note = |text: &str, fill: egui::Color32| {
                painter.rect_filled(picture, 3.0, fill);
                painter.text(picture.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(13.0), egui::Color32::from_gray(190));
            };
            match (&cell.source, &cell.thumb, &cell.error) {
                (_, Some((_, texture)), _) => {
                    painter.image(*texture, picture, FLIPPED, egui::Color32::WHITE);
                }
                (_, _, Some(_)) => note("ERROR", egui::Color32::from_rgb(90, 25, 25)),
                (Source::Clip(path), _, _) => match self.asset_thumbs.get(path) {
                    Some(Some(texture)) => {
                        // The clip's own picture, fitted into the cell whatever its shape.
                        painter.rect_filled(picture, 3.0, egui::Color32::BLACK);
                        let size = texture.size_vec2();
                        let fit = (picture.width() / size.x).min(picture.height() / size.y);
                        let shown = egui::Rect::from_center_size(picture.center(), size * fit);
                        painter.image(texture.id(), shown, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                    }
                    Some(None) => note("CLIP", egui::Color32::from_rgb(35, 55, 90)),
                    None => {
                        self.clip_thumbs_wanted.borrow_mut().push(path.clone());
                        note("CLIP", egui::Color32::from_rgb(35, 55, 90));
                    }
                },
                _ => note("…", egui::Color32::from_gray(30)),
            }
            painter.text(
                rect.left_bottom() + egui::vec2(4.0, -3.0),
                egui::Align2::LEFT_BOTTOM,
                self.source_name(&cell.source),
                egui::FontId::proportional(11.0),
                egui::Color32::from_gray(220),
            );
            let tag = |at: egui::Pos2, align: egui::Align2, text: String| {
                let size = painter.text(at, align, &text, egui::FontId::proportional(10.0), egui::Color32::TRANSPARENT);
                painter.rect_filled(size.expand(2.0), 2.0, egui::Color32::from_black_alpha(170));
                painter.text(at, align, text, egui::FontId::proportional(10.0), egui::Color32::WHITE);
            };
            tag(rect.right_top() + egui::vec2(-5.0, 4.0), egui::Align2::RIGHT_TOP, format!("{:.0} s", cell.seconds));
            if !cell.effects.is_empty() {
                tag(rect.left_top() + egui::vec2(5.0, 20.0), egui::Align2::LEFT_TOP, format!("FX {}", cell.effects.len()));
            }
            if let Some(react) = cell.react {
                tag(rect.left_top() + egui::vec2(5.0, 4.0), egui::Align2::LEFT_TOP, REACT_MODES[usize::from(react)].to_owned());
            }
            if layer.active == Some(column) && layer.auto != 0 {
                // How much of its time the live cell has had.
                let had = (layer.launched.elapsed().as_secs_f32() / cell.seconds).clamp(0.0, 1.0);
                let bar = egui::Rect::from_min_size(picture.left_bottom() - egui::vec2(0.0, 3.0), egui::vec2(CELL.x * had, 3.0));
                painter.rect_filled(bar, 0.0, GREEN);
            }
        }
        if let Some(dropped) = dropped::<DraggedEffect>(&response) {
            actions.push(Action::AddEffect(ChainAt::Cell(layer_index, column), dropped.0));
        }
        if (layer_index, column) == (1, 0) {
            self.seen_cell.set(Some(rect));
            if self.options.drag_test && (120..=130).contains(&self.frames) {
                self.drag_notes.borrow_mut().push(format!(
                    "f{} cell contains={} payload={} released={}",
                    self.frames,
                    response.contains_pointer(),
                    egui::DragAndDrop::has_payload_of_type::<Dragged>(ui.ctx()),
                    ui.input(|input| input.pointer.any_released())
                ));
            }
        }
        let picked = self.picked.contains(&(layer_index, column));
        if let Some(dropped) = dropped::<DraggedCells>(&response) {
            actions.push(Action::MoveCells(dropped.0.clone(), layer_index, column));
        }
        let carrying = response.dnd_hover_payload::<Dragged>().is_some()
            || response.dnd_hover_payload::<DraggedCells>().is_some()
            || response.dnd_hover_payload::<DraggedEffect>().is_some();
        let stroke = if carrying {
            (2.0, egui::Color32::from_rgb(240, 200, 60))
        } else if cell.is_some() && layer.active == Some(column) {
            (3.0, GREEN)
        } else {
            (1.0, egui::Color32::from_gray(if response.hovered() { 110 } else { 60 }))
        };
        painter.rect_stroke(rect, 3.0, stroke, egui::StrokeKind::Inside);
        if picked {
            painter.rect_stroke(rect.shrink(3.0), 2.0, (2.0, egui::Color32::from_rgb(70, 140, 255)), egui::StrokeKind::Inside);
        }
        if let Some(dropped) = dropped::<Dragged>(&response) {
            actions.push(Action::Place(layer_index, column, dropped.0.clone()));
        } else if self.learn {
            spot(actions, true, &response, Control::Cell(layer_index, column));
            return;
        } else if response.double_clicked() && cell.is_some() {
            // A double-click on a shader opens its editor under the preview.
            if let Some(Source::Shader(entry)) = cell.map(|cell| &cell.source) {
                actions.push(Action::Edit(*entry));
            }
        } else if response.clicked() && cell.is_some() {
            // Ctrl adds to or takes from the selection, Shift selects a run; a plain
            // click launches.
            let modifiers = ui.input(|input| input.modifiers);
            if modifiers.command || modifiers.shift {
                actions.push(Action::PickCell(layer_index, column, modifiers.shift));
            } else {
                actions.push(Action::Launch(layer_index, column));
            }
        }
        if response.dragged() && cell.is_some() {
            // Dragging one of the selected cells carries them all.
            let carried = if picked { self.picked.iter().copied().collect() } else { vec![(layer_index, column)] };
            response.dnd_set_drag_payload(DraggedCells(carried));
        }
        if let Some(error) = cell.and_then(|cell| cell.error.as_ref()) {
            response.clone().on_hover_text(error);
        }
        if let Some(cell) = cell {
            response.context_menu(|ui| {
                if picked && self.picked.len() > 1 {
                    if ui.button(format!("Clear the {} selected cells", self.picked.len())).clicked() {
                        actions.push(Action::ClearCells(self.picked.iter().copied().collect()));
                        ui.close_menu();
                    }
                    ui.separator();
                }
                let mut seconds = cell.seconds;
                ui.horizontal(|ui| {
                    ui.label("Plays for");
                    if ui.add(egui::DragValue::new(&mut seconds).range(1.0..=36000.0).suffix(" s")).changed() {
                        actions.push(Action::SetSeconds(layer_index, column, seconds));
                    }
                });
                ui.menu_button("React mode", |ui| {
                    if ui.selectable_label(cell.react.is_none(), "as the layer").clicked() {
                        actions.push(Action::SetReact(layer_index, column, None));
                        ui.close_menu();
                    }
                    for (mode, name) in REACT_MODES.iter().enumerate() {
                        if ui.selectable_label(cell.react == Some(mode as u8), *name).clicked() {
                            actions.push(Action::SetReact(layer_index, column, Some(mode as u8)));
                            ui.close_menu();
                        }
                    }
                });
                ui.separator();
                if ui.button("Clear cell").clicked() {
                    actions.push(Action::ClearCell(layer_index, column));
                    ui.close_menu();
                }
            });
        }
    }
    fn grid(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        if let Some(run) = &self.compile {
            ui.add(egui::ProgressBar::new(run.next as f32 / self.library.len().max(1) as f32).text("compiling the library"));
        }
        ui.horizontal(|ui| {
            ui.heading("Composition");
            if ui.button("Add layer").clicked() {
                actions.push(Action::AddLayer);
            }
            if ui.button("Add column").clicked() {
                actions.push(Action::AddColumn);
            }
            if ui.button("Add clips to selected layer…").clicked() {
                let picked = rfd::FileDialog::new().add_filter("Video", &CLIP_TYPES).pick_files().unwrap_or_default();
                if !picked.is_empty() {
                    let layer = self.selected_layer;
                    let column = self.layers[layer].cells.iter().position(Option::is_none).unwrap_or(self.columns);
                    actions.push(Action::Place(layer, column, picked.into_iter().map(Source::Clip).collect()));
                }
            }
            if ui.button("Open…").clicked() {
                let picked = rfd::FileDialog::new().add_filter("Composition", &["json"]).pick_file();
                let read = picked.and_then(|path| std::fs::read_to_string(path).ok());
                match read.and_then(|text| serde_json::from_str(&text).ok()) {
                    Some(saved) => actions.push(Action::OpenComposition(saved)),
                    None => {}
                }
            }
            if ui.button("Save as…").clicked() {
                let picked = rfd::FileDialog::new().add_filter("Composition", &["json"]).set_file_name("composition.json").save_file();
                if let Some(path) = picked {
                    let state = self.state();
                    let composition = serde_json::json!({ "version": 1, "columns": state["columns"], "layers": state["layers"] });
                    if let Err(error) = std::fs::write(&path, serde_json::to_vec_pretty(&composition).unwrap_or_default()) {
                        self.message = Some(format!("could not save {}: {error}", path.display()));
                    }
                }
            }
            if ui.add_enabled(!self.undo.is_empty(), egui::Button::new("Undo")).on_hover_text("Ctrl+Z").clicked() {
                actions.push(Action::Undo);
            }
            if ui.add_enabled(!self.redo.is_empty(), egui::Button::new("Redo")).on_hover_text("Ctrl+Y").clicked() {
                actions.push(Action::Redo);
            }
            ui.weak("Click launches. Ctrl- or Shift-click selects cells to drag or to clear (Delete).");
        });
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
            // Each row's cells scroll sideways in a strip of their own, and every strip is
            // held at the same place, so the layer controls on the left never move.
            let held = self.grid_x;
            let mut moved = held;
            let mut strip = |ui: &mut egui::Ui, salt: usize, bar: bool, add: &mut dyn FnMut(&mut egui::Ui)| {
                let shown = if bar { egui::scroll_area::ScrollBarVisibility::AlwaysVisible } else { egui::scroll_area::ScrollBarVisibility::AlwaysHidden };
                let out = egui::ScrollArea::horizontal()
                    .id_salt(("cells", salt))
                    .auto_shrink([false, true])
                    .scroll_bar_visibility(shown)
                    .scroll_offset(egui::vec2(held, 0.0))
                    .show(ui, |ui| add(ui));
                // The strip that was scrolled this frame leads the others.
                if (out.state.offset.x - held).abs() > 0.5 {
                    moved = out.state.offset.x;
                }
            };
            for index in (0..self.layers.len()).rev() {
                ui.horizontal_top(|ui| {
                    self.layer_header(ui, index, actions);
                    strip(ui, index, false, &mut |ui| {
                        ui.horizontal(|ui| {
                            for column in 0..self.columns {
                                self.cell(ui, index, column, actions);
                            }
                        });
                    });
                });
                if let Some(error) = &self.layers[index].error {
                    ui.colored_label(RED, format!("{}: {error}", self.layers[index].name));
                }
            }
            // One scroll bar for all of them, under the cells.
            ui.horizontal(|ui| {
                ui.add_space(HEADER_WIDTH + 4.0);
                let width = self.columns as f32 * (CELL.x + 4.0);
                strip(ui, usize::MAX, true, &mut |ui| {
                    ui.allocate_space(egui::vec2(width, 1.0));
                });
            });
            self.grid_x = moved;
        });
    }
    /// The picture the preview shows: a library shader being tried out, else the output.
    fn preview_picture(&self) -> Option<egui::TextureId> {
        if self.pre.active.is_some() { self.pre_texture } else { self.out_texture }
    }
    /// The preview in a window of its own; true when that window was closed.
    fn preview_window(&self, ctx: &egui::Context) -> bool {
        let builder = egui::ViewportBuilder::default().with_title("RPi-VJ preview").with_inner_size([640.0, 360.0]);
        ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("preview"), builder, |ctx, _| {
            egui::CentralPanel::default().frame(egui::Frame::new().fill(egui::Color32::BLACK)).show(ctx, |ui| {
                if let Some(texture) = self.preview_picture() {
                    let area = ui.max_rect();
                    let fit = (area.width() / 16.0).min(area.height() / 9.0);
                    let rect = egui::Rect::from_center_size(area.center(), egui::vec2(fit * 16.0, fit * 9.0));
                    ui.painter().image(texture, rect, FLIPPED, egui::Color32::WHITE);
                }
            });
            ctx.input(|input| input.viewport().close_requested())
        })
    }
    fn preview(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let trying = self.pre.active.and_then(|_| self.pre.cells[0].as_ref()).map(|cell| cell.source.clone());
        ui.horizontal(|ui| {
            ui.heading(if trying.is_some() { "Preview" } else { "Output" });
            if ui.button(if self.detached { "Attach" } else { "Detach" }).on_hover_text("Show the picture in a window of its own").clicked() {
                self.detached = !self.detached;
            }
        });
        if let Some(texture) = self.preview_picture().filter(|_| !self.detached) {
            let width = ui.available_width();
            let size = egui::vec2(width, width * 9.0 / 16.0);
            ui.add(egui::Image::new(egui::load::SizedTexture::new(texture, size)).uv(FLIPPED));
        }
        if let Some(Source::Shader(entry)) = trying {
            ui.horizontal(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                if ui.button("To selected layer").clicked() {
                    actions.push(Action::Send(entry));
                    self.close_preview();
                }
                if ui.button("Close").clicked() {
                    self.close_preview();
                }
                ui.label(self.library[entry].title());
            });
        }
        ui.add_space(6.0);
        for layer in [&self.auto].into_iter().filter(|_| self.autoplay.is_some()).chain(self.layers.iter().rev()) {
            let playing = layer.active.and_then(|column| layer.cells[column].as_ref()).map(|cell| self.source_name(&cell.source));
            ui.horizontal(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                ui.strong(&layer.name);
                match playing {
                    Some(name) => ui.label(name),
                    None => ui.weak("off"),
                };
            });
        }
        ui.add_space(6.0);
        ui.weak(format!(
            "sections {}   bass {:.2}  mid {:.2}  treble {:.2}",
            self.section_count, self.heard.bass, self.heard.mid, self.heard.treble
        ));
        if let Some(message) = &self.message {
            ui.colored_label(RED, message);
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            self.editor(ui, actions);
            let layer = self.selected_layer.min(self.layers.len().saturating_sub(1));
            let title = format!("{} effects", self.layers[layer].name);
            for (at, column) in self.focus_cells().into_iter().take(1) {
                let name = self.layers[at].cells[column].as_ref().map(|cell| self.source_name(&cell.source)).unwrap_or_default();
                self.chain_rows(ui, ChainAt::Cell(at, column), format!("Cell effects: {name}"), actions);
            }
            self.chain_rows(ui, ChainAt::Layer(layer), title, actions);
            self.chain_rows(ui, ChainAt::Composition, "Composition effects".into(), actions);
        });
    }
    fn project(&self, ctx: &egui::Context) {
        let (Some(texture), Some(display)) = (self.out_texture, self.displays.get(self.output_display)) else { return };
        let scale = ctx.native_pixels_per_point().unwrap_or(1.0).max(0.1);
        let position = egui::pos2(display.position.0 as f32 / scale, display.position.1 as f32 / scale);
        let size = egui::vec2(display.size.0 as f32 / scale, display.size.1 as f32 / scale);
        let builder = egui::ViewportBuilder::default()
            .with_title("RPi-VJ output")
            .with_decorations(false)
            .with_resizable(false)
            .with_position(position)
            .with_inner_size(size);
        ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("output"), builder, |ctx, _| {
            self.output_rect.set(ctx.input(|input| input.viewport().outer_rect));
            egui::CentralPanel::default().frame(egui::Frame::new().fill(egui::Color32::BLACK)).show(ctx, |ui| {
                // Fill the monitor, keeping the picture's shape.
                let area = ui.max_rect();
                let fit = (area.width() / 16.0).min(area.height() / 9.0);
                let rect = egui::Rect::from_center_size(area.center(), egui::vec2(fit * 16.0, fit * 9.0));
                ui.painter().image(texture, rect, FLIPPED, egui::Color32::WHITE);
            });
        });
        self.output_px.set(place_window("RPi-VJ output", display.position, display.size));
    }
    /// Clips dragged in from the file manager go to the selected layer's free cells.
    fn take_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|input| input.raw.dropped_files.iter().filter_map(|file| file.path.clone()).collect());
        let clips: Vec<Source> = dropped
            .into_iter()
            .filter(|path| is_media(path))
            .map(Source::Clip)
            .collect();
        if !clips.is_empty() {
            let layer = self.selected_layer;
            let column = self.layers[layer].cells.iter().position(Option::is_none).unwrap_or(self.columns);
            self.place(layer, column, clips);
        }
    }
    fn tests(&mut self, ctx: &egui::Context) {
        if let Some(path) = self.options.snapshot.clone().filter(|_| self.frames >= self.options.frames) {
            let (width, height) = self.out_target.size;
            let saved = image::save_buffer(&path, &self.gfx.read(&self.out_target), width, height, image::ColorType::Rgb8);
            println!("snapshot {} after {} frames: {saved:?}", path.display(), self.frames);
            self.options.snapshot = None;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if let Some(path) = self.options.ui_snapshot.clone() {
            if self.frames == self.options.frames {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            let shot = ctx.input(|input| {
                input.events.iter().find_map(|event| match event {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
            });
            if let Some(shot) = shot {
                let bytes: Vec<u8> = shot.pixels.iter().flat_map(|pixel| pixel.to_array()).collect();
                let saved =
                    image::save_buffer(&path, &bytes, shot.size[0] as u32, shot.size[1] as u32, image::ColorType::Rgba8);
                println!("window snapshot {}: {saved:?}", path.display());
                self.options.ui_snapshot = None;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if self.options.download.is_some() {
            let lines = self.download.as_ref().and_then(|job| job.log.lock().ok().map(|lines| lines.clone())).unwrap_or_default();
            if lines.last().is_some_and(|line| line == "Done." || line.ends_with("failed")) || self.download.is_none() {
                println!("{}", lines.join("\n"));
                println!("review {:?} library {}", self.review, self.library.len());
                self.options.download = None;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if self.options.undo_test {
            match self.frames {
                60 => self.apply(Action::ClearCells(vec![(1, 0), (1, 1)])),
                120 | 240 => self.step_history(true),
                180 => self.step_history(false),
                _ => {}
            }
            if [61, 121, 181, 241].contains(&self.frames) {
                let filled: Vec<usize> = (0..self.columns).filter(|at| self.layers[1].cells[*at].is_some()).collect();
                println!("frame {}: layer 2 holds {:?}, live {:?}, undo {} redo {}", self.frames, filled, self.layers[1].active, self.undo.len(), self.redo.len());
            }
        }
        if self.options.exit_after.is_some_and(|seconds| self.start.elapsed().as_secs_f32() >= seconds) {
            let errors: Vec<&String> = self
                .layers
                .iter()
                .flat_map(|layer| layer.error.iter().chain(layer.cells.iter().flatten().filter_map(|cell| cell.error.as_ref())))
                .collect();
            println!(
                "frames={} seconds={:.1} fps={:.1} tempo={:.1} sure={:.2} | audio {:?} rate {:?} error {:?} peak {:.4} | cells {} folders {} | library changes {} drawn {} | layers live {:?} drawn {:?} errors {:?} | output {:?} | midi {:?} out {:?} sent {} bound {} opacity {:.2} presets {} checked {} | groups {} pictures failed {} run {:?} preview {} | filled {:?} picked {:?} | cell fx {} layer fx {} | drag {:#?}",
                self.frames,
                self.start.elapsed().as_secs_f32(),
                self.frames as f32 / self.start.elapsed().as_secs_f32(),
                if self.heard.beat_period > 0.0 { 60.0 / self.heard.beat_period } else { 0.0 },
                self.heard.beat_sure,
                self.source.map(|index| self.sources[index].label()),
                self.capture.as_ref().map(|capture| capture.rate),
                self.audio_error,
                self.held.0,
                self.layers.iter().map(|layer| layer.cells.iter().flatten().count()).sum::<usize>(),
                self.folders.len(),
                self.auto_changes,
                self.auto.drawn,
                self.layers.iter().map(|layer| layer.active).collect::<Vec<_>>(),
                self.layers.iter().map(|layer| layer.drawn).collect::<Vec<_>>(),
                errors,
                (self.output_rect.get(), self.output_px.get(), self.displays.get(self.output_display).map(|display| (display.position, display.size))),
                self.midi.names,
                self.midi.output_names,
                self.midi.sent,
                self.bindings.len(),
                self.layers.get(1).map_or(0.0, |layer| layer.opacity),
                self.presets.len(),
                self.checked.len(),
                self.groups.len(),
                self.thumb_failed.len(),
                self.thumb_run,
                self.pre.drawn,
                self.layers.iter().map(|layer| (0..self.columns).filter(|at| layer.cells[*at].is_some()).collect::<Vec<_>>()).collect::<Vec<_>>(),
                self.picked,
                self.layers.iter().flat_map(|layer| layer.cells.iter().flatten()).map(|cell| cell.effects.len()).sum::<usize>(),
                self.layers.iter().map(|layer| layer.effects.len()).sum::<usize>(),
                self.drag_notes.borrow()
            );
            self.options.exit_after = None;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.compile_some(ctx);
        // New cells get their program and still picture a couple per frame, what is
        // live first.
        let mut waiting: Vec<(usize, usize)> = Vec::new();
        for (layer, row) in self.layers.iter().enumerate() {
            for (column, cell) in row.cells.iter().enumerate() {
                if cell.as_ref().is_some_and(|cell| !cell.prepared) {
                    waiting.push((layer, column));
                }
            }
        }
        waiting.sort_by_key(|(layer, column)| self.layers[*layer].active != Some(*column));
        for (layer, column) in waiting.into_iter().take(2) {
            self.prepare(layer, column, frame);
        }
        self.tick();
        if self.out_texture.is_none() {
            self.out_texture = Some(frame.register_native_glow_texture(self.out_target.texture));
        }
        if self.pre_texture.is_none() {
            self.pre_texture = Some(frame.register_native_glow_texture(self.pre_out.texture));
        }
        self.make_thumbs(ctx);
        self.take_thumbs(ctx);
        for path in std::mem::take(&mut *self.clip_thumbs_wanted.borrow_mut()) {
            if !self.asset_thumbs.contains_key(&path) && self.thumb_ask.send(path.clone()).is_ok() {
                self.asset_thumbs.insert(path, None);
            }
        }
        self.finish_download();
        // A changed number of the previewed shader: compile it again once the drag rests.
        if self.rebuild_at.is_some_and(|at| at.elapsed() > Duration::from_millis(150)) {
            self.rebuild_at = None;
            if let Some(entry) = self.editing.or(self.previewed()) {
                if self.previewed() == Some(entry) {
                    self.apply(Action::Preview(entry));
                }
                self.refresh_cells(entry);
            }
        }
        self.take_dropped_files(ctx);
        self.handle_midi();
        self.light();
        let mut actions = Vec::new();
        egui::TopBottomPanel::top("top").show(ctx, |ui| self.top_bar(ui, &mut actions));
        egui::SidePanel::left("library").default_width(330.0).show(ctx, |ui| self.library_panel(ui, &mut actions));
        egui::SidePanel::right("output").default_width(320.0).show(ctx, |ui| self.preview(ui, &mut actions));
        if self.detached && self.preview_window(ctx) {
            self.detached = false;
        }
        self.download_window(ctx);
        egui::CentralPanel::default().show(ctx, |ui| self.grid(ui, &mut actions));
        // In MIDI learn every bindable control gets a frame: yellow for the one waiting
        // for its knob or pad, blue for one that is bound, grey for one that is free.
        let frames = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("midi-learn")));
        // Ctrl+Z and Ctrl+Y (or Ctrl+Shift+Z), unless something is being typed: text has its own.
        if !ctx.wants_keyboard_input() {
            let pressed = |modifiers, key| ctx.input_mut(|input| input.consume_key(modifiers, key));
            if pressed(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Z) || pressed(egui::Modifiers::COMMAND, egui::Key::Y) {
                actions.push(Action::Redo);
            } else if pressed(egui::Modifiers::COMMAND, egui::Key::Z) {
                actions.push(Action::Undo);
            }
        }
        // Delete clears the selected cells, unless something is being typed.
        if !self.picked.is_empty() && !ctx.wants_keyboard_input() && ctx.input(|input| input.key_pressed(egui::Key::Delete)) {
            actions.push(Action::ClearCells(self.picked.iter().copied().collect()));
        }
        for action in actions {
            let Action::Spot(rect, control, picked, forget) = action else {
                self.apply(action);
                continue;
            };
            if forget {
                self.bindings.retain(|(_, bound)| *bound != control);
            } else if picked {
                self.learn_target = Some(control);
            }
            let bound = self.bindings.iter().find(|(_, known)| *known == control).map(|(key, _)| *key);
            let stroke = match (self.learn_target == Some(control), bound) {
                (true, _) => (3.0, egui::Color32::from_rgb(240, 200, 60)),
                (_, Some(_)) => (2.0, egui::Color32::from_rgb(70, 140, 255)),
                _ => (1.0, egui::Color32::from_gray(120)),
            };
            frames.rect_stroke(rect.expand(1.0), 3.0, stroke, egui::StrokeKind::Outside);
            if let Some((kind, _, number)) = bound {
                let tag = format!("{}{number}", if kind == 0xB0 { "K" } else { "P" });
                frames.text(rect.right_top(), egui::Align2::RIGHT_BOTTOM, tag, egui::FontId::proportional(10.0), stroke.1);
            }
        }
        if self.output_on {
            self.project(ctx);
        }
        self.track_undo(ctx);
        if self.saved_at.elapsed() > Duration::from_secs(20) {
            self.save_state();
        }
        self.tests(ctx);
        ctx.request_repaint();
    }
    /// The scripted pointer of `--drag-test`: press on the first list row, travel to the
    /// first cell of the second layer, let go.
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        if !self.options.drag_test {
            return;
        }
        let (Some(row), Some(cell)) = (self.seen_row.get(), self.seen_cell.get()) else { return };
        let (from, to) = (row.center(), cell.center());
        let button = |pressed| egui::Event::PointerButton {
            pos: if pressed { from } else { to },
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        match self.frames {
            100 => raw.events.push(egui::Event::PointerMoved(from)),
            104 => raw.events.push(button(true)),
            105..=124 => raw.events.push(egui::Event::PointerMoved(from + (to - from) * ((self.frames - 104) as f32 / 20.0))),
            128 => raw.events.push(button(false)),
            _ => {}
        }
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Leave the controllers dark, not showing a composition that is gone.
        for (status, number) in std::mem::take(&mut self.lights).into_keys() {
            if status & 0xF0 == 0x90 {
                self.midi.send([status, number, 0]);
            }
        }
        self.save_state();
    }
}
fn main() -> eframe::Result {
    let options = Options::parse();
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("RPi-VJ Desktop").with_inner_size([1700.0, 950.0]),
        renderer: eframe::Renderer::Glow,
        vsync: true,
        ..Default::default()
    };
    eframe::run_native(
        "RPi-VJ Desktop",
        native,
        Box::new(move |cc| Ok(Box::new(App::new(cc, options).map_err(|error| format!("{error:#}"))?))),
    )
}
