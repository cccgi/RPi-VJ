//! The GL side: shader programs, render targets, the shader pass and the layer mix.
//! The shader pass feeds the same uniforms as the Pi renderer's `shader_pass`.
use anyhow::{ensure, Result};
use glow::HasContext;
use rpi_vj_native::audio::{AudioFrame, Tempo};
use std::{collections::HashMap, sync::Arc};

const VERTEX: &str = include_str!("../../engine/shaders/fullscreen.vert");
const MIX: &str = "#version 300 es
precision highp float;
uniform sampler2D u_video;
uniform sampler2D u_shader;
uniform float u_video_on;
uniform float u_shader_on;
uniform float u_opacity;
uniform int u_mode;
uniform float u_flash;
uniform mat3 u_flash_turn;
uniform vec3 u_flash_tint;
in vec2 uv;
out vec4 color;
void main() {
    vec3 base = texture(u_video, uv).rgb * u_video_on;
    vec3 top = texture(u_shader, uv).rgb * u_shader_on;
    if (u_flash > 0.5) {
        float luma = dot(top, vec3(0.2126, 0.7152, 0.0722));
        top = clamp(u_flash_turn * top + u_flash_tint * (0.3 + 0.7 * luma), 0.0, 1.0);
    }
    vec3 mixed = base + top * u_opacity;
    if (u_mode == 1) mixed = 1.0 - (1.0 - base) * (1.0 - top * u_opacity);
    if (u_mode == 2) mixed = mix(base, top, u_opacity);
    if (u_mode == 3) mixed = base * mix(vec3(1.0), top, u_opacity);
    color = vec4(mixed, 1.0);
}
";
/// How the shader layer is laid over the video layer, in the order of `u_mode`.
pub const BLENDS: [&str; 4] = ["Add", "Screen", "Mix", "Multiply"];

