//! The GL side: shader programs with their input textures, render targets, the shader
//! pass and the layer mix. The shader pass feeds the same uniforms as the Pi renderer's
//! `shader_pass`.
use anyhow::{ensure, Context, Result};
use glow::HasContext;
use rpi_vj_native::{
    audio::{AudioFrame, Tempo},
    texture::{self, Channel, Kind},
};
use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

const VERTEX: &str = include_str!("../../engine/shaders/fullscreen.vert");
const MIX: &str = "#version 300 es
precision highp float;
uniform sampler2D u_base;
uniform sampler2D u_top;
uniform float u_base_on;
uniform float u_top_on;
uniform float u_opacity;
uniform int u_mode;
uniform int u_fx;
uniform float u_fx_amount;
uniform float u_flash;
uniform mat3 u_flash_turn;
uniform vec3 u_flash_tint;
in vec2 uv;
out vec4 color;
const float TAU = 6.2831853;
const float ASPECT = 16.0 / 9.0;
void main() {
    vec3 base = texture(u_base, uv).rgb * u_base_on;
    vec2 p = uv;
    if (u_fx == 1) {
        // Mirror: left onto right, then top onto bottom as well.
        p.x = 0.5 - abs(p.x - 0.5);
        if (u_fx_amount > 0.5) p.y = 0.5 - abs(p.y - 0.5);
    }
    if (u_fx == 2) {
        vec2 cells = vec2(ASPECT, 1.0) * mix(240.0, 10.0, u_fx_amount);
        p = (floor(p * cells) + 0.5) / cells;
    }
    if (u_fx == 7) {
        vec2 q = (p - 0.5) * vec2(ASPECT, 1.0);
        float slice = TAU / floor(mix(3.0, 12.0, u_fx_amount));
        float angle = abs(mod(atan(q.y, q.x), slice) - slice * 0.5);
        p = vec2(cos(angle), sin(angle)) * length(q) / vec2(ASPECT, 1.0) + 0.5;
    }
    vec3 top = texture(u_top, p).rgb;
    if (u_fx == 3) {
        vec2 shift = vec2(0.03 * u_fx_amount, 0.0);
        top = vec3(texture(u_top, p + shift).r, top.g, texture(u_top, p - shift).b);
    }
    float luma = dot(top, vec3(0.2126, 0.7152, 0.0722));
    if (u_fx == 4) top = mix(top, 1.0 - top, u_fx_amount);
    if (u_fx == 5) top = mix(top, vec3(luma), u_fx_amount);
    if (u_fx == 6) {
        // Turn the colours about the grey axis.
        float angle = u_fx_amount * TAU;
        vec3 axis = vec3(0.57735);
        top = top * cos(angle) + cross(axis, top) * sin(angle) + axis * dot(axis, top) * (1.0 - cos(angle));
    }
    top *= u_top_on;
    if (u_flash > 0.5) {
        top = clamp(u_flash_turn * top + u_flash_tint * (0.3 + 0.7 * luma), 0.0, 1.0);
    }
    vec3 mixed = base + top * u_opacity;
    if (u_mode == 1) mixed = 1.0 - (1.0 - base) * (1.0 - top * u_opacity);
    if (u_mode == 2) mixed = mix(base, top, u_opacity);
    if (u_mode == 3) mixed = base * mix(vec3(1.0), top, u_opacity);
    if (u_mode == 4) mixed = mix(base, max(base, top), u_opacity);
    if (u_mode == 5) mixed = mix(base, abs(base - top), u_opacity);
    color = vec4(clamp(mixed, 0.0, 1.0), 1.0);
}
";
/// How a layer is laid over the layers below it, in the order of `u_mode`.
pub const BLENDS: [&str; 6] = ["Add", "Screen", "Mix", "Multiply", "Lighten", "Difference"];
/// The effects, by `u_fx` number, each with the heading it is listed under.
pub const EFFECTS: [(&str, &str); 26] = [
    ("None", ""),
    ("Mirror", "Distort"),
    ("Pixelate", "Distort"),
    ("RGB split", "Stylize"),
    ("Invert", "Colour"),
    ("Mono", "Colour"),
    ("Hue", "Colour"),
    ("Kaleido", "Distort"),
    ("Blur", "Stylize"),
    ("Edges", "Stylize"),
    ("Posterize", "Colour"),
    ("Wave", "Distort"),
    ("Tile", "Distort"),
    ("Contrast", "Colour"),
    ("Saturate", "Colour"),
    ("Vignette", "Stylize"),
    ("Threshold", "Colour"),
    ("Scanlines", "Stylize"),
    ("Grain", "Stylize"),
    ("Glitch", "Distort"),
    ("Rotate", "Distort"),
    ("Bass zoom", "Audio"),
    ("Beat strobe", "Audio"),
    ("Tunnel", "Distort"),
    ("Colorize", "Colour"),
    ("Flip", "Distort"),
];
/// How a layer changes from one cell to the next, in the order of `u_kind`.
pub const TRANSITIONS: [&str; 10] =
    ["Cut", "Fade", "Wipe", "Wipe down", "Iris", "Zoom", "Dissolve", "Push", "Dip to black", "Flash"];
