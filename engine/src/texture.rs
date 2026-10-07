//! Shader input textures: manifest bindings, CPU decode and memory accounting.
//! Nothing here touches the GPU; upload and binding live in `native::textures`.
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// `iChannel0..3`.
pub const MAX_CHANNELS: usize = 4;
/// Largest accepted edge of a 2D texture, of a cubemap face and of a volume. The bank's
/// largest are 1024, 512 and 32; the limits keep one mistaken file from being worth
/// decoding at all (2048 squared with mipmaps is 21 MiB, a 1024 cubemap 32 MiB).
pub const MAX_EDGE: u32 = 2048;
pub const MAX_CUBE_EDGE: u32 = 1024;
pub const MAX_VOLUME_EDGE: u32 = 256;
/// Largest accepted asset file; the biggest Shadertoy texture is about 4 MiB.
pub const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
/// Default cap on GPU texture memory for shader inputs, all assets together. The
/// appliance has 2 GB shared with two video decoders; every binding of the bank's
/// single-pass texture shaders together needs 100.3 MiB (40 uploads, mipmaps included).
pub const DEFAULT_BUDGET_MIB: u64 = 128;
pub const BUDGET_ENV: &str = "RPI_VJ_TEXTURE_BUDGET_MB";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Kind {
    /// One PNG or JPEG.
    #[default]
    #[serde(rename = "2d")]
    Flat,
    /// Six square faces: `path` is +X, then `<stem>_1` .. `<stem>_5` are -X +Y -Y +Z -Z.
    #[serde(rename = "cube")]
    Cube,
    /// Shadertoy `.bin` volume.
    #[serde(rename = "volume")]
    Volume,
}
impl Kind {
    /// GLSL sampler type a shader must declare for a channel of this kind.
    pub fn sampler(self) -> &'static str {
        match self {
            Kind::Flat => "sampler2D",
            Kind::Cube => "samplerCube",
            Kind::Volume => "sampler3D",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Kind::Flat => "2d",
            Kind::Cube => "cube",
            Kind::Volume => "volume",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Filter {
    Nearest,
    Linear,
    /// Trilinear over generated mipmaps.
    #[default]
    Mipmap,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Wrap {
    Clamp,
    #[default]
    Repeat,
}
// GL enum values, kept here so the sampler mapping is testable without a GPU build;
// `native::textures` asserts at compile time that they equal the binding crate's.
pub const GL_NEAREST: u32 = 0x2600;
pub const GL_LINEAR: u32 = 0x2601;
pub const GL_LINEAR_MIPMAP_LINEAR: u32 = 0x2703;
pub const GL_REPEAT: u32 = 0x2901;
pub const GL_CLAMP_TO_EDGE: u32 = 0x812F;
impl Filter {
    /// `(TEXTURE_MIN_FILTER, TEXTURE_MAG_FILTER)` of the sampler object.
    pub fn gl(self) -> (u32, u32) {
        match self {
            Filter::Nearest => (GL_NEAREST, GL_NEAREST),
            Filter::Linear => (GL_LINEAR, GL_LINEAR),
            Filter::Mipmap => (GL_LINEAR_MIPMAP_LINEAR, GL_LINEAR),
        }
    }
    /// Whether the texture must carry a mipmap chain to be complete under this filter.
    pub fn mipmaps(self) -> bool {
        self == Filter::Mipmap
    }
}
impl Wrap {
    /// Wrap mode for all of S, T and R.
    pub fn gl(self) -> u32 {
        match self {
            Wrap::Clamp => GL_CLAMP_TO_EDGE,
            Wrap::Repeat => GL_REPEAT,
        }
    }
}
fn yes() -> bool {
    true
}
/// One `iChannelN` input of a shader. Sampler settings belong to the binding: the same
/// file is bound with different filters and wraps by different shaders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Channel {
    pub channel: u8,
    pub path: PathBuf,
    #[serde(default)]
    pub kind: Kind,
    #[serde(default)]
    pub filter: Filter,
    #[serde(default)]
    pub wrap: Wrap,
    /// Shadertoy's flag: true puts the image's first row at the top (t = 1).
    #[serde(default = "yes")]
    pub vflip: bool,
}
/// The `channels` value of one shader, parsed leniently: a block that is wrong in any way
/// (not a list, unknown key or value, channel outside 0..3, duplicate, empty path, more
/// than four) leaves `list` empty and says why in `error`. Only that shader is disabled;
/// the manifest around it still loads.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Bindings {
    pub list: Vec<Channel>,
    pub error: Option<String>,
    /// The rejected JSON, kept so that writing the manifest back does not lose it.
    rejected: Option<serde_json::Value>,
}
impl Bindings {
    pub fn is_empty(&self) -> bool {
        self.list.is_empty() && self.rejected.is_none()
    }
    /// `(channel, kind)` pairs, as shader preparation takes them.
    pub fn kinds(&self) -> Vec<(u8, Kind)> {
        self.list.iter().map(|c| (c.channel, c.kind)).collect()
    }
    /// The bindings, or the reason this shader must not render.
    pub fn checked(&self) -> Result<&[Channel]> {
        match &self.error {
            Some(error) => bail!("invalid channel bindings: {error}"),
            None => Ok(&self.list),
        }
    }
}
impl From<Vec<Channel>> for Bindings {
    fn from(list: Vec<Channel>) -> Self {
        Self {
            list,
            error: None,
            rejected: None,
        }
    }
}
impl<'de> Deserialize<'de> for Bindings {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        let parsed = serde_json::from_value::<Vec<Channel>>(value.clone())
            .map_err(|error| error.to_string())
            .and_then(|list| validate(&list).map(|()| list).map_err(|e| e.to_string()));
        Ok(match parsed {
            Ok(list) => list.into(),
            Err(error) => Self {
                list: Vec::new(),
                error: Some(error),
                rejected: Some(value),
            },
        })
    }
}
impl Serialize for Bindings {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.rejected {
            Some(value) => value.serialize(serializer),
            None => self.list.serialize(serializer),
        }
    }
}
/// What one upload is: two bindings share a texture only when all of this matches.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssetKey {
    pub path: PathBuf,
    pub kind: Kind,
    pub vflip: bool,
}
impl Channel {
    pub fn key(&self) -> AssetKey {
        AssetKey {
            path: self.path.clone(),
            kind: self.kind,
            // Row order is meaningless for the noise volumes and Shadertoy never flips
            // them, so both settings share one upload.
            vflip: self.vflip && self.kind == Kind::Flat,
        }
    }
}
/// Manifest-level check shared by the show and the bench manifest.
pub fn validate(channels: &[Channel]) -> Result<()> {
    ensure!(
        channels.len() <= MAX_CHANNELS,
        "a shader has at most {MAX_CHANNELS} channel bindings"
    );
    let mut seen = [false; MAX_CHANNELS];
    for binding in channels {
        let index = usize::from(binding.channel);
        ensure!(
            index < MAX_CHANNELS,
            "channel {} outside iChannel0..3",
            binding.channel
        );
        ensure!(!seen[index], "channel {} bound twice", binding.channel);
        seen[index] = true;
        ensure!(
            !binding.path.as_os_str().is_empty(),
            "channel {} has an empty path",
            binding.channel
        );
    }
    Ok(())
}
/// The six face files of a cubemap whose first face is `first`, in upload order
/// +X -X +Y -Y +Z -Z (`<hash>.<ext>`, `<hash>_1.<ext>` .. `<hash>_5.<ext>`).
pub fn cube_faces(first: &Path) -> [PathBuf; 6] {
    let stem = first
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let extension = first.extension().map(|e| e.to_string_lossy().into_owned());
    std::array::from_fn(|face| {
        if face == 0 {
            return first.to_owned();
        }
        let name = match &extension {
            Some(extension) => format!("{stem}_{face}.{extension}"),
            None => format!("{stem}_{face}"),
        };
        first.with_file_name(name)
    })
}
/// Every file a binding reads.
pub fn files(binding: &Channel) -> Vec<PathBuf> {
    match binding.kind {
        Kind::Cube => cube_faces(&binding.path).to_vec(),
        _ => vec![binding.path.clone()],
    }
}