pub struct Program {
    id: glow::Program,
    uniforms: HashMap<String, glow::UniformLocation>,
}
pub struct Target {
    pub fbo: glow::Framebuffer,
    pub texture: glow::Texture,
    pub size: (u32, u32),
}
/// Everything a shader pass reads.
pub struct Frame<'a> {
    pub time: f32,
    pub dt: f32,
    pub frame: i32,
    pub bpm: f32,
    pub tempo: Tempo,
    pub audio: &'a AudioFrame,
    pub flow: &'a [f32; 4],
    pub spec_flow: &'a [f32; 32],
}
pub struct Mix {
    pub video: Option<glow::Texture>,
    pub shader: Option<glow::Texture>,
    pub opacity: f32,
    pub mode: i32,
    /// From `Reactor::flash`: brightness, colour wash, hue turn, and 1 while STROBE is on.
    pub flash: [f32; 4],
}
pub struct Gfx {
    pub gl: Arc<glow::Context>,
    vao: glow::VertexArray,
    /// The driver takes the bank's `#version 300 es` as it is; otherwise sources are
    /// handed over as desktop GLSL 3.30, which reads the same for what the bank uses.
    es: bool,
    mix: Program,
}
pub fn spectrum_bands(audio: &AudioFrame) -> [f32; 32] {
    std::array::from_fn(|i| audio.spectrum[i * 4..i * 4 + 4].iter().copied().fold(0.0, f32::max))
}
impl Gfx {
    pub fn new(gl: Arc<glow::Context>) -> Result<Self> {
        // SAFETY: the context is current on this thread for the life of the app.
        unsafe {
            let vao = gl.create_vertex_array().map_err(anyhow::Error::msg)?;
            let probe = "#version 300 es\nprecision highp float;\nout vec4 c;\nvoid main(){c=vec4(1.0);}\n";
            let es = compile(&gl, VERTEX, probe).map(|id| gl.delete_program(id)).is_ok();
            let mix = build(&gl, es, MIX, &[])?;
            Ok(Self { gl, vao, es, mix })
        }
    }
    pub fn dialect(&self) -> &'static str {
        if self.es { "GLSL ES 3.00" } else { "GLSL 3.30" }
    }
    /// Compile a prepared fragment shader. `defaults` are its ISF float inputs.
    pub fn program(&self, fragment: &str, defaults: &[(String, f32)]) -> Result<Program> {
        build(&self.gl, self.es, fragment, defaults)
    }
    pub fn delete(&self, program: Program) {
        // SAFETY: the program came from `program` on this context.
        unsafe { self.gl.delete_program(program.id) }
    }
    pub fn target(&self, size: (u32, u32)) -> Result<Target> {
        let gl = &self.gl;
        // SAFETY: as in `new`.
        unsafe {
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
            for (name, value) in [
                (glow::TEXTURE_MIN_FILTER, glow::LINEAR),
                (glow::TEXTURE_MAG_FILTER, glow::LINEAR),
                (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
                (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
            ] {
                gl.tex_parameter_i32(glow::TEXTURE_2D, name, value as i32);
            }
            let fbo = gl.create_framebuffer().map_err(anyhow::Error::msg)?;
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
            gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(texture), 0);
            let complete = gl.check_framebuffer_status(glow::FRAMEBUFFER) == glow::FRAMEBUFFER_COMPLETE;
            gl.clear_color(0.0, 0.0, 0.0, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            gl.bind_texture(glow::TEXTURE_2D, None);
            ensure!(complete, "render target {}x{} is incomplete", size.0, size.1);
            Ok(Target { fbo, texture, size })
        }
    }
    unsafe fn begin(&self, target: &Target, program: &Program) {
        let gl = &self.gl;
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(target.fbo));
        gl.viewport(0, 0, target.size.0 as i32, target.size.1 as i32);
        gl.disable(glow::DEPTH_TEST);
        gl.disable(glow::SCISSOR_TEST);
        gl.disable(glow::BLEND);
        gl.disable(glow::CULL_FACE);
        gl.color_mask(true, true, true, true);
        gl.bind_vertex_array(Some(self.vao));
        gl.use_program(Some(program.id));
    }
    /// Leave the context as the UI painter expects to find it.
    unsafe fn end(&self) {
        let gl = &self.gl;
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        gl.use_program(None);
        gl.bind_vertex_array(None);
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, None);
    }
    /// Draw one frame of a bank shader into `target`.
    pub fn shader_pass(&self, p: &Program, target: &Target, f: &Frame) {
        let gl = &self.gl;
        let size = (target.size.0 as f32, target.size.1 as f32);
        let (time, audio) = (f.time, f.audio);
        // SAFETY: as in `new`; locations belong to `p`.
        unsafe {
            self.begin(target, p);
            gl.clear_color(0.0, 0.0, 0.0, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
            gl.uniform_2_f32(p.uniforms.get("u_resolution"), size.0, size.1);
            gl.uniform_2_f32(p.uniforms.get("RENDERSIZE"), size.0, size.1);
            gl.uniform_1_i32(p.uniforms.get("FRAMEINDEX"), f.frame);
            gl.uniform_4_f32(p.uniforms.get("DATE"), 0.0, 0.0, 0.0, 0.0);
            gl.uniform_4_f32(p.uniforms.get("iMouse"), 0.0, 0.0, 0.0, 0.0);
            for (name, value) in [
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
                ("u_beat_phase", f.tempo.beat),
                ("u_bar_phase", f.tempo.bar),
                ("u_locked", f.tempo.locked),
                ("u_canvas_cos", 1.0),
                ("u_canvas_sin", 0.0),
                ("u_canvas_x", 0.0),
                ("u_canvas_y", 0.0),
                ("u_canvas_cx", size.0 * 0.5),
                ("u_canvas_cy", size.1 * 0.5),
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
                ("uBeatPhase", f.tempo.beat),
                ("uBarPhase", f.tempo.bar),
            ] {
                gl.uniform_1_f32(p.uniforms.get(name), value);
            }
            gl.uniform_1_f32_slice(p.uniforms.get("iChannelTime"), &[time; 4]);
            gl.uniform_1_f32_slice(p.uniforms.get("u_spectrum"), &spectrum_bands(audio));
            gl.uniform_1_f32_slice(p.uniforms.get("u_spec_flow"), f.spec_flow);
            // No input textures yet: a channel a shader reads samples black, not
            // whatever the UI left on the unit.
            for unit in 0..4 {
                gl.active_texture(glow::TEXTURE0 + unit);
                gl.bind_texture(glow::TEXTURE_2D, None);
            }
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            self.end();
        }
    }
    /// Lay the shader layer over the video layer into `target`.
    pub fn mix(&self, target: &Target, m: &Mix) {
        let (gl, p) = (&self.gl, &self.mix);
        // The hue turn about the grey axis, worked out here so the shader only multiplies.
        let angle = m.flash[2] * std::f32::consts::TAU;
        let third = (1.0 - angle.cos()) / 3.0;
        let root = angle.sin() / 3.0f32.sqrt();
        let (same, up, down) = (angle.cos() + third, third + root, third - root);
        let turn = [same, up, down, down, same, up, up, down, same].map(|v| v * m.flash[0]);
        let tint = [same + 0.2 * down + 0.1 * up, up + 0.2 * same + 0.1 * down, down + 0.2 * up + 0.1 * same]
            .map(|v| v * m.flash[1]);
        // SAFETY: as in `new`; locations belong to the mix program.
        unsafe {
            self.begin(target, p);
            for (unit, name, texture) in [(0, "u_video", m.video), (1, "u_shader", m.shader)] {
                gl.active_texture(glow::TEXTURE0 + unit);
                gl.bind_texture(glow::TEXTURE_2D, texture);
                gl.uniform_1_i32(p.uniforms.get(name), unit as i32);
            }
            gl.uniform_1_f32(p.uniforms.get("u_video_on"), f32::from(u8::from(m.video.is_some())));
            gl.uniform_1_f32(p.uniforms.get("u_shader_on"), f32::from(u8::from(m.shader.is_some())));
            gl.uniform_1_f32(p.uniforms.get("u_opacity"), m.opacity);
            gl.uniform_1_i32(p.uniforms.get("u_mode"), m.mode);
            gl.uniform_1_f32(p.uniforms.get("u_flash"), m.flash[3]);
            gl.uniform_matrix_3_f32_slice(p.uniforms.get("u_flash_turn"), false, &turn);
            gl.uniform_3_f32(p.uniforms.get("u_flash_tint"), tint[0], tint[1], tint[2]);
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            gl.active_texture(glow::TEXTURE1);
            gl.bind_texture(glow::TEXTURE_2D, None);
            self.end();
        }
    }
    /// The target's pixels as RGB rows from the top, for a snapshot file.
    pub fn read(&self, target: &Target) -> Vec<u8> {
        let (width, height) = (target.size.0 as usize, target.size.1 as usize);
        let mut rgba = vec![0u8; width * height * 4];
        // SAFETY: the buffer holds exactly the pixels asked for.
        unsafe {
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(target.fbo));
            self.gl.read_pixels(
                0,
                0,
                width as i32,
                height as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut rgba)),
            );
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        }
        let mut rgb = Vec::with_capacity(width * height * 3);
        for row in (0..height).rev() {
            for pixel in rgba[row * width * 4..(row + 1) * width * 4].chunks_exact(4) {
                rgb.extend_from_slice(&pixel[..3]);
            }
        }
        rgb
    }
}
unsafe fn compile(gl: &glow::Context, vertex: &str, fragment: &str) -> Result<glow::Program> {
    let id = gl.create_program().map_err(anyhow::Error::msg)?;
    let result = (|| {
        for (kind, source) in [(glow::VERTEX_SHADER, vertex), (glow::FRAGMENT_SHADER, fragment)] {
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
        ensure!(gl.get_program_link_status(id), "link failed: {}", gl.get_program_info_log(id));
        Ok(id)
    })();
    if result.is_err() {
        gl.delete_program(id);
    }
    result
}
fn build(gl: &glow::Context, es: bool, fragment: &str, defaults: &[(String, f32)]) -> Result<Program> {
    let adapt = |source: &str| {
        if es { source.to_owned() } else { source.replacen("#version 300 es", "#version 330 core", 1) }
    };
    // SAFETY: the context is current; every object made here belongs to it.
    unsafe {
        let id = compile(gl, &adapt(VERTEX), &adapt(fragment))?;
            let mut uniforms = HashMap::new();
            for index in 0..gl.get_active_uniforms(id) {
                if let Some(uniform) = gl.get_active_uniform(id, index) {
                    let name = uniform.name.trim_end_matches("[0]");
                    if let Some(location) = gl.get_uniform_location(id, name) {
                        uniforms.insert(name.to_owned(), location);
                    }
                }
            }
            gl.use_program(Some(id));
            // Every sampler gets its own unit; left alone they would all read unit 0.
            for channel in 0..4 {
                gl.uniform_1_i32(uniforms.get(&format!("iChannel{channel}")), channel);
            }
            for (name, value) in defaults {
                gl.uniform_1_f32(uniforms.get(name), *value);
            }
            gl.use_program(None);
            Ok(Program { id, uniforms })
        }
    }
