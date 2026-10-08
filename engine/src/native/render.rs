use super::textures::{self, Channels, VIDEO_UNIT};
use crate::{
    audio::{AudioFrame, BeatClock, Tempo, FLOW_RATE, SPEC_FLOW_RATE},
    control::{BlendMode, State},
    shader,
    show::Show,
    texture::{Channel, MAX_CHANNELS},
};
use anyhow::{ensure, Result};
use font8x8::UnicodeFonts;
use glow::HasContext;
use std::collections::HashMap;

pub struct Target {
    pub fbo: glow::Framebuffer,
    pub texture: glow::Texture,
}
pub(super) struct Program {
    pub(super) id: glow::Program,
    uniforms: HashMap<String, glow::UniformLocation>,
    /// Declared sampler type of each `iChannelN` the shader actually reads.
    channel_types: [Option<u32>; MAX_CHANNELS],
    /// Input textures from the manifest; see `bind_channels`.
    channels: Channels,
    /// The shader reads the deck video through `uniform sampler2D u_video`.
    video: bool,
}

impl Program {
    pub(super) unsafe fn new(gl: &glow::Context, fragment: &str) -> Result<Self> {
        Self::with_vertex(gl, include_str!("../../shaders/fullscreen.vert"), fragment)
    }
    unsafe fn with_vertex(gl: &glow::Context, vertex: &str, fragment: &str) -> Result<Self> {
        let id = gl.create_program().map_err(anyhow::Error::msg)?;
        let result = (|| {
            for (kind, source) in [
                (glow::VERTEX_SHADER, vertex),
                (glow::FRAGMENT_SHADER, fragment),
            ] {
                let shader = gl.create_shader(kind).map_err(anyhow::Error::msg)?;
                gl.shader_source(shader, source);
                gl.compile_shader(shader);
                let ok = gl.get_shader_compile_status(shader);
                let log = gl.get_shader_info_log(shader);
                if ok {
                    gl.attach_shader(id, shader);
                }
                gl.delete_shader(shader);
                ensure!(ok, "shader compile failed: {log}");
            }
            gl.link_program(id);
            ensure!(
                gl.get_program_link_status(id),
                "link failed: {}",
                gl.get_program_info_log(id)
            );
            let mut uniforms = HashMap::new();
            let mut channel_types = [None; MAX_CHANNELS];
            let mut video = false;
            for index in 0..gl.get_active_uniforms(id) {
                if let Some(u) = gl.get_active_uniform(id, index) {
                    let name = u.name.trim_end_matches("[0]");
                    if let Some(loc) = gl.get_uniform_location(id, name) {
                        uniforms.insert(name.to_owned(), loc);
                    }
                    let channel = name
                        .strip_prefix("iChannel")
                        .and_then(|n| n.parse::<usize>().ok())
                        .filter(|n| *n < MAX_CHANNELS);
                    if let Some(channel) = channel {
                        channel_types[channel] = textures::sampler_target(u.utype).map(|_| u.utype);
                    }
                    video |= name == "u_video" && u.utype == glow::SAMPLER_2D;
                }
            }
            // Every sampler gets its own unit once. Left at the default they would all
            // read unit 0, and two sampler types on one unit make the draw call fail.
            gl.use_program(Some(id));
            for (channel, declared) in channel_types.iter().enumerate() {
                if declared.is_some() {
                    gl.uniform_1_i32(
                        uniforms.get(&format!("iChannel{channel}")),
                        channel as i32,
                    );
                }
            }
            if video {
                gl.uniform_1_i32(uniforms.get("u_video"), VIDEO_UNIT as i32);
            }
            gl.use_program(None);
            Ok(Self {
                id,
                uniforms,
                channel_types,
                channels: [None; MAX_CHANNELS],
                video,
            })
        })();
        if result.is_err() {
            gl.delete_program(id);
        }
        result
    }
    /// Attach the manifest's input textures. A channel the shader declares with another
    /// sampler type than its binding would silently read black, so that is an error.
    /// Returns the channels the shader reads that have no binding (they sample opaque
    /// black), for the caller to report once.
    pub(super) unsafe fn bind_channels(
        &mut self,
        gl: &glow::Context,
        channels: Channels,
    ) -> Result<Vec<usize>> {
        let mut resolution = [0.0f32; MAX_CHANNELS * 3];
        for (index, bound) in channels.iter().enumerate() {
            let Some(bound) = bound else { continue };
            if let Some(declared) = self.channel_types[index] {
                ensure!(
                    declared == textures::sampler_type(bound.kind),
                    "iChannel{index} is bound to a {} texture, which needs `uniform {} iChannel{index};` in the shader",
                    bound.kind.name(),
                    bound.kind.sampler()
                );
            }
            resolution[index * 3..index * 3 + 3].copy_from_slice(&bound.size);
        }
        gl.use_program(Some(self.id));
        gl.uniform_3_f32_slice(self.uniforms.get("iChannelResolution"), &resolution);
        gl.use_program(None);
        self.channels = channels;
        Ok((0..MAX_CHANNELS)
            .filter(|index| self.channel_types[*index].is_some() && channels[*index].is_none())
            .collect())
    }
    /// Message for `bind_channels`' unbound list, or `None` when every read channel is bound.
    pub(super) fn unbound_warning(unbound: &[usize]) -> Option<String> {
        (!unbound.is_empty()).then(|| {
            let names: Vec<String> = unbound.iter().map(|n| format!("iChannel{n}")).collect();
            format!(
                "reads {} without a channel binding: sampled as opaque black",
                names.join(", ")
            )
        })
    }
    pub(super) unsafe fn float(&self, gl: &glow::Context, name: &str, v: f32) {
        gl.uniform_1_f32(self.uniforms.get(name), v);
    }
    unsafe fn int(&self, gl: &glow::Context, name: &str, v: i32) {
        gl.uniform_1_i32(self.uniforms.get(name), v);
    }
    unsafe fn size(&self, gl: &glow::Context, name: &str, w: f32, h: f32) {
        gl.uniform_2_f32(self.uniforms.get(name), w, h);
    }
}
/// The deck targets hold the video the right way up (mpv renders them with FLIP_Y), so
/// `u_video` shaders see a normal bottom-left origin. The compositor flips them again for
/// the output because that reproduces what it showed before `u_video` existed, when mpv
/// rendered unflipped and this uniform was 0: an upright clip appears upside down on the
/// output. Whether that is wanted is the owner's decision; `false` shows video upright.
const DECK_OUTPUT_FLIP: bool = true;
pub struct Renderer {
    gl: glow::Context,
    program: Program,
    /// The compositor with shader-layer geometry (layer B), on a grid mesh.
    program_geo: Program,
    vao: glow::VertexArray,
    pub targets: [Target; 2],
    /// One offscreen target per distinct shader render size in the show.
    shader_targets: Vec<((u32, u32), Target)>,
    shader_sizes: Vec<(u32, u32)>,
    shaders: Vec<Option<Program>>,
    shader_sources: Vec<Option<shader::PreparedShader>>,
    /// Resolved input textures per show shader, attached when the shader is compiled.
    shader_channels: Vec<Channels>,
    bank: textures::Bank,
    errors: Vec<Option<String>>,
    previous_time: Option<f32>,
    flow: [f32; 4],
    spec_flow: [f32; 32],
    clock: BeatClock,
    frame: i32,
    clip_layouts: Vec<bool>,
    hud_texture: glow::Texture,
    hud_pixels: Vec<u8>,
    hud_stamp: f32,
    /// Accumulated drift rotation of the shader layer, radians.
    geo_drift: f32,
    /// Recent peak of each band that drives the layer geometry, and its level against it.
    geo_peak: [f32; 5],
    pub geo_level: [f32; 5],
    /// 1 while the audio is the silence stand-in, 0 with real sound.
    pub geo_idle: f32,
}
/// Diagnostic override: when either variable is set, every shader renders at this size
/// regardless of its manifest `render_height`.
pub fn render_override() -> Option<(u32, u32)> {
    let read = |name: &str| std::env::var(name).ok().and_then(|v| v.parse().ok());
    let (width, height) = (read("RPI_VJ_RENDER_WIDTH"), read("RPI_VJ_RENDER_HEIGHT"));
    if width.is_none() && height.is_none() {
        return None;
    }
    let (width, height) = (width.unwrap_or(1920), height.unwrap_or(1080));
    if (320..=3840).contains(&width) && (240..=2160).contains(&height) {
        Some((width, height))
    } else {
        Some((1920, 1080))
    }
}
pub fn render_size() -> (u32, u32) {
    render_override().unwrap_or((1920, 1080))
}
/// Everything a shader pass reads besides its own ISF defaults.
pub(super) struct ShaderFrame<'a> {
    pub time: f32,
    pub dt: f32,
    pub frame: i32,
    pub bpm: f32,
    /// Beat and bar phase and the lock level published as `u_locked`.
    pub tempo: Tempo,
    pub audio: &'a AudioFrame,
    pub flow: &'a [f32; 4],
    pub spec_flow: &'a [f32; 32],
    /// Texture a `u_video` shader samples; `None` reads opaque black.
    pub video: Option<glow::Texture>,
    /// The playing clip is a `quad_mirror` clip: `u_video` holds the stored quarter-frame
    /// source, not the unfolded picture the compositor shows. Passed on as
    /// `u_video_quad_mirror` so a shader can tell; the engine does not unfold for it.
    pub video_quad_mirror: bool,
}
pub(super) fn spectrum_bands(audio: &AudioFrame) -> [f32; 32] {
    std::array::from_fn(|i| {
        audio.spectrum[i * 4..i * 4 + 4]
            .iter()
            .copied()
            .fold(0.0, f32::max)
    })
}
/// Fixed-function state shared by every fullscreen pass.
pub(super) unsafe fn fullscreen_state(gl: &glow::Context, vao: glow::VertexArray) {
    gl.disable(glow::DEPTH_TEST);
    gl.disable(glow::SCISSOR_TEST);
    gl.disable(glow::BLEND);
    gl.disable(glow::CULL_FACE);
    gl.color_mask(true, true, true, true);
    gl.bind_vertex_array(Some(vao));
}
/// Render one shader frame into `fbo` at `size`. The live compositor and the benchmark
/// share this so the benchmark measures exactly the pass the show runs.
pub(super) unsafe fn shader_pass(
    gl: &glow::Context,
    p: &Program,
    fbo: glow::Framebuffer,
    size: (u32, u32),
    f: &ShaderFrame,
) {
    gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
    gl.viewport(0, 0, size.0 as i32, size.1 as i32);
    gl.use_program(Some(p.id));
    gl.clear_color(0.0, 0.0, 0.0, 1.0);
    gl.clear(glow::COLOR_BUFFER_BIT);
    p.size(gl, "u_resolution", size.0 as f32, size.1 as f32);
    p.size(gl, "RENDERSIZE", size.0 as f32, size.1 as f32);
    p.int(gl, "FRAMEINDEX", f.frame);
    gl.uniform_4_f32(p.uniforms.get("DATE"), 0.0, 0.0, 0.0, 0.0);
    let (time, audio) = (f.time, f.audio);
    let (beat, bar) = (f.tempo.beat, f.tempo.bar);
    for (name, v) in [
        ("u_time", time),
        ("TIME", time),
        ("TIMEDELTA", f.dt),
        ("u_bass", audio.bass),
        ("u_mid", audio.mid),
        ("u_treble", audio.treble),
        ("u_volume", audio.volume),
        ("u_onset", audio.onset),
        ("u_beat_onset", audio.onset),
        ("u_energy", audio.volume),
        ("u_kick", audio.onset),
        ("u_bpm", f.bpm),
        ("u_sub", audio.sub),
        ("u_low_mid", audio.low_mid),
        ("u_high_mid", audio.high_mid),
        ("u_hit_low", audio.onset * audio.bass),
        ("u_hit_mid", audio.onset * audio.mid),
        ("u_hit_high", audio.onset * audio.treble),
        ("u_beat_phase", beat),
        ("u_bar_phase", bar),
        ("u_locked", f.tempo.locked),
        ("u_flow", f.flow[0]),
        ("u_flow_low", f.flow[1]),
        ("u_flow_mid", f.flow[2]),
        ("u_flow_high", f.flow[3]),
        ("uSubBass", audio.sub),
        ("uBass", audio.bass),
        ("uLowMid", audio.low_mid),
        ("uHighMid", audio.high_mid),
        ("uTreble", audio.treble),
        ("uEnergy", audio.volume),
        ("uKick", audio.onset),
        ("uBPM", f.bpm),
        ("uBeatPhase", beat),
        ("uBarPhase", bar),
    ] {
        p.float(gl, name, v);
    }
    gl.uniform_1_f32_slice(p.uniforms.get("iChannelTime"), &[time; 4]);
    gl.uniform_1_f32_slice(p.uniforms.get("u_spectrum"), &spectrum_bands(audio));
    gl.uniform_1_f32_slice(p.uniforms.get("u_spec_flow"), f.spec_flow);
    // Input textures. Only samplers the shader really reads cost anything here, and each
    // of those gets a defined texture: its binding, or nothing (opaque black) instead of
    // whatever the compositor or a previous shader left on the unit.
    let mut touched = false;
    for (unit, declared) in p.channel_types.iter().enumerate() {
        let Some(declared) = declared else { continue };
        touched = true;
        gl.active_texture(glow::TEXTURE0 + unit as u32);
        match &p.channels[unit] {
            Some(bound) => {
                gl.bind_texture(textures::target(bound.kind), Some(bound.texture));
                gl.bind_sampler(unit as u32, Some(bound.sampler));
            }
            None => {
                if let Some(target) = textures::sampler_target(*declared) {
                    gl.bind_texture(target, None);
                }
            }
        }
    }
    if p.video {
        touched = true;
        gl.active_texture(glow::TEXTURE0 + VIDEO_UNIT);
        gl.bind_texture(glow::TEXTURE_2D, f.video);
        p.int(gl, "u_video_quad_mirror", i32::from(f.video_quad_mirror));
    }
    gl.draw_arrays(glow::TRIANGLES, 0, 3);
    if touched {
        // Sampler objects override texture parameters for whoever uses the unit next.
        for (unit, bound) in p.channels.iter().enumerate() {
            if let (Some(bound), Some(_)) = (bound, p.channel_types[unit]) {
                gl.active_texture(glow::TEXTURE0 + unit as u32);
                gl.bind_texture(textures::target(bound.kind), None);
                gl.bind_sampler(unit as u32, None);
            }
        }
        if p.video {
            gl.active_texture(glow::TEXTURE0 + VIDEO_UNIT);
            gl.bind_texture(glow::TEXTURE_2D, None);
        }
        gl.active_texture(glow::TEXTURE0);
    }
}
impl Renderer {
    /// Construct only on the current SDL GLES context thread. All compilation is startup-only.
    pub fn new(video: &sdl2::VideoSubsystem, show: &Show) -> Result<Self> {
        // SAFETY: caller owns a current GLES context; resources never leave this thread.
        unsafe {
            let gl =
                glow::Context::from_loader_function(|name| video.gl_get_proc_address(name).cast());
            eprintln!(
                "GPU: {} / {}",
                gl.get_parameter_string(glow::RENDERER),
                gl.get_parameter_string(glow::VERSION)
            );
            let composite = include_str!("../../shaders/program.frag");
            let program = Program::new(&gl, composite)?;
            let program_geo = Program::with_vertex(
                &gl,
                include_str!("../../shaders/composite.vert"),
                &composite.replacen("#version 300 es\n", "#version 300 es\n#define GEO 1\n", 1),
            )?;
            let vao = gl.create_vertex_array().map_err(anyhow::Error::msg)?;
            let render_size = render_size();
            let targets = [target(&gl, render_size)?, target(&gl, render_size)?];
            let forced = render_override();
            let shader_sizes: Vec<(u32, u32)> = show
                .shaders
                .iter()
                .map(|shader| forced.unwrap_or(shader.render_size()))
                .collect();
            // The compositor always samples a shader texture, so keep one even without shaders.
            let mut sizes = shader_sizes.clone();
            sizes.sort_unstable_by(|a, b| b.cmp(a));
            sizes.dedup();
            if sizes.is_empty() {
                sizes.push(render_size);
            }
            let mut shader_targets = Vec::new();
            for size in sizes {
                shader_targets.push((size, target(&gl, size)?));
            }
            eprintln!(
                "shader targets: {}",
                shader_targets
                    .iter()
                    .map(|((w, h), _)| format!("{w}x{h}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            let hud_texture = gl.create_texture().map_err(anyhow::Error::msg)?;
            gl.bind_texture(glow::TEXTURE_2D, Some(hud_texture));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::R8 as i32,
                768,
                16,
                0,
                glow::RED,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );
            for parameter in [glow::TEXTURE_MIN_FILTER, glow::TEXTURE_MAG_FILTER] {
                gl.tex_parameter_i32(glow::TEXTURE_2D, parameter, glow::NEAREST as i32);
            }
            // Input textures are decoded and uploaded once, here, never on the show path.
            let wanted: Vec<&Channel> =
                show.shaders.iter().flat_map(|s| &s.channels.list).collect();
            let bank = textures::Bank::load(&gl, &wanted);
            let mut shaders = Vec::new();
            let mut shader_sources = Vec::new();
            let mut shader_channels = Vec::new();
            let mut errors = Vec::new();
            for shader in &show.shaders {
                // An invalid channel block, or a missing or refused texture, disables this
                // shader only: it must not render with a wrong or absent input.
                let loaded = shader
                    .channels
                    .checked()
                    .and_then(|list| bank.resolve(list))
                    .and_then(|channels| {
                        let source = shader::load_with(&shader.path, &shader.channels.kinds())?;
                        Ok((source, channels))
                    });
                match loaded {
                    Ok((source, channels)) => {
                        shaders.push(None);
                        shader_sources.push(Some(source));
                        shader_channels.push(channels);
                        errors.push(None);
                    }
                    Err(e) => {
                        eprintln!("Shader disabled [{}]: {e:#}", shader.label);
                        shaders.push(None);
                        shader_sources.push(None);
                        shader_channels.push([None; MAX_CHANNELS]);
                        errors.push(Some(format!("{e:#}")));
                    }
                }
            }
            gl.use_program(None);
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            Ok(Self {
                gl,
                program,
                program_geo,
                vao,
                targets,
                shader_targets,
                shader_sizes,
                shaders,
                shader_sources,
                shader_channels,
                bank,
                errors,
                previous_time: None,
                flow: [0.0; 4],
                spec_flow: [0.0; 32],
                clock: BeatClock::default(),
                frame: 0,
                clip_layouts: show.clips.iter().map(|c| c.quad_mirror).collect(),
                hud_texture,
                hud_pixels: vec![0; 768 * 16],
                hud_stamp: -1.0,
                geo_drift: 0.0,
                geo_peak: [0.02; 5],
                geo_level: [0.0; 5],
                geo_idle: 0.0,
            })
        }
    }
    pub fn shader_errors(&self) -> &[Option<String>] {
        &self.errors
    }
    fn compile_selected(&mut self, index: usize) {
        if self.shaders.get(index).is_none_or(Option::is_some) || self.errors.get(index).is_some_and(Option::is_some) { return; }
        let Some(source) = self.shader_sources.get_mut(index).and_then(Option::take) else { return; };
        // SAFETY: the renderer owns the current GL context and this is called from draw.
        unsafe {
            let compiled = Program::new(&self.gl, &source.source).and_then(|mut program| {
                match program.bind_channels(&self.gl, self.shader_channels[index]) {
                    Ok(unbound) => {
                        if let Some(warning) = Program::unbound_warning(&unbound) {
                            eprintln!("Shader [{index}] {warning}");
                        }
                        Ok(program)
                    }
                    Err(error) => {
                        self.gl.delete_program(program.id);
                        Err(error)
                    }
                }
            });
            match compiled {
                Ok(program) => {
                    self.gl.use_program(Some(program.id));
                    for (name, value) in source.defaults { program.float(&self.gl, &name, value); }
                    self.gl.use_program(None);
                    self.shaders[index] = Some(program);
                }
                Err(error) => {
                    self.errors[index] = Some(format!("{error:#}"));
                    eprintln!("Shader disabled [{}]: {error:#}", index);
                }
            }
        }
    }
    pub fn draw(
        &mut self,
        active: usize,
        state: &State,
        audio: &AudioFrame,
        time: f32,
        size: (u32, u32),
    ) {
        let dt = self
            .previous_time
            .map_or(0.0, |t| (time - t).clamp(0.0, 0.1));
        self.previous_time = Some(time);
        self.frame = self.frame.wrapping_add(1);
        for (clock, energy) in
            self.flow
                .iter_mut()
                .zip([audio.volume, audio.bass, audio.mid, audio.treble])
        {
            *clock += dt * (FLOW_RATE.0 + energy * FLOW_RATE.1);
        }
        for (clock, energy) in self.spec_flow.iter_mut().zip(spectrum_bands(audio)) {
            *clock += dt * (SPEC_FLOW_RATE.0 + energy * SPEC_FLOW_RATE.1);
        }
        let tempo = self.clock.tick(dt, state.bpm, audio);
        // Layer B macros: knobs 1-5 each let one band drive one transform of the shader layer
        // (bass bulge, low-mid twist, mid wave, treble ripple, kick stretch).
        let m = state.macros;
        let spin = (m[6] - 0.5) * 2.0;
        if spin.abs() > 0.04 {
            self.geo_drift = (self.geo_drift + dt * spin * 1.5) % std::f32::consts::TAU;
        } else if self.geo_drift != 0.0 {
            // Knob centred: settle back upright instead of freezing at an angle.
            self.geo_drift *= 1.0 - (dt * 3.0).min(1.0);
            if self.geo_drift.abs() < 0.002 {
                self.geo_drift = 0.0;
            }
        }
        // Band levels depend on the track and the input trim, so each band is measured
        // against its own recent peak: a knob at full then moves the picture on any music.
        let bands = [audio.bass, audio.low_mid, audio.mid, audio.treble, audio.onset];
        let mut level = [0.0f32; 5];
        for ((peak, value), out) in self.geo_peak.iter_mut().zip(bands).zip(&mut level) {
            *peak = (*peak * (1.0 - dt * 0.07)).max(value).max(0.02);
            let lifted = (value / *peak).clamp(0.0, 1.0).powf(0.8);
            // In silence the stand-in levels are used as they are, so the layer moves half
            // way at most instead of being stretched to full scale against a decayed peak.
            *out = lifted + (value.clamp(0.0, 1.0) - lifted) * self.geo_idle;
        }
        self.geo_level = level;
        let geo = [
            m[0] * 0.85 * level[0],
            self.geo_drift,
            m[1] * 3.5 * level[1] * (0.6 + 0.4 * (time * 0.7).sin()),
            m[4] * 0.6 * level[4],
        ];
        let geo2 = [
            m[3] * 0.12 * level[3],
            self.flow[3] * 8.0,
            if m[5] < 0.06 { 0.0 } else { 2.0 + (m[5] * 6.99).floor() },
            m[7] * 2.0,
        ];
        let geo3 = [m[2] * 0.2 * level[2], self.flow[2] * 3.0, 0.0, 0.0];
        // The geometry build of the compositor is used only while a knob is off its default.
        let geo_on = m[..6].iter().any(|v| *v > 0.0)
            || self.geo_drift != 0.0
            || (m[7] - 0.5).abs() > 0.004;
        if state.shader && !state.blackout {
            if let Some(index) = state.selected_shader { self.compile_selected(index); }
        }
        let shader = state
            .selected_shader
            .and_then(|i| self.shaders.get(i))
            .and_then(Option::as_ref)
            .filter(|_| state.shader && !state.blackout);
        // The selected shader's tier picks the target; the compositor upsamples it.
        let wanted = state
            .selected_shader
            .and_then(|i| self.shader_sizes.get(i))
            .copied();
        let (shader_size, shader_target) = self
            .shader_targets
            .iter()
            .find(|(size, _)| Some(*size) == wanted)
            .unwrap_or(&self.shader_targets[0]);
        // SAFETY: draw is called with this renderer's context current; cached locations belong to their programs.
        unsafe {
            let gl = &self.gl;
            if state.hud && time - self.hud_stamp >= 0.25 {
                self.hud_pixels.fill(0);
                let text = format!(
                    "BPM {:.0} | SPEED {:.2} | VIDEO FX {} | SHADER FX {} | FRAME {:.1}ms",
                    state.bpm,
                    state.speed,
                    state.video_effect,
                    state.shader_effect,
                    dt * 1000.0
                );
                for (index, c) in text.chars().take(95).enumerate() {
                    if let Some(glyph) = font8x8::BASIC_FONTS.get(c) {
                        for (y, bits) in glyph.iter().enumerate() {
                            for x in 0..8 {
                                if bits & (1 << x) != 0 {
                                    self.hud_pixels[(11 - y) * 768 + index * 8 + x] = 255;
                                }
                            }
                        }
                    }
                }
                gl.active_texture(glow::TEXTURE2);
                gl.bind_texture(glow::TEXTURE_2D, Some(self.hud_texture));
                gl.tex_sub_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    0,
                    0,
                    768,
                    16,
                    glow::RED,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(Some(&self.hud_pixels)),
                );
                self.hud_stamp = time;
            }
            fullscreen_state(gl, self.vao);
            if let Some(p) = shader {
                shader_pass(
                    gl,
                    p,
                    shader_target.fbo,
                    *shader_size,
                    &ShaderFrame {
                        time,
                        dt,
                        frame: self.frame,
                        bpm: state.bpm,
                        tempo,
                        audio,
                        flow: &self.flow,
                        spec_flow: &self.spec_flow,
                        // The deck target mpv just rendered; black while nothing plays.
                        video: state
                            .playing
                            .map(|_| self.targets[active.min(1)].texture),
                        video_quad_mirror: state
                            .playing
                            .and_then(|i| self.clip_layouts.get(i))
                            .copied()
                            .unwrap_or(false),
                    },
                );
            }
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            gl.viewport(0, 0, size.0 as i32, size.1 as i32);
            let p = if geo_on { &self.program_geo } else { &self.program };
            gl.use_program(Some(p.id));
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.targets[active.min(1)].texture));
            gl.active_texture(glow::TEXTURE1);
            gl.bind_texture(glow::TEXTURE_2D, Some(shader_target.texture));
            gl.active_texture(glow::TEXTURE2);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.hud_texture));
            p.int(gl, "u_hud_tex", 2);
            p.int(gl, "u_hud", i32::from(state.hud));
            p.int(gl, "u_deck_tex", 0);
            p.int(gl, "u_shader_tex", 1);
            p.size(gl, "u_resolution", size.0 as f32, size.1 as f32);
            p.float(
                gl,
                "u_deck_opacity",
                if state.playing.is_some() { state.video_opacity } else { 0.0 },
            );
            p.float(
                gl,
                "u_shader_opacity",
                if shader.is_some() { state.opacity } else { 0.0 },
            );
            p.float(gl, "u_fx_time", time);
            for (name, value) in [("u_geo", geo), ("u_geo2", geo2), ("u_geo3", geo3)] {
                if let Some(location) = gl.get_uniform_location(p.id, name) {
                    gl.uniform_4_f32(Some(&location), value[0], value[1], value[2], value[3]);
                }
            }
            p.float(gl, "u_fx_amount", state.video_effect_amount);
            p.float(gl, "u_sfx_amount", state.shader_effect_amount);
            p.int(gl, "u_fx", state.video_effect as i32);
            p.int(gl, "u_sfx", state.shader_effect as i32);
            p.int(gl, "u_blackout", i32::from(state.blackout));
            p.int(gl, "u_invert_order", i32::from(state.invert_order));
            p.int(
                gl,
                "u_deck_layout",
                i32::from(
                    state
                        .playing
                        .and_then(|i| self.clip_layouts.get(i))
                        .copied()
                        .unwrap_or(false),
                ),
            );
            p.int(gl, "u_deck_flip_y", i32::from(DECK_OUTPUT_FLIP));
            p.int(
                gl,
                "u_blend_mode",
                match state.blend {
                    BlendMode::Alpha => 0,
                    BlendMode::Additive => 1,
                    BlendMode::Screen => 2,
                    BlendMode::Multiply => 3,
                    BlendMode::Difference => 6,
                    BlendMode::Overlay => 8,
                    BlendMode::SoftLight => 9,
                    BlendMode::LumaKey => 14,
                },
            );
            // The geometry build draws the 64x36 quad grid generated in composite.vert.
            gl.draw_arrays(glow::TRIANGLES, 0, if geo_on { 64 * 36 * 6 } else { 3 });
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl.active_texture(glow::TEXTURE1);
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl.bind_vertex_array(None);
            gl.use_program(None);
        }
    }
}
pub(super) unsafe fn target(gl: &glow::Context, size: (u32, u32)) -> Result<Target> {
    let texture = gl.create_texture().map_err(anyhow::Error::msg)?;
    gl.bind_texture(glow::TEXTURE_2D, Some(texture));
    gl.tex_image_2d(
        glow::TEXTURE_2D,
        0,
        glow::RGBA8 as i32,
        size.0 as i32,
        size.1 as i32,
        0,
        glow::RGBA,
        glow::UNSIGNED_BYTE,
        glow::PixelUnpackData::Slice(None),
    );
    for parameter in [glow::TEXTURE_MIN_FILTER, glow::TEXTURE_MAG_FILTER] {
        gl.tex_parameter_i32(glow::TEXTURE_2D, parameter, glow::LINEAR as i32);
    }
    for parameter in [glow::TEXTURE_WRAP_S, glow::TEXTURE_WRAP_T] {
        gl.tex_parameter_i32(glow::TEXTURE_2D, parameter, glow::CLAMP_TO_EDGE as i32);
    }
    let fbo = gl.create_framebuffer().map_err(anyhow::Error::msg)?;
    gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
    gl.framebuffer_texture_2d(
        glow::FRAMEBUFFER,
        glow::COLOR_ATTACHMENT0,
        glow::TEXTURE_2D,
        Some(texture),
        0,
    );
    ensure!(
        gl.check_framebuffer_status(glow::FRAMEBUFFER) == glow::FRAMEBUFFER_COMPLETE,
        "incomplete framebuffer"
    );
    gl.clear_color(0.0, 0.0, 0.0, 1.0);
    gl.clear(glow::COLOR_BUFFER_BIT);
    Ok(Target { fbo, texture })
}
impl Drop for Renderer {
    fn drop(&mut self) {
        // SAFETY: renderer drops before the current SDL context.
        unsafe {
            for target in self
                .targets
                .iter()
                .chain(self.shader_targets.iter().map(|(_, target)| target))
            {
                self.gl.delete_framebuffer(target.fbo);
                self.gl.delete_texture(target.texture);
            }
            for p in self.shaders.iter().flatten() {
                self.gl.delete_program(p.id);
            }
            self.bank.delete(&self.gl);
            self.gl.delete_vertex_array(self.vao);
            self.gl.delete_texture(self.hud_texture);
            self.gl.delete_program(self.program.id);
            self.gl.delete_program(self.program_geo.id);
        }
    }
}