/// Size of one layer as its file header declares it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shape {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    /// Bytes per texel once uploaded: 4 (RGBA8) for images; 1 (R8) or 4 for volumes.
    pub channels: u8,
}
/// Tightly packed 8-bit texels, first row first.
#[derive(Debug, Clone, PartialEq)]
pub struct Pixels {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    /// 4 (RGBA8) for images; 1 (R8) or 4 for volumes.
    pub channels: u8,
    pub data: Vec<u8>,
}
impl Pixels {
    pub fn shape(&self) -> Shape {
        Shape {
            width: self.width,
            height: self.height,
            depth: self.depth,
            channels: self.channels,
        }
    }
}
/// Reverse the row order of every slice in place.
pub fn flip_rows(pixels: &mut Pixels) {
    let row = pixels.width as usize * usize::from(pixels.channels);
    let slice = row * pixels.height as usize;
    if row == 0 || slice == 0 {
        return;
    }
    for layer in pixels.data.chunks_exact_mut(slice) {
        let rows = pixels.height as usize;
        for y in 0..rows / 2 {
            let (top, bottom) = layer.split_at_mut((rows - 1 - y) * row);
            top[y * row..(y + 1) * row].swap_with_slice(&mut bottom[..row]);
        }
    }
}
/// Shadertoy volume: `BIN\0`, u32 width/height/depth, u8 channels, u8 layout, u16 format,
/// then packed texels. Only 8-bit data with 1 or 4 channels exists in the asset set.
pub fn parse_volume(bytes: &[u8]) -> Result<Pixels> {
    let shape = volume_shape(bytes, bytes.len() as u64)?;
    Ok(Pixels {
        width: shape.width,
        height: shape.height,
        depth: shape.depth,
        channels: shape.channels,
        data: bytes[20..].to_vec(),
    })
}
/// Check a volume's 20-byte header against the length of its file.
pub fn volume_shape(bytes: &[u8], file_len: u64) -> Result<Shape> {
    ensure!(bytes.len() >= 20, "volume file shorter than its header");
    ensure!(&bytes[..4] == b"BIN\0", "volume file lacks the BIN signature");
    let word = |at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    let (width, height, depth) = (word(4), word(8), word(12));
    let (channels, layout) = (bytes[16], bytes[17]);
    let format = u16::from_le_bytes([bytes[18], bytes[19]]);
    for edge in [width, height, depth] {
        ensure!(
            (1..=MAX_VOLUME_EDGE).contains(&edge),
            "volume size {width}x{height}x{depth} out of range"
        );
    }
    ensure!(
        channels == 1 || channels == 4,
        "unsupported volume channel count {channels}"
    );
    ensure!(
        layout == 0 && format == 0,
        "unsupported volume layout {layout} / format {format}: only packed 8-bit texels"
    );
    let size = u64::from(width) * u64::from(height) * u64::from(depth) * u64::from(channels);
    ensure!(
        file_len - 20 == size,
        "volume holds {} texel bytes, header says {size}",
        file_len - 20
    );
    Ok(Shape {
        width,
        height,
        depth,
        channels,
    })
}
/// Decode a PNG or JPEG to RGBA8 the way a browser hands it to WebGL: grey is replicated
/// to RGB and a missing alpha channel becomes 1.
#[cfg(any(feature = "renderer", feature = "controller"))]
pub fn decode_image(bytes: &[u8]) -> Result<Pixels> {
    use std::io::Cursor;
    let (width, height) = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()?
        .into_dimensions()
        .context("unreadable image header")?;
    ensure!(
        (1..=MAX_EDGE).contains(&width) && (1..=MAX_EDGE).contains(&height),
        "image size {width}x{height} out of range (limit {MAX_EDGE})"
    );
    let image = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()?
        .decode()
        .context("undecodable image")?
        .to_rgba8();
    Ok(Pixels {
        width,
        height,
        depth: 1,
        channels: 4,
        data: image.into_raw(),
    })
}
/// Length of a regular file within the size limit.
fn file_len(path: &Path) -> Result<u64> {
    let meta = std::fs::metadata(path).with_context(|| format!("texture {}", path.display()))?;
    ensure!(meta.is_file(), "texture {} is not a file", path.display());
    ensure!(
        meta.len() <= MAX_FILE_BYTES,
        "texture {} exceeds {} MiB",
        path.display(),
        MAX_FILE_BYTES / (1024 * 1024)
    );
    Ok(meta.len())
}
fn read(path: &Path) -> Result<Vec<u8>> {
    file_len(path)?;
    std::fs::read(path).with_context(|| format!("texture {}", path.display()))
}
/// Layer sizes of one asset read from the file headers alone, in upload order. Nothing is
/// decoded: the caller reserves memory from this before any pixel buffer is allocated,
/// so an oversized or mistaken file costs a few hundred bytes of I/O, not its pixels.
#[cfg(any(feature = "renderer", feature = "controller"))]
pub fn measure(key: &AssetKey) -> Result<Vec<Shape>> {
    let image = |path: &Path, limit: u32| -> Result<Shape> {
        file_len(path)?;
        let (width, height) = image::ImageReader::open(path)
            .and_then(|reader| reader.with_guessed_format())
            .with_context(|| format!("texture {}", path.display()))?
            .into_dimensions()
            .with_context(|| format!("texture {}: unreadable image header", path.display()))?;
        ensure!(
            (1..=limit).contains(&width) && (1..=limit).contains(&height),
            "texture {}: image size {width}x{height} out of range (limit {limit})",
            path.display()
        );
        Ok(Shape {
            width,
            height,
            depth: 1,
            channels: 4,
        })
    };
    match key.kind {
        Kind::Flat => Ok(vec![image(&key.path, MAX_EDGE)?]),
        Kind::Cube => {
            let faces = cube_faces(&key.path)
                .iter()
                .map(|path| image(path, MAX_CUBE_EDGE))
                .collect::<Result<Vec<_>>>()?;
            let edge = faces[0].width;
            for face in &faces {
                ensure!(
                    face.width == edge && face.height == edge,
                    "cubemap {} needs six equal square faces",
                    key.path.display()
                );
            }
            Ok(faces)
        }
        Kind::Volume => {
            use std::io::Read;
            let len = file_len(&key.path)?;
            let mut header = Vec::with_capacity(20);
            std::fs::File::open(&key.path)
                .and_then(|file| file.take(20).read_to_end(&mut header))
                .with_context(|| format!("texture {}", key.path.display()))?;
            Ok(vec![volume_shape(&header, len)
                .with_context(|| format!("texture {}", key.path.display()))?])
        }
    }
}
/// Decoded layers of one asset in upload order: one image, six cube faces or one volume.
/// `vflip` reverses the rows so that the first image row lands at t = 1.
#[cfg(any(feature = "renderer", feature = "controller"))]
pub fn load(key: &AssetKey) -> Result<Vec<Pixels>> {
    let image = |path: &Path| -> Result<Pixels> {
        decode_image(&read(path)?).with_context(|| format!("texture {}", path.display()))
    };
    match key.kind {
        Kind::Flat => {
            let mut pixels = image(&key.path)?;
            if key.vflip {
                flip_rows(&mut pixels);
            }
            Ok(vec![pixels])
        }
        Kind::Cube => {
            let faces = cube_faces(&key.path)
                .iter()
                .map(|path| image(path))
                .collect::<Result<Vec<_>>>()?;
            let edge = faces[0].width;
            for face in &faces {
                ensure!(
                    face.width == edge && face.height == edge,
                    "cubemap {} needs six equal square faces",
                    key.path.display()
                );
            }
            Ok(faces)
        }
        Kind::Volume => Ok(vec![parse_volume(&read(&key.path)?)
            .with_context(|| format!("texture {}", key.path.display()))?]),
    }
}