const TRANSITION: &str = "#version 300 es
precision highp float;
uniform sampler2D u_from;
uniform sampler2D u_to;
uniform int u_kind;
uniform float u_t;
in vec2 uv;
out vec4 color;
void main() {
    float t = smoothstep(0.0, 1.0, u_t);
    vec3 a = texture(u_from, uv).rgb;
    vec3 b = texture(u_to, uv).rgb;
    vec3 c = mix(a, b, t);
    if (u_kind == 2) c = mix(a, b, smoothstep(-0.04, 0.04, t * 1.08 - 0.04 - uv.x));
    if (u_kind == 3) c = mix(a, b, smoothstep(-0.04, 0.04, t * 1.08 - 0.04 - (1.0 - uv.y)));
    if (u_kind == 4) {
        float reach = length((uv - 0.5) * vec2(16.0 / 9.0, 1.0));
        c = mix(a, b, smoothstep(-0.03, 0.03, t * 1.1 - reach));
    }
    if (u_kind == 5) c = mix(texture(u_from, (uv - 0.5) / (1.0 + t * 2.0) + 0.5).rgb, b, t);
    if (u_kind == 6) {
        float grain = fract(sin(dot(floor(uv * vec2(160.0, 90.0)), vec2(12.9898, 78.233))) * 43758.5453);
        c = grain < t ? b : a;
    }
    if (u_kind == 7) c = uv.x < 1.0 - t ? texture(u_from, vec2(uv.x + t, uv.y)).rgb : texture(u_to, vec2(uv.x - (1.0 - t), uv.y)).rgb;
    if (u_kind == 8) c = t < 0.5 ? a * (1.0 - 2.0 * t) : b * (2.0 * t - 1.0);
    if (u_kind == 9) c = mix(a, b, t) + vec3(1.0 - abs(2.0 * t - 1.0));
    color = vec4(clamp(c, 0.0, 1.0), 1.0);
}
";
const GRADE: &str = "#version 300 es
precision highp float;
uniform sampler2D u_src;
uniform float u_hue;
uniform float u_saturation;
uniform float u_brightness;
uniform float u_contrast;
uniform float u_gamma;
uniform float u_invert;
uniform float u_tile[7];
uniform vec4 u_colorize;
in vec2 uv;
out vec4 color;
const float ASPECT = 16.0 / 9.0;
void main() {
    vec3 c = texture(u_src, uv).rgb;
    int tiling = int(u_tile[0] + 0.5);
    if (tiling > 0) {
        // The finished picture laid out as tiles: turned, zoomed and moved as a whole.
        vec2 q = mat2(u_tile[1], -u_tile[2], u_tile[2], u_tile[1]) * ((uv - 0.5) * vec2(ASPECT, 1.0)) / max(u_tile[5], 0.01);
        vec2 p = q / vec2(ASPECT, 1.0) + 0.5 + vec2(u_tile[3] / ASPECT, u_tile[4]);
        vec2 f = fract(p);
        if (tiling == 1) c = texture(u_src, f).rgb;
        if (tiling == 2) c = texture(u_src, 1.0 - abs(1.0 - mod(p, 2.0))).rgb;
        if (tiling == 3) {
            // Near a tile's edge the picture gives way to itself shifted by half a tile,
            // which has no edge there, so neighbouring tiles meet without a seam.
            float reach = max(u_tile[6], 0.001);
            float wx = smoothstep(0.0, reach, f.x) * smoothstep(0.0, reach, 1.0 - f.x);
            float wy = smoothstep(0.0, reach, f.y) * smoothstep(0.0, reach, 1.0 - f.y);
            vec3 shifted_x = texture(u_src, vec2(fract(f.x + 0.5), f.y)).rgb;
            vec3 shifted_y = texture(u_src, vec2(f.x, fract(f.y + 0.5))).rgb;
            vec3 shifted_both = texture(u_src, fract(f + 0.5)).rgb;
            c = mix(mix(shifted_both, shifted_y, wx), mix(shifted_x, texture(u_src, f).rgb, wx), wy);
        }
    }
    float angle = u_hue * 6.2831853;
    vec3 axis = vec3(0.57735);
    c = c * cos(angle) + cross(axis, c) * sin(angle) + axis * dot(axis, c) * (1.0 - cos(angle));
    c = mix(vec3(dot(c, vec3(0.2126, 0.7152, 0.0722))), c, u_saturation);
    c = ((c - 0.5) * u_contrast + 0.5) * u_brightness;
    c = pow(max(c, 0.0), vec3(1.0 / max(u_gamma, 0.05)));
    // Colorize: the picture's light and dark kept, its colours swapped for one tint.
    c = mix(c, u_colorize.rgb * dot(c, vec3(0.2126, 0.7152, 0.0722)) * 1.6, u_colorize.a);
    c = mix(c, 1.0 - c, u_invert);
    color = vec4(clamp(c, 0.0, 1.0), 1.0);
}
";
const FX: &str = "#version 300 es
precision highp float;
uniform sampler2D u_src;
uniform int u_fx;
uniform float u_amount;
uniform float u_time;
uniform float u_beat;
uniform float u_bass;
uniform vec2 u_px;
in vec2 uv;
out vec4 color;
const float TAU = 6.2831853;
const float ASPECT = 16.0 / 9.0;
void main() {
    float a = u_amount;
    vec2 p = uv;
    if (u_fx == 1) {
        p.x = 0.5 - abs(p.x - 0.5);
        if (a > 0.5) p.y = 0.5 - abs(p.y - 0.5);
    }
    if (u_fx == 2) {
        vec2 cells = vec2(ASPECT, 1.0) * mix(240.0, 10.0, a);
        p = (floor(p * cells) + 0.5) / cells;
    }
    if (u_fx == 7) {
        vec2 q = (p - 0.5) * vec2(ASPECT, 1.0);
        float slice = TAU / floor(mix(3.0, 12.0, a));
        float angle = abs(mod(atan(q.y, q.x), slice) - slice * 0.5);
        p = vec2(cos(angle), sin(angle)) * length(q) / vec2(ASPECT, 1.0) + 0.5;
    }
    if (u_fx == 11) {
        p.x += sin(p.y * 20.0 + u_time * 2.0) * 0.05 * a;
        p.y += cos(p.x * 16.0 + u_time * 1.6) * 0.05 * a;
    }
    if (u_fx == 12) p = fract(p * floor(mix(1.0, 6.0, a) + 0.5));
    if (u_fx == 19) {
        float band = floor(p.y * 24.0);
        float chance = fract(sin(band * 91.7 + floor(u_time * 12.0) * 13.3) * 43758.5);
        if (chance < a * 0.6) p.x = fract(p.x + (chance - 0.3) * 0.4 * a);
    }
    if (u_fx == 20) {
        float angle = a * TAU;
        vec2 q = mat2(cos(angle), -sin(angle), sin(angle), cos(angle)) * ((p - 0.5) * vec2(ASPECT, 1.0));
        p = q / vec2(ASPECT, 1.0) + 0.5;
    }
    if (u_fx == 21) p = (p - 0.5) / (1.0 + a * 0.6 * u_bass) + 0.5;
    if (u_fx == 23) {
        vec2 q = (p - 0.5) * vec2(ASPECT, 1.0);
        p = vec2(atan(q.y, q.x) / TAU + 0.5, fract(mix(0.15, 0.6, a) / max(length(q), 0.02) + u_time * 0.2));
    }
    if (u_fx == 25) {
        p.x = 1.0 - p.x;
        if (a > 0.5) p.y = 1.0 - p.y;
    }
    vec3 c = texture(u_src, p).rgb;
    if (u_fx == 3) {
        vec2 shift = vec2(0.03 * a, 0.0);
        c = vec3(texture(u_src, p + shift).r, c.g, texture(u_src, p - shift).b);
    }
    if (u_fx == 8) {
        vec3 sum = vec3(0.0);
        for (int x = -2; x <= 2; x++)
            for (int y = -2; y <= 2; y++)
                sum += texture(u_src, p + vec2(float(x), float(y)) * u_px * a * 6.0).rgb;
        c = sum / 25.0;
    }
    if (u_fx == 9) {
        vec3 across = texture(u_src, p + vec2(u_px.x, 0.0)).rgb - texture(u_src, p - vec2(u_px.x, 0.0)).rgb;
        vec3 down = texture(u_src, p + vec2(0.0, u_px.y)).rgb - texture(u_src, p - vec2(0.0, u_px.y)).rgb;
        c = mix(c, vec3(length(across) + length(down)) * 3.0, a);
    }
    float luma = dot(c, vec3(0.2126, 0.7152, 0.0722));
    if (u_fx == 4) c = mix(c, 1.0 - c, a);
    if (u_fx == 5) c = mix(c, vec3(luma), a);
    if (u_fx == 6) {
        float angle = a * TAU;
        vec3 axis = vec3(0.57735);
        c = c * cos(angle) + cross(axis, c) * sin(angle) + axis * dot(axis, c) * (1.0 - cos(angle));
    }
    if (u_fx == 10) {
        float steps = mix(16.0, 2.0, a);
        c = floor(c * steps + 0.5) / steps;
    }
    if (u_fx == 13) c = (c - 0.5) * (1.0 + a * 2.0) + 0.5;
    if (u_fx == 14) c = mix(vec3(luma), c, 1.0 + a * 2.0);
    if (u_fx == 15) c *= 1.0 - a * smoothstep(0.3, 0.9, length(uv - 0.5) * 1.4);
    if (u_fx == 16) c = vec3(step(mix(0.15, 0.85, a), luma));
    if (u_fx == 17) c *= 1.0 - a * 0.7 * step(0.5, fract(uv.y / u_px.y * 0.25));
    if (u_fx == 18) c += (fract(sin(dot(uv / u_px + u_time * 60.0, vec2(12.9898, 78.233))) * 43758.5453) - 0.5) * a * 0.6;
    if (u_fx == 22) c *= mix(1.0, 0.15 + 1.6 * u_beat, a);
    if (u_fx == 24) c = vec3(luma) * (0.5 + 0.5 * cos(TAU * (a + vec3(0.0, 0.33, 0.67)))) * 1.6;
    color = vec4(clamp(c, 0.0, 1.0), 1.0);
}
";

