//! GPU half of shader input textures: each asset is uploaded once, sampler settings are
//! GLES 3 sampler objects owned by the binding. Decode and accounting are in `crate::texture`.
use crate::texture::{self, AssetKey, Budget, Channel, Filter, Kind, Pixels, Wrap, MAX_CHANNELS};
use anyhow::{anyhow, ensure, Result};
use glow::HasContext;
use std::collections::HashMap;

/// Channels use units 0..3; the deck video for shaders declaring `u_video` comes next.
pub(super) const VIDEO_UNIT: u32 = MAX_CHANNELS as u32;

const _: () = {
    assert!(texture::GL_NEAREST == glow::NEAREST && texture::GL_LINEAR == glow::LINEAR);
    assert!(texture::GL_LINEAR_MIPMAP_LINEAR == glow::LINEAR_MIPMAP_LINEAR);
    assert!(texture::GL_REPEAT == glow::REPEAT && texture::GL_CLAMP_TO_EDGE == glow::CLAMP_TO_EDGE);
};

pub(super) fn target(kind: Kind) -> u32 {
    match kind {
        Kind::Flat => glow::TEXTURE_2D,
        Kind::Cube => glow::TEXTURE_CUBE_MAP,
        Kind::Volume => glow::TEXTURE_3D,
    }
}
/// Active-uniform type a shader must have declared for a channel of this kind.
pub(super) fn sampler_type(kind: Kind) -> u32 {
    match kind {
        Kind::Flat => glow::SAMPLER_2D,
        Kind::Cube => glow::SAMPLER_CUBE,
        Kind::Volume => glow::SAMPLER_3D,
    }
}
/// Texture target a declared sampler type reads from, for the types a channel may have.
pub(super) fn sampler_target(sampler_type: u32) -> Option<u32> {
    match sampler_type {
        glow::SAMPLER_2D => Some(glow::TEXTURE_2D),
        glow::SAMPLER_CUBE => Some(glow::TEXTURE_CUBE_MAP),
        glow::SAMPLER_3D => Some(glow::TEXTURE_3D),
        _ => None,
    }
}

/// One channel of one shader, ready to bind.
#[derive(Clone, Copy)]
pub(super) struct Bound {
    pub kind: Kind,
    pub texture: glow::Texture,
    pub sampler: glow::Sampler,
    /// `iChannelResolution` value.
    pub size: [f32; 3],
}
pub(super) type Channels = [Option<Bound>; MAX_CHANNELS];