/// Bytes of one layer including its full mipmap chain when `mipmaps` is set.
pub fn layer_bytes(width: u32, height: u32, depth: u32, channels: u8, mipmaps: bool) -> u64 {
    let (mut w, mut h, mut d) = (u64::from(width), u64::from(height), u64::from(depth));
    let mut total = 0;
    loop {
        total += w * h * d * u64::from(channels);
        if !mipmaps || (w == 1 && h == 1 && d == 1) {
            return total;
        }
        (w, h, d) = ((w / 2).max(1), (h / 2).max(1), (d / 2).max(1));
    }
}
/// GPU bytes an asset occupies once uploaded.
pub fn asset_bytes(layers: &[Shape], mipmaps: bool) -> u64 {
    layers
        .iter()
        .map(|l| layer_bytes(l.width, l.height, l.depth, l.channels, mipmaps))
        .sum()
}
/// Running total against a cap. An asset that does not fit is refused, never truncated.
#[derive(Debug, Clone, PartialEq)]
pub struct Budget {
    pub cap: u64,
    pub used: u64,
}
impl Budget {
    pub fn new(cap: u64) -> Self {
        Self { cap, used: 0 }
    }
    /// Cap from `RPI_VJ_TEXTURE_BUDGET_MB`, else the default.
    pub fn from_env() -> Self {
        Self::new(budget_cap(std::env::var(BUDGET_ENV).ok().as_deref()))
    }
    pub fn reserve(&mut self, bytes: u64) -> Result<()> {
        if self.used.saturating_add(bytes) > self.cap {
            bail!(
                "texture budget exceeded: {:.1} MiB needed, {:.1} of {:.0} MiB in use (raise {BUDGET_ENV})",
                mib(bytes),
                mib(self.used),
                mib(self.cap)
            );
        }
        self.used += bytes;
        Ok(())
    }
}
pub fn mib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}
/// Cap in bytes for an optional override in MiB; nonsense falls back to the default.
pub fn budget_cap(setting: Option<&str>) -> u64 {
    setting
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|v| (1..=1024).contains(v))
        .unwrap_or(DEFAULT_BUDGET_MIB)
        * 1024
        * 1024
}

