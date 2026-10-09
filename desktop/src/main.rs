//! RPi-VJ Desktop, phase 1: the shader library, one shader live with audio input and
//! beat prediction, a video clip under it, a preview, and projection on a chosen monitor.
mod audio_in;
mod gfx;
mod library;
mod mpv;

use anyhow::Result;
use eframe::egui;
use rpi_vj_native::{
    audio::{
        idle_audio, AudioFrame, BeatClock, BeatPredictor, Reactor, SectionDetector, BEAT_LEAD, FLOW_RATE,
        REACT_MODES, SPEC_FLOW_RATE,
    },
    shader,
};
use std::{
    path::PathBuf,
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};

const RENDER_SIZES: [(u32, u32); 3] = [(1280, 720), (1920, 1080), (2560, 1440)];

/// What the command line asks for besides the normal app; all of it is for testing.
#[derive(Default)]
struct Options {
    root: Option<PathBuf>,
    /// Compile every runnable shader, write a report and leave.
    compile_all: bool,
    /// Go live with this shader id at start.
    live: Option<String>,
    /// Play this clip at start.
    clip: Option<PathBuf>,
    /// Save the output picture here after `frames` frames and leave.
    snapshot: Option<PathBuf>,
    frames: u64,
    /// Save a picture of the app window here after `frames` frames and leave.
    ui_snapshot: Option<PathBuf>,
    /// FFT gain at start.
    gain: Option<f32>,
    /// Project on this monitor (by number) at start.
    output: Option<usize>,
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
                "--snapshot" => options.snapshot = Some(value().into()),
                "--ui-snapshot" => options.ui_snapshot = Some(value().into()),
                "--gain" => options.gain = value().parse().ok(),
                "--frames" => options.frames = value().parse().unwrap_or(90),
                "--output" => options.output = value().parse().ok(),
                "--exit-after" => options.exit_after = value().parse().ok(),
                other => eprintln!("unknown option {other}"),
            }
        }
        options
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
struct Live {
    entry: usize,
    program: gfx::Program,
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
    library: Vec<library::Entry>,
    filter: String,
    show_unrunnable: bool,
    live: Option<Live>,
    shader_error: Option<String>,
    size: usize,
    shader_target: gfx::Target,
    video_target: gfx::Target,
    out_target: gfx::Target,
    out_texture: Option<egui::TextureId>,
    opacity: f32,
    blend: usize,
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
    reactor: Reactor,
    sections: SectionDetector,
    section_count: u32,
    clock: BeatClock,
    lead_ms: f32,
    react: u8,
    idle: f32,
    idle_since: Instant,
    flow: [f32; 4],
    spec_flow: [f32; 32],
    // Video.
    player: Option<mpv::Player>,
    clip: Option<PathBuf>,
    video_error: Option<String>,
    // Output.
    displays: Vec<Display>,
    output_on: bool,
    output_display: usize,
    /// Where the projection window ended up, in points; for the test printout.
    output_rect: std::cell::Cell<Option<egui::Rect>>,
    // Timing.
    start: Instant,
    last: Instant,
    frames: u64,
    fps: f32,
    compile: Option<CompileRun>,
}
impl App {
    fn new(cc: &eframe::CreationContext<'_>, options: Options) -> Result<Self> {
        cc.egui_ctx.set_theme(egui::Theme::Dark);
        let gl = cc.gl.clone().ok_or_else(|| anyhow::anyhow!("no OpenGL context"))?;
        let gfx = gfx::Gfx::new(gl)?;
        let root = options.root.clone().unwrap_or_else(library::root);
        let library = library::load(&root)?;
        let size = 1;
        let displays = displays();
        let player = match cc.get_proc_address {
            Some(resolve) => mpv::Player::new(resolve),
            None => Err(anyhow::anyhow!("no OpenGL loader for the video player")),
        };
        let (player, video_error) = match player {
            Ok(player) => (Some(player), None),
            Err(error) => (None, Some(format!("{error:#}"))),
        };
        let now = Instant::now();
        let mut app = Self {
            shader_target: gfx.target(RENDER_SIZES[size])?,
            video_target: gfx.target(RENDER_SIZES[size])?,
            out_target: gfx.target(RENDER_SIZES[size])?,
            out_texture: None,
            root,
            gfx,
            library,
            filter: String::new(),
            show_unrunnable: false,
            live: None,
            shader_error: None,
            size,
            opacity: 1.0,
            blend: 0,
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
            reactor: Reactor::default(),
            sections: SectionDetector::default(),
            section_count: 0,
            clock: BeatClock::default(),
            lead_ms: BEAT_LEAD * 1000.0,
            react: 0,
            idle: 0.0,
            idle_since: now,
            flow: [0.0; 4],
            spec_flow: [0.0; 32],
            player,
            clip: None,
            video_error,
            output_on: false,
            output_rect: std::cell::Cell::new(None),
            output_display: displays.len().saturating_sub(1),
            displays,
            start: now,
            last: now,
            frames: 0,
            fps: 0.0,
            compile: None,
            options,
        };
        // Start on what the computer is playing, if it has an output device.
        if let Some(first) = app.sources.iter().position(|source| source.loopback).or((!app.sources.is_empty()).then_some(0)) {
            app.listen(first);
        }
        if let Some(id) = app.options.live.clone() {
            match app.library.iter().position(|entry| entry.id == id) {
                Some(entry) => app.go_live(entry),
                None => eprintln!("no shader {id} in the library"),
            }
        }
        if let Some(clip) = app.options.clip.clone() {
            app.play(clip);
        }
        if let Some(display) = app.options.output {
            app.output_on = true;
            app.output_display = display.saturating_sub(1).min(app.displays.len().saturating_sub(1));
        }
        if app.options.compile_all {
            app.compile = Some(CompileRun { next: 0, passed: 0, failures: Vec::new(), started: now });
        }
        Ok(app)
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
    fn build(&self, entry: usize) -> Result<gfx::Program> {
        let prepared = shader::load(&self.library[entry].path)?;
        self.gfx.program(&prepared.source, &prepared.defaults)
    }
    fn go_live(&mut self, entry: usize) {
        match self.build(entry) {
            Ok(program) => {
                if let Some(old) = self.live.replace(Live { entry, program }) {
                    self.gfx.delete(old.program);
                }
                self.shader_error = None;
            }
            Err(error) => self.shader_error = Some(format!("{}: {error:#}", self.library[entry].title())),
        }
    }
    fn play(&mut self, clip: PathBuf) {
        let Some(player) = &mut self.player else { return };
        match player.load(&clip) {
            Ok(()) => {
                self.clip = Some(clip);
                self.video_error = None;
            }
            Err(error) => self.video_error = Some(format!("{error:#}")),
        }
    }
    fn resize(&mut self, size: usize) -> Result<()> {
        // The old targets are small and few; they go with the context at exit.
        self.shader_target = self.gfx.target(RENDER_SIZES[size])?;
        self.video_target = self.gfx.target(RENDER_SIZES[size])?;
        self.out_target = self.gfx.target(RENDER_SIZES[size])?;
        self.out_texture = None;
        self.size = size;
        Ok(())
    }
    /// One frame of sound and picture: the same chain as the Pi's render loop.
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
        if self.sections.tick(dt, heard) {
            self.section_count += 1;
            self.reactor.new_section();
        }
        let quiet = heard.volume < 0.01 && heard.bass < 0.01 && heard.mid < 0.01;
        if self.idle == 0.0 {
            self.idle_since = now;
        }
        self.idle = (self.idle + if quiet { dt * 0.4 } else { -dt * 4.0 }).clamp(0.0, 1.0);
        let timed = self.predictor.apply(dt, self.lead_ms / 1000.0, heard);
        let moving = idle_audio(&timed, self.idle_since.elapsed().as_secs_f32(), self.idle);
        let moving = self.reactor.apply(self.react, dt, &moving);
        let flash = self.reactor.flash(self.react);
        for (clock, energy) in self.flow.iter_mut().zip([moving.volume, moving.bass, moving.mid, moving.treble]) {
            *clock += dt * (FLOW_RATE.0 + energy * FLOW_RATE.1);
        }
        for (clock, energy) in self.spec_flow.iter_mut().zip(gfx::spectrum_bands(&moving)) {
            *clock += dt * (SPEC_FLOW_RATE.0 + energy * SPEC_FLOW_RATE.1);
        }
        let found = self.predictor.bpm();
        let bpm = if found > 0.0 { found } else { 128.0 };
        let tempo = self.clock.tick(dt, bpm, &moving);
        if let Some(live) = &self.live {
            self.gfx.shader_pass(
                &live.program,
                &self.shader_target,
                &gfx::Frame {
                    time: self.start.elapsed().as_secs_f32(),
                    dt,
                    frame: self.frames as i32,
                    bpm,
                    tempo,
                    audio: &moving,
                    flow: &self.flow,
                    spec_flow: &self.spec_flow,
                },
            );
        }
        let mut video = None;
        if let Some(player) = &mut self.player {
            if let Err(error) = player.poll() {
                self.video_error = Some(format!("{error:#}"));
            }
            if self.clip.is_some() {
                match player.render(&self.gfx.gl, &self.video_target) {
                    Ok(()) => video = Some(self.video_target.texture),
                    Err(error) => self.video_error = Some(format!("{error:#}")),
                }
            }
        }
        self.gfx.mix(
            &self.out_target,
            &gfx::Mix {
                video,
                shader: self.live.as_ref().map(|_| self.shader_target.texture),
                opacity: self.opacity,
                mode: self.blend as i32,
                flash,
            },
        );
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
    fn top_bar(&mut self, ui: &mut egui::Ui) {
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
            ui.add(egui::Slider::new(&mut self.gain, 0.05..=16.0).logarithmic(true).text("FFT gain"));
            let db = |sample: f32| 20.0 * sample.max(1e-4).log10();
            let place = |sample: f32| ((db(sample) + 48.0) / 48.0).clamp(0.0, 1.0);
            let clipping = self.clipped.is_some_and(|at| at.elapsed() < Duration::from_secs(2));
            let color = if clipping {
                egui::Color32::from_rgb(235, 40, 40)
            } else if db(self.held.0) < -18.0 {
                egui::Color32::from_rgb(70, 120, 240)
            } else if db(self.held.0) < -6.0 {
                egui::Color32::from_rgb(60, 200, 90)
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
            let found = self.predictor.bpm();
            let tracked = if self.heard.beat_period > 0.0 { 60.0 / self.heard.beat_period } else { 0.0 };
            ui.label(format!(
                "Tempo {:.1}  sure {:.0}%  {}",
                tracked,
                self.heard.beat_sure * 100.0,
                if found > 0.0 { "predicting" } else { "following" }
            ));
            ui.add(egui::Slider::new(&mut self.lead_ms, 0.0..=250.0).suffix(" ms").text("beat lead"));
            ui.separator();
            ui.label("React");
            egui::ComboBox::from_id_salt("react").selected_text(REACT_MODES[usize::from(self.react)]).show_ui(ui, |ui| {
                for (index, name) in REACT_MODES.iter().enumerate() {
                    ui.selectable_value(&mut self.react, index as u8, *name);
                }
            });
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
                    self.shader_error = Some(format!("{error:#}"));
                }
            }
            ui.label(format!("{:.0} fps", self.fps));
        });
        if let Some(error) = &self.audio_error {
            ui.colored_label(egui::Color32::from_rgb(235, 90, 90), format!("Audio: {error}"));
        }
    }
    fn library_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Shaders");
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("search name, author or id").desired_width(200.0));
            ui.checkbox(&mut self.show_unrunnable, "multipass");
        });
        let needle = self.filter.to_lowercase();
        let shown: Vec<usize> = (0..self.library.len())
            .filter(|index| {
                let entry = &self.library[*index];
                (self.show_unrunnable || entry.runnable())
                    && (needle.is_empty()
                        || entry.name.to_lowercase().contains(&needle)
                        || entry.author.to_lowercase().contains(&needle)
                        || entry.id.to_lowercase().contains(&needle))
            })
            .collect();
        ui.label(format!("{} of {}", shown.len(), self.library.len()));
        let live = self.live.as_ref().map(|live| live.entry);
        let mut chosen = None;
        let row = ui.text_style_height(&egui::TextStyle::Body) + 4.0;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, row, shown.len(), |ui, rows| {
            // One line each: the list is drawn by row height.
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            for index in &shown[rows] {
                let entry = &self.library[*index];
                let mut text = egui::RichText::new(if entry.author.is_empty() {
                    entry.title().to_owned()
                } else {
                    format!("{}  ·  {}", entry.title(), entry.author)
                });
                if !entry.runnable() {
                    text = text.weak();
                }
                let response = ui.add_enabled(entry.runnable(), egui::SelectableLabel::new(live == Some(*index), text));
                if response.clicked() {
                    chosen = Some(*index);
                }
            }
        });
        if let Some(index) = chosen {
            self.go_live(index);
        }
    }
    fn stage(&mut self, ui: &mut egui::Ui) {
        if let Some(run) = &self.compile {
            ui.add(egui::ProgressBar::new(run.next as f32 / self.library.len().max(1) as f32).text("compiling the library"));
        }
        if let Some(texture) = self.out_texture {
            let width = ui.available_width().min(960.0);
            let size = egui::vec2(width, width * 9.0 / 16.0);
            // The render target's first row is its bottom one.
            let flipped = egui::Rect::from_min_max(egui::pos2(0.0, 1.0), egui::pos2(1.0, 0.0));
            ui.add(egui::Image::new(egui::load::SizedTexture::new(texture, size)).uv(flipped));
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.strong("Shader layer");
            match &self.live {
                Some(live) => ui.label(self.library[live.entry].title()),
                None => ui.weak("pick a shader on the left"),
            };
            if self.live.is_some() && ui.button("Off").clicked() {
                if let Some(old) = self.live.take() {
                    self.gfx.delete(old.program);
                }
            }
        });
        ui.horizontal(|ui| {
            ui.add(egui::Slider::new(&mut self.opacity, 0.0..=1.0).text("opacity"));
            egui::ComboBox::from_id_salt("blend").selected_text(gfx::BLENDS[self.blend]).show_ui(ui, |ui| {
                for (index, name) in gfx::BLENDS.iter().enumerate() {
                    ui.selectable_value(&mut self.blend, index, *name);
                }
            });
        });
        if let Some(error) = &self.shader_error {
            ui.colored_label(egui::Color32::from_rgb(235, 90, 90), error);
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.strong("Video layer");
            match &self.clip {
                Some(clip) => ui.label(clip.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()),
                None => ui.weak("no clip"),
            };
            if ui.add_enabled(self.player.is_some(), egui::Button::new("Open clip…")).clicked() {
                let picked = rfd::FileDialog::new()
                    .add_filter("Video", &["mp4", "mov", "mkv", "webm", "avi", "m4v"])
                    .pick_file();
                if let Some(clip) = picked {
                    self.play(clip);
                }
            }
            if self.clip.is_some() && ui.button("Off").clicked() {
                if let Some(player) = &mut self.player {
                    let _ = player.stop();
                }
                self.clip = None;
            }
        });
        if let Some(error) = &self.video_error {
            ui.colored_label(egui::Color32::from_rgb(235, 90, 90), format!("Video: {error}"));
        }
        ui.add_space(6.0);
        ui.weak(format!(
            "sections {}   bass {:.2}  mid {:.2}  treble {:.2}   shader dialect {}",
            self.section_count,
            self.heard.bass,
            self.heard.mid,
            self.heard.treble,
            self.gfx.dialect()
        ));
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
                let flipped = egui::Rect::from_min_max(egui::pos2(0.0, 1.0), egui::pos2(1.0, 0.0));
                ui.painter().image(texture, rect, flipped, egui::Color32::WHITE);
            });
        });
    }
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.compile_some(ctx);
        self.tick();
        if self.out_texture.is_none() {
            self.out_texture = Some(frame.register_native_glow_texture(self.out_target.texture));
        }
        egui::TopBottomPanel::top("top").show(ctx, |ui| self.top_bar(ui));
        egui::SidePanel::left("library").default_width(380.0).show(ctx, |ui| self.library_panel(ui));
        egui::CentralPanel::default().show(ctx, |ui| self.stage(ui));
        if self.output_on {
            self.project(ctx);
        }
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
        if self.options.exit_after.is_some_and(|seconds| self.start.elapsed().as_secs_f32() >= seconds) {
            println!(
                "frames={} seconds={:.1} fps={:.1} tempo={:.1} sure={:.2} | audio {:?} rate {:?} error {:?} peak {:.4} bass {:.3} | video {:?} error {:?} | output {:?}",
                self.frames,
                self.start.elapsed().as_secs_f32(),
                self.frames as f32 / self.start.elapsed().as_secs_f32(),
                if self.heard.beat_period > 0.0 { 60.0 / self.heard.beat_period } else { 0.0 },
                self.heard.beat_sure,
                self.source.map(|index| self.sources[index].label()),
                self.capture.as_ref().map(|capture| capture.rate),
                self.audio_error,
                self.held.0,
                self.heard.bass,
                self.clip,
                self.video_error,
                self.output_rect.get()
            );
            self.options.exit_after = None;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint();
    }
}
fn main() -> eframe::Result {
    let options = Options::parse();
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("RPi-VJ Desktop").with_inner_size([1500.0, 900.0]),
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