struct Asset {
    texture: glow::Texture,
    size: [f32; 3],
}
pub(super) struct Bank {
    assets: HashMap<AssetKey, Result<Asset, String>>,
    samplers: HashMap<(Filter, Wrap), glow::Sampler>,
    budget: Budget,
}
impl Bank {
    /// Decode and upload every distinct asset the bindings name, in first-use order, up to
    /// the memory cap. A failure is remembered per asset and surfaces from `resolve`, so
    /// one bad file disables only the shaders that use it.
    pub(super) unsafe fn load(gl: &glow::Context, bindings: &[&Channel]) -> Self {
        let mut order: Vec<AssetKey> = Vec::new();
        let mut mipmaps: HashMap<AssetKey, bool> = HashMap::new();
        for binding in bindings {
            let key = binding.key();
            let wanted = mipmaps.entry(key.clone()).or_insert_with(|| {
                order.push(key);
                false
            });
            // One upload serves every filter: it carries mipmaps when any binding wants them.
            *wanted |= binding.filter.mipmaps();
        }
        let mut bank = Self {
            assets: HashMap::new(),
            samplers: HashMap::new(),
            budget: Budget::from_env(),
        };
        let mut failed = 0;
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        for key in order {
            let result = bank.upload(gl, &key, mipmaps[&key]).map_err(|error| {
                failed += 1;
                let message = format!("{error:#}");
                eprintln!("texture unavailable: {message}");
                message
            });
            bank.assets.insert(key, result);
        }
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 4);
        for kind in [Kind::Flat, Kind::Cube, Kind::Volume] {
            gl.bind_texture(target(kind), None);
        }
        for binding in bindings {
            let settings = (binding.filter, binding.wrap);
            if bank.samplers.contains_key(&settings) {
                continue;
            }
            match gl.create_sampler() {
                Ok(sampler) => {
                    let (min, mag) = binding.filter.gl();
                    gl.sampler_parameter_i32(sampler, glow::TEXTURE_MIN_FILTER, min as i32);
                    gl.sampler_parameter_i32(sampler, glow::TEXTURE_MAG_FILTER, mag as i32);
                    for axis in [glow::TEXTURE_WRAP_S, glow::TEXTURE_WRAP_T, glow::TEXTURE_WRAP_R] {
                        gl.sampler_parameter_i32(sampler, axis, binding.wrap.gl() as i32);
                    }
                    bank.samplers.insert(settings, sampler);
                }
                Err(error) => eprintln!("sampler object unavailable: {error}"),
            }
        }
        if !bank.assets.is_empty() {
            eprintln!(
                "textures: {} assets uploaded, {failed} unavailable, {:.1} MiB of {:.0} MiB cap, {} sampler objects",
                bank.assets.len() - failed,
                texture::mib(bank.budget.used),
                texture::mib(bank.budget.cap),
                bank.samplers.len()
            );
        }
        bank
    }
    unsafe fn upload(&mut self, gl: &glow::Context, key: &AssetKey, mipmaps: bool) -> Result<Asset> {
        // Sizes come from the file headers; memory is reserved before a pixel is decoded,
        // so a file that does not fit is never loaded into RAM at all.
        let shapes = texture::measure(key)?;
        let reserved = texture::asset_bytes(&shapes, mipmaps);
        self.budget
            .reserve(reserved)
            .map_err(|error| anyhow!("texture {}: {error}", key.path.display()))?;
        let result = self.decode_and_upload(gl, key, &shapes, mipmaps);
        if result.is_err() {
            self.budget.used -= reserved;
        }
        result
    }
    /// The caller holds the budget reservation and releases it when this fails.
    unsafe fn decode_and_upload(
        &self,
        gl: &glow::Context,
        key: &AssetKey,
        shapes: &[texture::Shape],
        mipmaps: bool,
    ) -> Result<Asset> {
        let layers = texture::load(key)?;
        ensure!(
            layers.len() == shapes.len() && layers.iter().zip(shapes).all(|(l, s)| l.shape() == *s),
            "texture {}: decoded size differs from its header",
            key.path.display()
        );
        // Clear stale error flags so the check below is about this upload.
        for _ in 0..16 {
            if gl.get_error() == glow::NO_ERROR {
                break;
            }
        }
        let id = gl.create_texture().map_err(anyhow::Error::msg)?;
        let target = target(key.kind);
        gl.bind_texture(target, Some(id));
        let first = &layers[0];
        match key.kind {
            Kind::Flat => image_2d(gl, glow::TEXTURE_2D, first),
            Kind::Cube => {
                for (face, layer) in layers.iter().enumerate() {
                    image_2d(gl, glow::TEXTURE_CUBE_MAP_POSITIVE_X + face as u32, layer);
                }
            }
            Kind::Volume => {
                // Shadertoy uploads a one-channel volume as R8: green and blue read 0.
                let (internal, format) = if first.channels == 1 {
                    (glow::R8, glow::RED)
                } else {
                    (glow::RGBA8, glow::RGBA)
                };
                gl.tex_image_3d(
                    glow::TEXTURE_3D,
                    0,
                    internal as i32,
                    first.width as i32,
                    first.height as i32,
                    first.depth as i32,
                    0,
                    format,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(Some(&first.data)),
                );
            }
        }
        // Defaults for use without a sampler object; bindings always bring their own.
        let filter = if mipmaps { Filter::Mipmap } else { Filter::Linear };
        gl.tex_parameter_i32(target, glow::TEXTURE_MIN_FILTER, filter.gl().0 as i32);
        gl.tex_parameter_i32(target, glow::TEXTURE_MAG_FILTER, filter.gl().1 as i32);
        if mipmaps {
            gl.generate_mipmap(target);
        }
        let error = gl.get_error();
        if error != glow::NO_ERROR {
            gl.bind_texture(target, None);
            gl.delete_texture(id);
            ensure!(
                false,
                "texture {}: upload failed with glGetError 0x{error:04x}",
                key.path.display()
            );
        }
        Ok(Asset {
            texture: id,
            size: [first.width as f32, first.height as f32, first.depth as f32],
        })
    }
    /// The texture and sampler of every binding of one shader, or why it cannot render.
    pub(super) fn resolve(&self, channels: &[Channel]) -> Result<Channels> {
        let mut bound: Channels = [None; MAX_CHANNELS];
        for binding in channels {
            let slot = bound
                .get_mut(usize::from(binding.channel))
                .ok_or_else(|| anyhow!("channel {} outside iChannel0..3", binding.channel))?;
            let asset = match self.assets.get(&binding.key()) {
                Some(Ok(asset)) => asset,
                Some(Err(error)) => return Err(anyhow!("iChannel{}: {error}", binding.channel)),
                None => {
                    return Err(anyhow!(
                        "iChannel{}: texture {} was not loaded",
                        binding.channel,
                        binding.path.display()
                    ))
                }
            };
            let sampler = *self
                .samplers
                .get(&(binding.filter, binding.wrap))
                .ok_or_else(|| anyhow!("iChannel{}: no sampler object", binding.channel))?;
            *slot = Some(Bound {
                kind: binding.kind,
                texture: asset.texture,
                sampler,
                size: asset.size,
            });
        }
        Ok(bound)
    }
    pub(super) unsafe fn delete(&self, gl: &glow::Context) {
        for asset in self.assets.values().flatten() {
            gl.delete_texture(asset.texture);
        }
        for sampler in self.samplers.values() {
            gl.delete_sampler(*sampler);
        }
    }
}
unsafe fn image_2d(gl: &glow::Context, target: u32, pixels: &Pixels) {
    gl.tex_image_2d(
        target,
        0,
        glow::RGBA8 as i32,
        pixels.width as i32,
        pixels.height as i32,
        0,
        glow::RGBA,
        glow::UNSIGNED_BYTE,
        glow::PixelUnpackData::Slice(Some(&pixels.data)),
    );
}
/// Stand-in for the deck video in the benchmark: four distinct quadrants (the stem atlas
/// layout) with fine detail, so a `u_video` shader samples real 1080p texture memory.
pub(super) unsafe fn test_video(gl: &glow::Context) -> Result<glow::Texture> {
    let (width, height) = (1920usize, 1080usize);
    let mut data = vec![255u8; width * height * 4];
    for (index, pixel) in data.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let (x, y) = (index % width, index / width);
        let (qx, qy) = (x % (width / 2), y % (height / 2));
        let rings = ((qx as f32 - 480.0).hypot(qy as f32 - 270.0) * 0.08).sin() * 0.5 + 0.5;
        let bars = if (qx / 24 + qy / 24) % 2 == 0 { 1.0 } else { 0.55 };
        let tint: [f32; 3] = match (x >= width / 2, y >= height / 2) {
            (false, true) => [0.9, 0.5, 0.2],
            (true, true) => [0.9, 0.9, 0.9],
            (false, false) => [0.3, 0.8, 0.9],
            (true, false) => [0.8, 0.8, 1.0],
        };
        for (value, tint) in pixel.iter_mut().zip(tint) {
            *value = (255.0 * tint * rings * bars) as u8;
        }
    }
    let id = gl.create_texture().map_err(anyhow::Error::msg)?;
    gl.bind_texture(glow::TEXTURE_2D, Some(id));
    gl.tex_image_2d(
        glow::TEXTURE_2D,
        0,
        glow::RGBA8 as i32,
        width as i32,
        height as i32,
        0,
        glow::RGBA,
        glow::UNSIGNED_BYTE,
        glow::PixelUnpackData::Slice(Some(&data)),
    );
    for parameter in [glow::TEXTURE_MIN_FILTER, glow::TEXTURE_MAG_FILTER] {
        gl.tex_parameter_i32(glow::TEXTURE_2D, parameter, glow::LINEAR as i32);
    }
    for parameter in [glow::TEXTURE_WRAP_S, glow::TEXTURE_WRAP_T] {
        gl.tex_parameter_i32(glow::TEXTURE_2D, parameter, glow::CLAMP_TO_EDGE as i32);
    }
    gl.bind_texture(glow::TEXTURE_2D, None);
    Ok(id)
}
