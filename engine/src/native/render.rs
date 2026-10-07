use crate::{
    audio::AudioFrame,
    control::{BlendMode, State},
    shader,
    show::Show,
};
use anyhow::{ensure, Result};
use font8x8::UnicodeFonts;
use glow::HasContext;
use std::collections::HashMap;

pub struct Target {
    pub fbo: glow::Framebuffer,
    pub texture: glow::Texture,
}
struct Program {
    id: glow::Program,
    uniforms: HashMap<String, glow::UniformLocation>,
}
impl Program {
    unsafe fn new(gl: &glow::Context, fragment: &str) -> Result<Self> {
        let id = gl.create_program().map_err(anyhow::Error::msg)?;
        let result = (|| {
            for (kind, source) in [
                (
                    glow::VERTEX_SHADER,
                    include_str!("../../shaders/fullscreen.vert"),
                ),
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
            for index in 0..gl.get_active_uniforms(id) {
                if let Some(u) = gl.get_active_uniform(id, index) {
                    let name = u.name.trim_end_matches("[0]");
                    if let Some(loc) = gl.get_uniform_location(id, name) {
                        uniforms.insert(name.to_owned(), loc);
                    }
                }
            }
            Ok(Self { id, uniforms })
        })();
        if result.is_err() {
            gl.delete_program(id);
        }
        result
    }
    unsafe fn float(&self, gl: &glow::Context, name: &str, v: f32) {
        gl.uniform_1_f32(self.uniforms.get(name), v);
    }
    unsafe fn int(&self, gl: &glow::Context, name: &str, v: i32) {
        gl.uniform_1_i32(self.uniforms.get(name), v);
    }
    unsafe fn size(&self, gl: &glow::Context, name: &str, w: f32, h: f32) {
        gl.uniform_2_f32(self.uniforms.get(name), w, h);
    }
}
pub struct Renderer {
    gl: glow::Context,
    program: Program,
    vao: glow::VertexArray,
    pub targets: [Target; 2],
    shader_target: Target,
    shaders: Vec<Option<Program>>,
    shader_sources: Vec<Option<shader::PreparedShader>>,
    errors: Vec<Option<String>>,
    previous_time: Option<f32>,
    flow: [f32; 4],
    spec_flow: [f32; 32],
    frame: i32,
    clip_layouts: Vec<bool>,
    hud_texture: glow::Texture,
    hud_pixels: Vec<u8>,
    hud_stamp: f32,
    render_size: (u32, u32),
}
pub fn render_size() -> (u32, u32) {
    let width = std::env::var("RPI_VJ_RENDER_WIDTH").ok().and_then(|v| v.parse().ok()).unwrap_or(1920);
    let height = std::env::var("RPI_VJ_RENDER_HEIGHT").ok().and_then(|v| v.parse().ok()).unwrap_or(1080);
    if (320..=3840).contains(&width) && (240..=2160).contains(&height) { (width, height) } else { (1920, 1080) }
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
            let program = Program::new(&gl, include_str!("../../shaders/program.frag"))?;
            let vao = gl.create_vertex_array().map_err(anyhow::Error::msg)?;
            let render_size = render_size();
            let targets = [target(&gl, render_size)?, target(&gl, render_size)?];
            let shader_target = target(&gl, render_size)?;
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
            let mut shaders = Vec::new();
            let mut shader_sources = Vec::new();
            let mut errors = Vec::new();
            for shader in &show.shaders {
                match shader::load(&shader.path) {
                    Ok(source) => {
                        shaders.push(None);
                        shader_sources.push(Some(source));
                        errors.push(None);
                    }
                    Err(e) => {
                        eprintln!("Shader disabled [{}]: {e:#}", shader.label);
                        shaders.push(None);
                        shader_sources.push(None);
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
                vao,
                targets,
                shader_target,
                shaders,
                shader_sources,
                errors,
                previous_time: None,
                flow: [0.0; 4],
                spec_flow: [0.0; 32],
                frame: 0,
                clip_layouts: show.clips.iter().map(|c| c.quad_mirror).collect(),
                hud_texture,
                hud_pixels: vec![0; 768 * 16],
                hud_stamp: -1.0,
                render_size,
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
            match Program::new(&self.gl, &source.source) {
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
            *clock += dt * (0.15 + energy * 2.0);
        }
        let spectrum: [f32; 32] = std::array::from_fn(|i| {
            audio.spectrum[i * 4..i * 4 + 4]
                .iter()
                .copied()
                .fold(0.0, f32::max)
        });
        for (clock, energy) in self.spec_flow.iter_mut().zip(spectrum) {
            *clock += dt * (0.05 + energy * 2.0);
        }
        if state.shader && !state.blackout {
            if let Some(index) = state.selected_shader { self.compile_selected(index); }
        }
        let shader = state
            .selected_shader
            .and_then(|i| self.shaders.get(i))
            .and_then(Option::as_ref)
            .filter(|_| state.shader && !state.blackout);
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
            gl.disable(glow::DEPTH_TEST);
            gl.disable(glow::SCISSOR_TEST);
            gl.disable(glow::BLEND);
            gl.disable(glow::CULL_FACE);
            gl.color_mask(true, true, true, true);
            gl.bind_vertex_array(Some(self.vao));
            if let Some(p) = shader {
                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.shader_target.fbo));
                gl.viewport(0, 0, self.render_size.0 as i32, self.render_size.1 as i32);
                gl.use_program(Some(p.id));
                gl.clear_color(0.0, 0.0, 0.0, 1.0);
                gl.clear(glow::COLOR_BUFFER_BIT);
                p.size(gl, "u_resolution", self.render_size.0 as f32, self.render_size.1 as f32);
                p.size(gl, "RENDERSIZE", self.render_size.0 as f32, self.render_size.1 as f32);
                p.int(gl, "FRAMEINDEX", self.frame);
                gl.uniform_4_f32(p.uniforms.get("DATE"), 0.0, 0.0, 0.0, 0.0);
                let beat = (time * state.bpm / 60.0).fract();
                let bar = (time * state.bpm / 240.0).fract();
                for (name, v) in [
                    ("u_time", time),
                    ("TIME", time),
                    ("TIMEDELTA", dt),
                    ("u_bass", audio.bass),
                    ("u_mid", audio.mid),
                    ("u_treble", audio.treble),
                    ("u_volume", audio.volume),
                    ("u_onset", audio.onset),
                    ("u_beat_onset", audio.onset),
                    ("u_energy", audio.volume),
                    ("u_kick", audio.onset),
                    ("u_bpm", state.bpm),
                    ("u_sub", audio.bass),
                    ("u_low_mid", audio.mid),
                    ("u_high_mid", audio.mid),
                    ("u_hit_low", audio.onset * audio.bass),
                    ("u_hit_mid", audio.onset * audio.mid),
                    ("u_hit_high", audio.onset * audio.treble),
                    ("u_beat_phase", beat),
                    ("u_bar_phase", bar),
                    ("u_locked", 0.0),
                    ("u_flow", self.flow[0]),
                    ("u_flow_low", self.flow[1]),
                    ("u_flow_mid", self.flow[2]),
                    ("u_flow_high", self.flow[3]),
                    ("uSubBass", audio.bass),
                    ("uBass", audio.bass),
                    ("uLowMid", audio.mid),
                    ("uHighMid", audio.mid),
                    ("uTreble", audio.treble),
                    ("uEnergy", audio.volume),
                    ("uKick", audio.onset),
                    ("uBPM", state.bpm),
                    ("uBeatPhase", beat),
                    ("uBarPhase", bar),
                ] {
                    p.float(gl, name, v);
                }
                gl.uniform_1_f32_slice(p.uniforms.get("iChannelTime"), &[time; 4]);
                gl.uniform_1_f32_slice(p.uniforms.get("u_spectrum"), &spectrum);
                gl.uniform_1_f32_slice(p.uniforms.get("u_spec_flow"), &self.spec_flow);
                gl.draw_arrays(glow::TRIANGLES, 0, 3);
            }
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            gl.viewport(0, 0, size.0 as i32, size.1 as i32);
            gl.use_program(Some(self.program.id));
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.targets[active.min(1)].texture));
            gl.active_texture(glow::TEXTURE1);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.shader_target.texture));
            let p = &self.program;
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
                if state.playing.is_some() { 1.0 } else { 0.0 },
            );
            p.float(
                gl,
                "u_shader_opacity",
                if shader.is_some() { state.opacity } else { 0.0 },
            );
            p.float(gl, "u_fx_time", time);
            p.float(gl, "u_fx_amount", state.video_effect_amount);
            p.float(gl, "u_sfx_amount", state.shader_effect_amount);
            p.int(gl, "u_fx", state.video_effect as i32);
            p.int(gl, "u_sfx", state.shader_effect as i32);
            p.int(gl, "u_blackout", i32::from(state.blackout));
            p.int(gl, "u_invert_order", 0);
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
            p.int(gl, "u_deck_flip_y", 0);
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
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
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
unsafe fn target(gl: &glow::Context, size: (u32, u32)) -> Result<Target> {
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
                .chain(std::iter::once(&self.shader_target))
            {
                self.gl.delete_framebuffer(target.fbo);
                self.gl.delete_texture(target.texture);
            }
            for p in self.shaders.iter().flatten() {
                self.gl.delete_program(p.id);
            }
            self.gl.delete_vertex_array(self.vao);
            self.gl.delete_texture(self.hud_texture);
            self.gl.delete_program(self.program.id);
        }
    }
}