/// An input texture of a shader: one upload shared by every shader that binds the same
/// file with the same sampling.
pub struct Texture {
    id: glow::Texture,
    target: u32,
    size: [f32; 3],
}
pub struct Program {
    id: glow::Program,
    uniforms: HashMap<String, glow::UniformLocation>,
    channels: [Option<Rc<Texture>>; 4],
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
    /// The picture turned (cos, sin) and shifted (x, y in picture heights), and zoomed.
    pub canvas: [f32; 4],
    pub zoom: f32,
}
/// One step of the layer stack: `top` over `base`.
pub struct Mix {
    pub base: Option<glow::Texture>,
    pub top: Option<glow::Texture>,
    pub opacity: f32,
    pub mode: i32,
    pub fx: i32,
    pub fx_amount: f32,
    /// From `Reactor::flash`: brightness, colour wash, hue turn, and 1 while STROBE is on.
    pub flash: [f32; 4],
}
impl Mix {
    /// `base` passed on as it is.
    pub fn copy(base: glow::Texture) -> Self {
        Self { base: Some(base), top: None, opacity: 0.0, mode: 0, fx: 0, fx_amount: 0.0, flash: [1.0, 0.0, 0.0, 0.0] }
    }
}
pub struct Gfx {
    pub gl: Arc<glow::Context>,
    vao: glow::VertexArray,
    /// The driver takes the bank's `#version 300 es` as it is; otherwise sources are
    /// handed over as desktop GLSL 3.30, which reads the same for what the bank uses.
    es: bool,
    mix: Program,
    fx: Program,
    grade: Program,
    cross: Program,
    textures: RefCell<HashMap<String, Rc<Texture>>>,
}
pub fn spectrum_bands(audio: &AudioFrame) -> [f32; 32] {
    std::array::from_fn(|i| audio.spectrum[i * 4..i * 4 + 4].iter().copied().fold(0.0, f32::max))
}
fn gl_target(kind: Kind) -> u32 {
    match kind {
        Kind::Flat => glow::TEXTURE_2D,
        Kind::Cube => glow::TEXTURE_CUBE_MAP,
        Kind::Volume => glow::TEXTURE_3D,
    }
}
impl Gfx {
    pub fn new(gl: Arc<glow::Context>) -> Result<Self> {
        // SAFETY: the context is current on this thread for the life of the app.
        unsafe {
            let vao = gl.create_vertex_array().map_err(anyhow::Error::msg)?;
            let probe = "#version 300 es\nprecision highp float;\nout vec4 c;\nvoid main(){c=vec4(1.0);}\n";
            let es = compile(&gl, VERTEX, probe).map(|id| gl.delete_program(id)).is_ok();
            let mix = build(&gl, es, MIX, &[])?;
            let fx = build(&gl, es, FX, &[])?;
            let grade = build(&gl, es, GRADE, &[])?;
            let cross = build(&gl, es, TRANSITION, &[])?;
            Ok(Self { gl, vao, es, mix, fx, grade, cross, textures: RefCell::default() })
        }
    }
    pub fn dialect(&self) -> &'static str {
        if self.es { "GLSL ES 3.00" } else { "GLSL 3.30" }
    }
    /// Compile a prepared fragment shader. `defaults` are its ISF float inputs and
    /// `channels` the input textures its `iChannelN` samplers read.
    pub fn program(&self, fragment: &str, defaults: &[(String, f32)], channels: &[Channel]) -> Result<Program> {
        let mut program = build(&self.gl, self.es, fragment, defaults)?;
        let mut resolution = [0.0f32; 12];
        for channel in channels {
            let index = usize::from(channel.channel).min(3);
            match self.texture(channel) {
                Ok(texture) => {
                    resolution[index * 3..index * 3 + 3].copy_from_slice(&texture.size);
                    program.channels[index] = Some(texture);
                }
                Err(error) => {
                    self.delete(program);
                    return Err(error);
                }
            }
        }
        // SAFETY: as in `new`; the location belongs to this program.
        unsafe {
            self.gl.use_program(Some(program.id));
            self.gl.uniform_3_f32_slice(program.uniforms.get("iChannelResolution"), &resolution);
            self.gl.use_program(None);
        }
        Ok(program)
    }
    pub fn delete(&self, program: Program) {
        // SAFETY: the program came from `program` on this context.
        unsafe { self.gl.delete_program(program.id) }
    }
    pub fn discard(&self, target: Target) {
        // SAFETY: both objects came from `target` on this context.
        unsafe {
            self.gl.delete_framebuffer(target.fbo);
            self.gl.delete_texture(target.texture);
        }
    }
    /// The upload of one input texture, made on first use.
    fn texture(&self, channel: &Channel) -> Result<Rc<Texture>> {
        let key = format!("{}|{:?}|{:?}|{:?}|{}", channel.path.display(), channel.kind, channel.filter, channel.wrap, channel.vflip);
        if let Some(found) = self.textures.borrow().get(&key) {
            return Ok(Rc::clone(found));
        }
        let layers = texture::load(&channel.key())?;
        let first = layers.first().context("texture has no data")?;
        let (gl, target) = (&self.gl, gl_target(channel.kind));
        let format = |channels: u8| if channels == 1 { (glow::R8, glow::RED) } else { (glow::RGBA8, glow::RGBA) };
        // SAFETY: as in `new`; every buffer holds exactly the texels its size declares.
        let id = unsafe {
            let id = gl.create_texture().map_err(anyhow::Error::msg)?;
            gl.bind_texture(target, Some(id));
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            for (index, layer) in layers.iter().enumerate() {
                let (internal, external) = format(layer.channels);
                let data = glow::PixelUnpackData::Slice(Some(&layer.data));
                let (width, height, depth) = (layer.width as i32, layer.height as i32, layer.depth as i32);
                match channel.kind {
                    Kind::Flat => {
                        gl.tex_image_2d(target, 0, internal as i32, width, height, 0, external, glow::UNSIGNED_BYTE, data)
                    }
                    Kind::Cube => gl.tex_image_2d(
                        glow::TEXTURE_CUBE_MAP_POSITIVE_X + index as u32,
                        0,
                        internal as i32,
                        width,
                        height,
                        0,
                        external,
                        glow::UNSIGNED_BYTE,
                        data,
                    ),
                    Kind::Volume => gl.tex_image_3d(
                        target,
                        0,
                        internal as i32,
                        width,
                        height,
                        depth,
                        0,
                        external,
                        glow::UNSIGNED_BYTE,
                        data,
                    ),
                }
            }
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 4);
            let (min, mag) = channel.filter.gl();
            gl.tex_parameter_i32(target, glow::TEXTURE_MIN_FILTER, min as i32);
            gl.tex_parameter_i32(target, glow::TEXTURE_MAG_FILTER, mag as i32);
            for axis in [glow::TEXTURE_WRAP_S, glow::TEXTURE_WRAP_T, glow::TEXTURE_WRAP_R] {
                gl.tex_parameter_i32(target, axis, channel.wrap.gl() as i32);
            }
            if channel.filter.mipmaps() {
                gl.generate_mipmap(target);
            }
            gl.bind_texture(target, None);
            id
        };
        let made = Rc::new(Texture { id, target, size: [first.width as f32, first.height as f32, first.depth as f32] });
        self.textures.borrow_mut().insert(key, Rc::clone(&made));
        Ok(made)
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
    pub fn clear(&self, target: &Target) {
        // SAFETY: as in `new`.
        unsafe {
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(target.fbo));
            self.gl.disable(glow::SCISSOR_TEST);
            self.gl.color_mask(true, true, true, true);
            self.gl.clear_color(0.0, 0.0, 0.0, 1.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, None);
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
                ("u_canvas_cos", f.canvas[0]),
                ("u_canvas_sin", f.canvas[1]),
                ("u_canvas_x", f.canvas[2] * size.1),
                ("u_canvas_y", f.canvas[3] * size.1),
                ("u_canvas_zoom", f.zoom),
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
            // Each unit gets its input texture, or nothing: a channel without one
            // samples black, not whatever the UI left on the unit.
            for (unit, channel) in p.channels.iter().enumerate() {
                gl.active_texture(glow::TEXTURE0 + unit as u32);
                gl.bind_texture(glow::TEXTURE_2D, None);
                if let Some(texture) = channel {
                    gl.bind_texture(texture.target, Some(texture.id));
                }
            }
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            for (unit, channel) in p.channels.iter().enumerate() {
                if let Some(texture) = channel {
                    gl.active_texture(glow::TEXTURE0 + unit as u32);
                    gl.bind_texture(texture.target, None);
                }
            }
            self.end();
        }
    }
    /// One step of the layer stack into `target`.
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
            for (unit, name, texture) in [(0, "u_base", m.base), (1, "u_top", m.top)] {
                gl.active_texture(glow::TEXTURE0 + unit);
                gl.bind_texture(glow::TEXTURE_2D, texture);
                gl.uniform_1_i32(p.uniforms.get(name), unit as i32);
            }
            gl.uniform_1_f32(p.uniforms.get("u_base_on"), f32::from(u8::from(m.base.is_some())));
            gl.uniform_1_f32(p.uniforms.get("u_top_on"), f32::from(u8::from(m.top.is_some())));
            gl.uniform_1_f32(p.uniforms.get("u_opacity"), m.opacity);
            gl.uniform_1_i32(p.uniforms.get("u_mode"), m.mode);
            gl.uniform_1_i32(p.uniforms.get("u_fx"), m.fx);
            gl.uniform_1_f32(p.uniforms.get("u_fx_amount"), m.fx_amount);
            gl.uniform_1_f32(p.uniforms.get("u_flash"), m.flash[3]);
            gl.uniform_matrix_3_f32_slice(p.uniforms.get("u_flash_turn"), false, &turn);
            gl.uniform_3_f32(p.uniforms.get("u_flash_tint"), tint[0], tint[1], tint[2]);
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            gl.active_texture(glow::TEXTURE1);
            gl.bind_texture(glow::TEXTURE_2D, None);
            self.end();
        }
    }
    /// One step of a transition: `from` giving way to `to`, `t` from 0 to 1.
    pub fn transition(&self, target: &Target, from: glow::Texture, to: glow::Texture, kind: i32, t: f32) {
        let (gl, p) = (&self.gl, &self.cross);
        // SAFETY: as in `new`; locations belong to the transition program.
        unsafe {
            self.begin(target, p);
            for (unit, name, texture) in [(0, "u_from", from), (1, "u_to", to)] {
                gl.active_texture(glow::TEXTURE0 + unit);
                gl.bind_texture(glow::TEXTURE_2D, Some(texture));
                gl.uniform_1_i32(p.uniforms.get(name), unit as i32);
            }
            gl.uniform_1_i32(p.uniforms.get("u_kind"), kind);
            gl.uniform_1_f32(p.uniforms.get("u_t"), t.clamp(0.0, 1.0));
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            gl.active_texture(glow::TEXTURE1);
            gl.bind_texture(glow::TEXTURE_2D, None);
            self.end();
        }
    }
    /// Colour work on one picture: hue turn (0..1), saturation, brightness, contrast,
    /// gamma, and invert (0 or 1), in that order of `grade`.
    /// `tile` lays the picture out as tiles first: kind (0 none, 1 repeat, 2 mirrored,
    /// 3 seamless), turn as cos and sin, move across and up in picture heights, zoom,
    /// and how far in from a tile's edge the seamless blend reaches.
    /// `colorize` is a tint (red, green, blue) and how far the picture is taken to it.
    pub fn grade(&self, target: &Target, source: glow::Texture, grade: [f32; 6], tile: [f32; 7], colorize: [f32; 4]) {
        let (gl, p) = (&self.gl, &self.grade);
        // SAFETY: as in `new`; locations belong to the grade program.
        unsafe {
            self.begin(target, p);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(source));
            gl.uniform_1_i32(p.uniforms.get("u_src"), 0);
            for (name, value) in ["u_hue", "u_saturation", "u_brightness", "u_contrast", "u_gamma", "u_invert"].into_iter().zip(grade) {
                gl.uniform_1_f32(p.uniforms.get(name), value);
            }
            gl.uniform_1_f32_slice(p.uniforms.get("u_tile"), &tile);
            gl.uniform_4_f32(p.uniforms.get("u_colorize"), colorize[0], colorize[1], colorize[2], colorize[3]);
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            self.end();
        }
    }
    /// One effect: `source` through effect number `fx` into `target`. `beat` and `bass`
    /// drive the effects that follow the music.
    #[allow(clippy::too_many_arguments)]
    pub fn effect(&self, target: &Target, source: glow::Texture, fx: i32, amount: f32, time: f32, beat: f32, bass: f32) {
        let (gl, p) = (&self.gl, &self.fx);
        // SAFETY: as in `new`; locations belong to the effect program.
        unsafe {
            self.begin(target, p);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(source));
            gl.uniform_1_i32(p.uniforms.get("u_src"), 0);
            gl.uniform_1_i32(p.uniforms.get("u_fx"), fx);
            gl.uniform_1_f32(p.uniforms.get("u_amount"), amount);
            gl.uniform_1_f32(p.uniforms.get("u_time"), time);
            gl.uniform_1_f32(p.uniforms.get("u_beat"), beat);
            gl.uniform_1_f32(p.uniforms.get("u_bass"), bass);
            gl.uniform_2_f32(p.uniforms.get("u_px"), 1.0 / target.size.0 as f32, 1.0 / target.size.1 as f32);
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
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
        Ok(Program { id, uniforms, channels: [None, None, None, None] })
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