#[cfg(test)]
mod tests {
    use super::*;
    fn channel(json: &str) -> Result<Channel, serde_json::Error> {
        serde_json::from_str(json)
    }
    #[test]
    fn binding_defaults_follow_shadertoy_and_names_are_fixed() {
        let c = channel(r#"{"channel":2,"path":"a/noise.png"}"#).unwrap();
        assert_eq!(
            (c.channel, c.kind, c.filter, c.wrap, c.vflip),
            (2, Kind::Flat, Filter::Mipmap, Wrap::Repeat, true)
        );
        let c = channel(
            r#"{"channel":0,"path":"sky.jpg","kind":"cube","filter":"linear","wrap":"clamp","vflip":false}"#,
        )
        .unwrap();
        assert_eq!(
            (c.kind, c.filter, c.wrap, c.vflip),
            (Kind::Cube, Filter::Linear, Wrap::Clamp, false)
        );
        let c = channel(r#"{"channel":0,"path":"n.bin","kind":"volume","filter":"nearest"}"#).unwrap();
        assert_eq!((c.kind, c.filter), (Kind::Volume, Filter::Nearest));
        // Round trip keeps the manifest spelling.
        let text = serde_json::to_string(&c).unwrap();
        assert!(text.contains(r#""kind":"volume""#) && text.contains(r#""filter":"nearest""#));
        assert_eq!(channel(&text).unwrap(), c);
        for bad in [
            r#"{"channel":0}"#,
            r#"{"path":"a.png"}"#,
            r#"{"channel":0,"path":"a.png","kind":"3d"}"#,
            r#"{"channel":0,"path":"a.png","filter":"trilinear"}"#,
            r#"{"channel":0,"path":"a.png","wrap":"mirror"}"#,
            r#"{"channel":0,"path":"a.png","srgb":false}"#,
            r#"{"channel":-1,"path":"a.png"}"#,
        ] {
            assert!(channel(bad).is_err(), "{bad}");
        }
        assert_eq!(
            [Kind::Flat, Kind::Cube, Kind::Volume].map(Kind::sampler),
            ["sampler2D", "samplerCube", "sampler3D"]
        );
    }
    #[test]
    fn sampler_settings_map_to_gl() {
        assert_eq!(Filter::Nearest.gl(), (0x2600, 0x2600));
        assert_eq!(Filter::Linear.gl(), (0x2601, 0x2601));
        // Trilinear minification, plain linear magnification (a mipmap mag filter is invalid).
        assert_eq!(Filter::Mipmap.gl(), (0x2703, 0x2601));
        assert_eq!(
            [Filter::Nearest, Filter::Linear, Filter::Mipmap].map(Filter::mipmaps),
            [false, false, true]
        );
        assert_eq!((Wrap::Repeat.gl(), Wrap::Clamp.gl()), (0x2901, 0x812F));
    }
    fn bindings(json: &str) -> Bindings {
        serde_json::from_str(json).unwrap()
    }
    #[test]
    fn a_bad_channel_block_is_an_error_value_not_a_parse_failure() {
        let good = bindings(r#"[{"channel":1,"path":"a.png","kind":"cube"}]"#);
        assert_eq!((good.list.len(), &good.error), (1, &None));
        assert_eq!(good.kinds(), vec![(1, Kind::Cube)]);
        assert_eq!(good.checked().unwrap().len(), 1);
        assert!(!good.is_empty() && Bindings::default().is_empty());
        assert!(bindings("[]").is_empty() && bindings("[]").checked().unwrap().is_empty());
        for bad in [
            r#"[{"channel":4,"path":"a.png"}]"#,
            r#"[{"channel":0,"path":"a.png"},{"channel":0,"path":"b.png"}]"#,
            r#"[{"channel":0,"path":""}]"#,
            r#"[{"channel":0,"path":"a"},{"channel":1,"path":"a"},{"channel":2,"path":"a"},{"channel":3,"path":"a"},{"channel":3,"path":"a"}]"#,
            r#"[{"channel":0,"path":"a.png","kind":"video"}]"#,
            r#"[{"channel":0,"path":"a.png","filter":"trilinear"}]"#,
            r#"[{"channel":0,"path":"a.png","wrap":"mirror"}]"#,
            r#"[{"channel":0,"path":"a.png","srgb":true}]"#,
            r#"[{"channel":0}]"#,
            r#"{"0":"a.png"}"#,
            r#""a.png""#,
            "7",
            "null",
        ] {
            let b = bindings(bad);
            assert!(b.list.is_empty() && b.error.is_some(), "{bad}");
            let message = format!("{:#}", b.checked().unwrap_err());
            assert!(message.starts_with("invalid channel bindings: "), "{message}");
            // Written back unchanged, and still an error when read again.
            assert!(!b.is_empty());
            let text = serde_json::to_string(&b).unwrap();
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&text).unwrap(),
                serde_json::from_str::<serde_json::Value>(bad).unwrap()
            );
            assert_eq!(bindings(&text), b);
        }
        assert!(bindings(r#"[{"channel":4,"path":"a.png"}]"#)
            .error
            .unwrap()
            .contains("outside iChannel0..3"));
        assert_eq!(serde_json::to_string(&good).unwrap(), serde_json::to_string(&good.list).unwrap());
    }
    #[test]
    fn validation_bounds_channels() {
        let make = |n: u8| channel(&format!(r#"{{"channel":{n},"path":"a.png"}}"#)).unwrap();
        assert!(validate(&[]).is_ok());
        assert!(validate(&[make(0), make(3), make(1), make(2)]).is_ok());
        assert!(validate(&[make(4)]).is_err());
        assert!(validate(&[make(1), make(1)]).is_err());
        let mut empty = make(0);
        empty.path = PathBuf::new();
        assert!(validate(&[empty]).is_err());
    }
    #[test]
    fn asset_identity_separates_only_what_changes_the_upload() {
        let mut a = channel(r#"{"channel":0,"path":"a.png"}"#).unwrap();
        let mut b = channel(r#"{"channel":3,"path":"a.png","filter":"nearest","wrap":"clamp"}"#).unwrap();
        // Sampler settings and the channel number do not make a second texture.
        assert_eq!(a.key(), b.key());
        b.vflip = false;
        assert_ne!(a.key(), b.key());
        a.kind = Kind::Volume;
        b.kind = Kind::Volume;
        assert_eq!(a.key(), b.key());
    }
    #[test]
    fn cube_faces_follow_the_store_naming() {
        let faces = cube_faces(Path::new("/srv/assets/94284d43.png"));
        let names: Vec<_> = faces
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            [
                "94284d43.png",
                "94284d43_1.png",
                "94284d43_2.png",
                "94284d43_3.png",
                "94284d43_4.png",
                "94284d43_5.png"
            ]
        );
        assert!(faces.iter().all(|p| p.parent() == Some(Path::new("/srv/assets"))));
        assert_eq!(cube_faces(Path::new("sky"))[5], PathBuf::from("sky_5"));
        let cube = channel(r#"{"channel":0,"path":"s.jpg","kind":"cube"}"#).unwrap();
        assert_eq!(files(&cube).len(), 6);
        assert_eq!(files(&channel(r#"{"channel":0,"path":"s.jpg"}"#).unwrap()).len(), 1);
    }
    fn volume(width: u32, height: u32, depth: u32, channels: u8, layout: u8, format: u16, texels: usize) -> Vec<u8> {
        let mut bytes = b"BIN\0".to_vec();
        for value in [width, height, depth] {
            bytes.extend(value.to_le_bytes());
        }
        bytes.extend([channels, layout]);
        bytes.extend(format.to_le_bytes());
        bytes.extend((0..texels).map(|i| i as u8));
        bytes
    }
    #[test]
    fn volume_header_is_parsed_and_checked() {
        let v = parse_volume(&volume(32, 32, 32, 1, 0, 0, 32768)).unwrap();
        assert_eq!((v.width, v.height, v.depth, v.channels), (32, 32, 32, 1));
        assert_eq!((v.data.len(), v.data[0], v.data[257]), (32768, 0, 1));
        let v = parse_volume(&volume(2, 3, 4, 4, 0, 0, 96)).unwrap();
        assert_eq!((v.width, v.height, v.depth, v.channels, v.data.len()), (2, 3, 4, 4, 96));
        for bad in [
            volume(32, 32, 32, 1, 0, 0, 32767),
            volume(32, 32, 32, 1, 0, 0, 32769),
            volume(32, 32, 32, 3, 0, 0, 98304),
            volume(32, 32, 32, 1, 1, 0, 32768),
            volume(32, 32, 32, 1, 0, 10, 32768),
            volume(0, 32, 32, 1, 0, 0, 0),
            volume(4096, 4096, 4096, 1, 0, 0, 16),
            b"BIN\0".to_vec(),
            Vec::new(),
        ] {
            assert!(parse_volume(&bad).is_err());
        }
        let mut wrong = volume(2, 2, 2, 1, 0, 0, 8);
        wrong[0] = b'P';
        assert!(parse_volume(&wrong).is_err());
    }
    #[test]
    fn rows_flip_per_layer() {
        let mut p = Pixels {
            width: 2,
            height: 3,
            depth: 2,
            channels: 1,
            data: vec![1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6],
        };
        flip_rows(&mut p);
        assert_eq!(p.data, [3, 3, 2, 2, 1, 1, 6, 6, 5, 5, 4, 4]);
        let mut rgba = Pixels {
            width: 1,
            height: 2,
            depth: 1,
            channels: 4,
            data: vec![1, 2, 3, 4, 5, 6, 7, 8],
        };
        flip_rows(&mut rgba);
        assert_eq!(rgba.data, [5, 6, 7, 8, 1, 2, 3, 4]);
    }
    #[cfg(any(feature = "renderer", feature = "controller"))]
    fn png(width: u32, height: u32, color: image::ExtendedColorType, data: &[u8]) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        image::write_buffer_with_format(&mut out, data, width, height, color, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }
    #[cfg(any(feature = "renderer", feature = "controller"))]
    #[test]
    fn grey_and_rgb_images_expand_to_rgba8() {
        use image::ExtendedColorType::{L8, La8, Rgb8, Rgba8};
        let grey = decode_image(&png(2, 1, L8, &[10, 200])).unwrap();
        assert_eq!((grey.width, grey.height, grey.depth, grey.channels), (2, 1, 1, 4));
        assert_eq!(grey.data, [10, 10, 10, 255, 200, 200, 200, 255]);
        let grey_alpha = decode_image(&png(1, 1, La8, &[7, 9])).unwrap();
        assert_eq!(grey_alpha.data, [7, 7, 7, 9]);
        let rgb = decode_image(&png(1, 2, Rgb8, &[1, 2, 3, 4, 5, 6])).unwrap();
        assert_eq!(rgb.data, [1, 2, 3, 255, 4, 5, 6, 255]);
        let rgba = decode_image(&png(1, 1, Rgba8, &[1, 2, 3, 4])).unwrap();
        assert_eq!(rgba.data, [1, 2, 3, 4]);
        assert!(decode_image(b"not an image").is_err());
        assert!(decode_image(&png(2, 1, L8, &[10, 200])[..40]).is_err());
    }
    #[cfg(any(feature = "renderer", feature = "controller"))]
    #[test]
    fn loading_flips_cubes_need_six_faces_and_missing_files_are_errors() {
        use image::ExtendedColorType::Rgb8;
        let dir = std::env::temp_dir().join(format!("rpi-vj-texture-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let flat = dir.join("flat.png");
        std::fs::write(&flat, png(1, 2, Rgb8, &[1, 1, 1, 9, 9, 9])).unwrap();
        let key = |path: &Path, kind, vflip| AssetKey {
            path: path.to_owned(),
            kind,
            vflip,
        };
        let plain = load(&key(&flat, Kind::Flat, false)).unwrap();
        assert_eq!(plain[0].data, [1, 1, 1, 255, 9, 9, 9, 255]);
        let flipped = load(&key(&flat, Kind::Flat, true)).unwrap();
        assert_eq!(flipped[0].data, [9, 9, 9, 255, 1, 1, 1, 255]);
        let cube = dir.join("cube.png");
        for (face, path) in cube_faces(&cube).iter().enumerate().take(5) {
            std::fs::write(path, png(1, 1, Rgb8, &[face as u8; 3])).unwrap();
        }
        // Five faces only: the sixth is missing.
        let error = format!("{:#}", load(&key(&cube, Kind::Cube, false)).unwrap_err());
        assert!(error.contains("cube_5.png"), "{error}");
        std::fs::write(&cube_faces(&cube)[5], png(1, 1, Rgb8, &[5; 3])).unwrap();
        let faces = load(&key(&cube, Kind::Cube, false)).unwrap();
        assert_eq!(faces.len(), 6);
        assert_eq!(faces.iter().map(|f| f.data[0]).collect::<Vec<_>>(), [0, 1, 2, 3, 4, 5]);
        // A face of another size is refused.
        std::fs::write(&cube_faces(&cube)[3], png(2, 2, Rgb8, &[0; 12])).unwrap();
        assert!(load(&key(&cube, Kind::Cube, false)).is_err());
        let bin = dir.join("v.bin");
        std::fs::write(&bin, volume(2, 2, 2, 1, 0, 0, 8)).unwrap();
        assert_eq!(load(&key(&bin, Kind::Volume, false)).unwrap()[0].depth, 2);
        std::fs::write(&bin, b"garbage").unwrap();
        assert!(load(&key(&bin, Kind::Volume, false)).is_err());
        assert!(load(&key(&dir.join("absent.png"), Kind::Flat, true)).is_err());
        assert!(load(&key(&dir, Kind::Flat, true)).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[cfg(any(feature = "renderer", feature = "controller"))]
    #[test]
    fn sizes_come_from_headers_before_anything_is_decoded() {
        use image::ExtendedColorType::{L8, Rgb8};
        let dir = std::env::temp_dir().join(format!("rpi-vj-measure-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let key = |name: &str, kind| AssetKey {
            path: dir.join(name),
            kind,
            vflip: false,
        };
        let shape = |width, height, depth, channels| Shape {
            width,
            height,
            depth,
            channels,
        };
        // A grey PNG is accounted as the RGBA8 it becomes.
        std::fs::write(dir.join("grey.png"), png(3, 2, L8, &[0; 6])).unwrap();
        assert_eq!(measure(&key("grey.png", Kind::Flat)).unwrap(), [shape(3, 2, 1, 4)]);
        assert_eq!(load(&key("grey.png", Kind::Flat)).unwrap()[0].shape(), shape(3, 2, 1, 4));
        // The header alone is enough: the pixel data of this file is cut off.
        let whole = png(64, 64, Rgb8, &[7; 64 * 64 * 3]);
        std::fs::write(dir.join("cut.png"), &whole[..48]).unwrap();
        assert_eq!(measure(&key("cut.png", Kind::Flat)).unwrap(), [shape(64, 64, 1, 4)]);
        assert!(load(&key("cut.png", Kind::Flat)).is_err());
        // Limits apply to what the header claims, 2D and cube separately.
        let wide = MAX_CUBE_EDGE + 1;
        let row = vec![0u8; wide as usize];
        std::fs::write(dir.join("wide.png"), png(wide, 1, L8, &row)).unwrap();
        assert_eq!(measure(&key("wide.png", Kind::Flat)).unwrap()[0].width, wide);
        for (face, path) in cube_faces(&dir.join("wide.png")).iter().enumerate().skip(1) {
            std::fs::write(path, png(1, 1, L8, &[face as u8])).unwrap();
        }
        let error = format!("{:#}", measure(&key("wide.png", Kind::Cube)).unwrap_err());
        assert!(error.contains("out of range"), "{error}");
        let huge = MAX_EDGE + 1;
        std::fs::write(dir.join("huge.png"), png(huge, 1, L8, &vec![0u8; huge as usize])).unwrap();
        assert!(measure(&key("huge.png", Kind::Flat)).is_err());
        // Six faces are measured, and must agree.
        for path in cube_faces(&dir.join("c.png")) {
            std::fs::write(path, png(2, 2, Rgb8, &[0; 12])).unwrap();
        }
        assert_eq!(measure(&key("c.png", Kind::Cube)).unwrap(), [shape(2, 2, 1, 4); 6]);
        std::fs::write(&cube_faces(&dir.join("c.png"))[4], png(4, 4, Rgb8, &[0; 48])).unwrap();
        assert!(measure(&key("c.png", Kind::Cube)).is_err());
        std::fs::remove_file(&cube_faces(&dir.join("c.png"))[5]).unwrap();
        assert!(measure(&key("c.png", Kind::Cube)).is_err());
        // Volumes: the header is checked against the file length without reading the texels.
        std::fs::write(dir.join("v.bin"), volume(4, 2, 3, 4, 0, 0, 96)).unwrap();
        assert_eq!(measure(&key("v.bin", Kind::Volume)).unwrap(), [shape(4, 2, 3, 4)]);
        std::fs::write(dir.join("short.bin"), volume(4, 2, 3, 4, 0, 0, 95)).unwrap();
        assert!(measure(&key("short.bin", Kind::Volume)).is_err());
        std::fs::write(dir.join("lie.bin"), volume(256, 256, 256, 4, 0, 0, 16)).unwrap();
        assert!(measure(&key("lie.bin", Kind::Volume)).is_err());
        assert!(measure(&key("absent.png", Kind::Flat)).is_err());
        assert!(measure(&key("absent.bin", Kind::Volume)).is_err());
        // What a refused asset would have cost is known without decoding it.
        let mut budget = Budget::new(1024);
        let cost = asset_bytes(&measure(&key("cut.png", Kind::Flat)).unwrap(), true);
        assert!(cost > 16384 && budget.reserve(cost).is_err() && budget.used == 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn memory_accounting_counts_mipmaps_faces_and_depth() {
        assert_eq!(layer_bytes(1024, 1024, 1, 4, false), 4 * 1024 * 1024);
        // Full chain of a square power of two is (4^(n+1) - 1) / 3 texels.
        assert_eq!(layer_bytes(4, 4, 1, 4, true), (16 + 4 + 1) * 4);
        assert_eq!(layer_bytes(256, 32, 1, 4, true), (8192 + 2048 + 512 + 128 + 32 + 8 + 4 + 2 + 1) * 4);
        assert_eq!(layer_bytes(32, 32, 32, 1, false), 32768);
        assert_eq!(layer_bytes(4, 4, 4, 1, true), 64 + 8 + 1);
        let face = Shape {
            width: 2,
            height: 2,
            depth: 1,
            channels: 4,
        };
        assert_eq!(asset_bytes(&[face; 6], false), 96);
        assert_eq!(asset_bytes(&[face; 6], true), 120);
        // The whole budget question is answered from headers: a 2048 square with mipmaps.
        let big = Shape {
            width: MAX_EDGE,
            height: MAX_EDGE,
            depth: 1,
            channels: 4,
        };
        assert!(asset_bytes(&[big], true) < 23 << 20);
    }
    #[test]
    fn budget_refuses_what_does_not_fit_and_keeps_the_total() {
        let mut budget = Budget::new(10);
        assert!(budget.reserve(6).is_ok());
        let error = budget.reserve(5).unwrap_err().to_string();
        assert!(error.contains("texture budget exceeded") && error.contains(BUDGET_ENV));
        assert_eq!(budget.used, 6);
        assert!(budget.reserve(4).is_ok());
        assert!(budget.reserve(1).is_err());
        assert!(Budget::new(u64::MAX).reserve(u64::MAX).is_ok());
        assert_eq!(budget_cap(None), DEFAULT_BUDGET_MIB << 20);
        assert_eq!(budget_cap(Some("64")), 64 << 20);
        assert_eq!(budget_cap(Some(" 256 ")), 256 << 20);
        for nonsense in ["0", "-5", "lots", "99999", ""] {
            assert_eq!(budget_cap(Some(nonsense)), DEFAULT_BUDGET_MIB << 20);
        }
    }
}
