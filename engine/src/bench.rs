//! Shader benchmark contracts: manifest, result lines, statistics and synthetic inputs.
//! Nothing here touches the GPU; the measuring loop lives in `native::bench`.
use crate::{
    audio::{AudioFrame, BINS, FLOW_RATE, SPEC_FLOW_RATE},
    texture::{self, Bindings},
};
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

/// Frames kept per tier after the discarded first frame.
pub const MAX_FRAMES: usize = 90;
pub const MIN_FRAMES: usize = 3;
/// Measured GPU time per tier after which sampling stops (once `MIN_FRAMES` exist).
pub const TIER_BUDGET_MS: f64 = 2500.0;
/// A first frame slower than this on the first tier ends the entry as `too_slow`.
pub const TOO_SLOW_FIRST_MS: f64 = 1500.0;
/// Synthetic `u_time` values cover this span so scene phases are sampled.
pub const TIME_SPAN: f32 = 40.0;
pub const BPM: f32 = 128.0;
pub const THUMB: (usize, usize) = (96, 54);
/// A 64x36 probe frame whose cost, scaled to the first tier, exceeds the too-slow limit
/// by this factor ends the entry without submitting the full-size job at all. A process
/// cannot be stopped inside a GPU job, so jobs of unbounded length must not be started.
pub const PROBE_FACTOR: f64 = 4.0;
/// First-tier frame time implied by a probe frame of `probe_ms` on `probe` pixels.
pub fn probe_estimate_ms(probe_ms: f64, probe: (u32, u32), tier: (u32, u32)) -> f64 {
    probe_ms * (f64::from(tier.0) * f64::from(tier.1)) / (f64::from(probe.0) * f64::from(probe.1))
}
/// A render job needs at least this long before the V3D hang watchdog can kill it, so
/// only frames this slow are checked for having actually completed.
pub const RESET_SUSPECT_MS: f64 = 450.0;
/// Fill colour that a completed frame overwrites everywhere.
pub const SENTINEL: [u8; 4] = [1, 254, 3, 252];
/// Whether a readback taken over a `SENTINEL` fill shows the whole frame was written.
/// A killed job leaves whole tiles untouched; a few matching pixels are coincidence.
pub fn frame_completed(rgba: &[u8]) -> bool {
    rgba.as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| **pixel == SENTINEL)
        .count()
        < 256
}
/// Fractions of `TIME_SPAN` read back for pixel statistics; the middle one is the thumbnail.
pub const PIXEL_TIMES: [f32; 3] = [0.1, 0.5, 0.9];
pub const MAX_ERROR_CHARS: usize = 400;

#[derive(Debug, Clone, Deserialize)]
pub struct Entry {
    pub id: String,
    pub variant: String,
    pub path: PathBuf,
    /// Input textures for `iChannel0..3`, as in the show manifest but with paths used
    /// as written. Absent in manifests from before channel textures existed. A block that
    /// does not parse or validate fails this entry with `prepare_error`, not the run.
    #[serde(default)]
    pub channels: Bindings,
}
impl Entry {
    pub fn key(&self) -> (String, String) {
        (self.id.clone(), self.variant.clone())
    }
    /// Thumbnail file name inside `thumb_dir`; never a path.
    pub fn thumb_name(&self) -> String {
        let clean = |s: &str| -> String {
            s.chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect()
        };
        format!("{}.{}.ppm", clean(&self.id), clean(&self.variant))
    }
    /// Thumbnail name for a pattern: the 128 BPM one keeps the plain name.
    pub fn thumb_name_for(&self, pattern: AudioPattern) -> String {
        let name = self.thumb_name();
        match pattern {
            AudioPattern::Beat => name,
            AudioPattern::Max => name.replace(".ppm", ".max.ppm"),
        }
    }
}
#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub version: u32,
    pub out: PathBuf,
    pub thumb_dir: PathBuf,
    pub tiers: Vec<(u32, u32)>,
    pub entries: Vec<Entry>,
}
impl Manifest {
    pub fn load(path: &Path) -> Result<Self> {
        Self::parse(
            &std::fs::read(path)
                .with_context(|| format!("read bench manifest {}", path.display()))?,
        )
    }
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let manifest: Self = serde_json::from_slice(bytes).context("invalid bench manifest")?;
        ensure!(manifest.version == 1, "unsupported bench manifest version");
        ensure!(
            (1..=8).contains(&manifest.tiers.len()),
            "bench manifest needs 1 to 8 tiers"
        );
        let mut heights = HashSet::new();
        for (width, height) in &manifest.tiers {
            ensure!(
                (16..=3840).contains(width) && (16..=2160).contains(height),
                "tier {width}x{height} out of range"
            );
            // Result tiers are keyed by height alone.
            ensure!(heights.insert(*height), "duplicate tier height {height}");
        }
        ensure!(manifest.entries.len() <= 65536, "too many bench entries");
        let mut keys = HashSet::new();
        for entry in &manifest.entries {
            ensure!(
                !entry.id.is_empty() && !entry.variant.is_empty(),
                "bench entry needs id and variant"
            );
            ensure!(
                keys.insert(entry.key()),
                "duplicate bench entry {} {}",
                entry.id,
                entry.variant
            );
        }
        Ok(manifest)
    }
}

/// `(id, variant)` keys that already hold a `result` line. Torn or foreign lines are ignored.
pub fn completed_keys(results: &str) -> HashSet<(String, String)> {
    results
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|v| v["event"] == "result")
        .filter_map(|v| {
            Some((
                v["id"].as_str()?.to_owned(),
                v["variant"].as_str()?.to_owned(),
            ))
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct TierStats {
    pub frames: usize,
    pub mean_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub max_ms: f64,
    pub first_ms: f64,
}
/// Nearest-rank percentile of an ascending slice; `q` in 0..=1.
pub fn percentile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = (q.clamp(0.0, 1.0) * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}
/// `frames_ms` excludes the discarded first frame, which is reported as `first_ms`.
pub fn tier_stats(first_ms: f64, frames_ms: &[f64]) -> TierStats {
    let mut sorted = frames_ms.to_vec();
    sorted.sort_by(f64::total_cmp);
    TierStats {
        frames: sorted.len(),
        mean_ms: sorted.iter().sum::<f64>() / sorted.len().max(1) as f64,
        p50_ms: percentile(&sorted, 0.50),
        p95_ms: percentile(&sorted, 0.95),
        max_ms: sorted.last().copied().unwrap_or(0.0),
        first_ms,
    }
}
/// Whether a tier keeps sampling after `frames` kept frames costing `spent_ms` in total.
pub fn keep_sampling(frames: usize, spent_ms: f64) -> bool {
    frames < MIN_FRAMES || (frames < MAX_FRAMES && spent_ms < TIER_BUDGET_MS)
}
/// Deterministic low-discrepancy time for frame `index`: any frame count covers the span.
pub fn frame_time(index: usize) -> f32 {
    TIME_SPAN * ((index as f32 + 0.5) * 0.618_034).fract()
}

/// Fixed 128 BPM pattern: kick on the beat, hats off the beat, slow mid swell.
pub fn synthetic_audio(time: f32) -> AudioFrame {
    let beat = (time * BPM / 60.0).fract();
    let off = (beat + 0.5).fract();
    let bar = (time * BPM / 240.0).fract();
    let onset = (-8.0 * beat).exp();
    let bass = 0.25 + 0.65 * (-4.0 * beat).exp();
    let treble = 0.15 + 0.5 * (-7.0 * off).exp();
    let mid = 0.35 + 0.25 * (std::f32::consts::TAU * bar).sin();
    // The three levels added with the five-band analysis: a tighter kick fundamental,
    // the swell in the low mids, a snare on beats two and four in the high mids.
    let snare = (time * BPM / 120.0 + 0.5).fract();
    let sub = 0.20 + 0.70 * (-5.0 * beat).exp();
    let low_mid = 0.40 + 0.25 * (std::f32::consts::TAU * bar).sin();
    let high_mid = 0.20 + 0.45 * (-6.0 * snare).exp();
    let mut spectrum = [0.0; BINS];
    for (i, bin) in spectrum.iter_mut().enumerate() {
        let x = i as f32 / (BINS - 1) as f32;
        let level = bass * (1.0 - x).powi(4)
            + mid * (-((x - 0.45) / 0.18).powi(2)).exp()
            + treble * x.powi(3);
        *bin = level.clamp(0.0, 1.0);
    }
    AudioFrame {
        bass,
        mid,
        treble,
        volume: ((bass + mid + treble) / 3.0).clamp(0.0, 1.0),
        onset,
        sub,
        low_mid,
        high_mid,
        spectrum,
        sequence: 0,
        beat_period: 0.0,
        beat_ahead: 0.0,
        beat_sure: 0.0,
    }
}
/// Closed-form stand-ins for the renderer's integrated flow clocks.
pub fn synthetic_flow(time: f32) -> ([f32; 4], [f32; 32]) {
    (
        [0.40, 0.45, 0.35, 0.30].map(|energy: f32| time * (0.15 + energy * 2.0)),
        std::array::from_fn(|i| time * (0.05 + (0.4 - i as f32 * 0.01) * 2.0)),
    )
}
/// Worst-case audio: every level, the onset and every spectrum bin pinned at 1.
pub fn max_audio() -> AudioFrame {
    AudioFrame {
        bass: 1.0,
        mid: 1.0,
        treble: 1.0,
        volume: 1.0,
        onset: 1.0,
        sub: 1.0,
        low_mid: 1.0,
        high_mid: 1.0,
        spectrum: [1.0; BINS],
        sequence: 0,
        beat_period: 0.0,
        beat_ahead: 0.0,
        beat_sure: 0.0,
    }
}
/// Flow clocks after `time` seconds at the highest rate the renderer can advance them.
pub fn max_flow(time: f32) -> ([f32; 4], [f32; 32]) {
    (
        [time * (FLOW_RATE.0 + FLOW_RATE.1); 4],
        [time * (SPEC_FLOW_RATE.0 + SPEC_FLOW_RATE.1); 32],
    )
}
/// Frames and GPU time kept per tier under the worst-case pattern: half the 128 BPM
/// sample, since its inputs do not vary with the beat.
pub const MAX_PATTERN_FRAMES: usize = 45;
pub const MAX_PATTERN_BUDGET_MS: f64 = 1250.0;
/// The audio a tier is measured under. A shader has to fit the budget under both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioPattern {
    /// The 128 BPM pattern of plan section 3.3; fills `tiers`, `pixels` and `thumb`.
    Beat,
    /// Everything pinned at maximum and sustained; fills `tiers_max`, `pixels_max` and
    /// `thumb_max`.
    Max,
}
impl AudioPattern {
    /// Name in the result line's `audio` list.
    pub fn name(self) -> &'static str {
        match self {
            Self::Beat => "beat128",
            Self::Max => "max",
        }
    }
    pub fn audio(self, time: f32) -> AudioFrame {
        match self {
            Self::Beat => synthetic_audio(time),
            Self::Max => max_audio(),
        }
    }
    pub fn flow(self, time: f32) -> ([f32; 4], [f32; 32]) {
        match self {
            Self::Beat => synthetic_flow(time),
            Self::Max => max_flow(time),
        }
    }
    /// Whether a tier keeps sampling after `frames` kept frames costing `spent_ms`.
    pub fn keep_sampling(self, frames: usize, spent_ms: f64) -> bool {
        match self {
            Self::Beat => keep_sampling(frames, spent_ms),
            Self::Max => {
                frames < MIN_FRAMES
                    || (frames < MAX_PATTERN_FRAMES && spent_ms < MAX_PATTERN_BUDGET_MS)
            }
        }
    }
}

/// Box-filter a bottom-up RGBA readback to a top-down `THUMB`-sized RGB image.
pub fn downsample(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
    let (tw, th) = THUMB;
    let mut out = Vec::with_capacity(tw * th * 3);
    if width == 0 || height == 0 || rgba.len() < width * height * 4 {
        out.resize(tw * th * 3, 0);
        return out;
    }
    for ty in 0..th {
        // Flip: GL row 0 is the bottom of the picture.
        let flipped = th - 1 - ty;
        let y0 = flipped * height / th;
        let y1 = ((flipped + 1) * height / th).clamp(y0 + 1, height.max(y0 + 1));
        for tx in 0..tw {
            let x0 = tx * width / tw;
            let x1 = ((tx + 1) * width / tw).clamp(x0 + 1, width.max(x0 + 1));
            let mut sum = [0u64; 3];
            let mut count = 0u64;
            for y in y0..y1.min(height) {
                for x in x0..x1.min(width) {
                    let p = (y * width + x) * 4;
                    for (total, value) in sum.iter_mut().zip(&rgba[p..p + 3]) {
                        *total += u64::from(*value);
                    }
                    count += 1;
                }
            }
            out.extend(sum.map(|total| (total / count.max(1)) as u8));
        }
    }
    out
}
#[derive(Debug, Clone, PartialEq)]
pub struct Pixels {
    pub mean_luma: f64,
    pub spatial_std: f64,
    pub temporal_diff: f64,
}
fn luma(rgb: &[u8]) -> Vec<f64> {
    rgb.as_chunks::<3>()
        .0
        .iter()
        .map(|p| {
            (0.2126 * f64::from(p[0]) + 0.7152 * f64::from(p[1]) + 0.0722 * f64::from(p[2])) / 255.0
        })
        .collect()
}
/// Statistics over equally sized RGB samples, all in 0..=1 luma units:
/// mean luma over all samples, mean per-sample spatial standard deviation, and the
/// mean absolute per-pixel luma change between consecutive samples.
pub fn pixel_stats(samples: &[Vec<u8>]) -> Pixels {
    let lumas: Vec<Vec<f64>> = samples.iter().map(|s| luma(s)).collect();
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
    let means: Vec<f64> = lumas.iter().map(|l| mean(l)).collect();
    let stds: Vec<f64> = lumas
        .iter()
        .zip(&means)
        .map(|(l, m)| mean(&l.iter().map(|x| (x - m).powi(2)).collect::<Vec<_>>()).sqrt())
        .collect();
    let diffs: Vec<f64> = lumas
        .windows(2)
        .map(|pair| {
            mean(
                &pair[0]
                    .iter()
                    .zip(&pair[1])
                    .map(|(a, b)| (a - b).abs())
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    Pixels {
        mean_luma: mean(&means),
        spatial_std: mean(&stds),
        temporal_diff: mean(&diffs),
    }
}
/// Binary PPM (P6) of a top-down RGB image.
pub fn ppm(rgb: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut out = format!("P6\n{width} {height}\n255\n").into_bytes();
    out.extend_from_slice(rgb);
    out
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tier {
    Measured(TierStats),
    /// Attempted but not a measurement: `too_slow` (first frame over `TOO_SLOW_FIRST_MS`)
    /// or `gpu_reset` (the kernel GPU hang watchdog killed the render job).
    Aborted {
        reason: &'static str,
        first_ms: f64,
    },
    Skipped(String),
}
/// One engine-written `result` line. `crash` and `hang` belong to the supervisor.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub status: &'static str,
    pub compile_ms: Option<f64>,
    /// `(height, tier)` in manifest order.
    pub tiers: Vec<(u32, Tier)>,
    pub pixels: Option<Pixels>,
    pub thumb: Option<String>,
    pub error: Option<String>,
    pub temp_c: Option<f64>,
    pub throttled: Option<String>,
    /// `(channel, kind)` of every input texture that was bound while measuring.
    pub channels: Vec<(u8, texture::Kind)>,
    /// `(height, tier)` under worst-case audio, for the tiers that were measured under
    /// the 128 BPM pattern first. Empty when no tier got that far.
    pub tiers_max: Vec<(u32, Tier)>,
    pub pixels_max: Option<Pixels>,
    pub thumb_max: Option<String>,
}
impl Outcome {
    /// A failure before any tier ran: every manifest tier is reported as skipped.
    pub fn failed(status: &'static str, error: &str, manifest: &Manifest) -> Self {
        Self {
            status,
            compile_ms: None,
            tiers: manifest
                .tiers
                .iter()
                .map(|(_, height)| (*height, Tier::Skipped(status.to_owned())))
                .collect(),
            pixels: None,
            thumb: None,
            error: Some(error.to_owned()),
            temp_c: None,
            throttled: None,
            channels: Vec::new(),
            tiers_max: Vec::new(),
            pixels_max: None,
            thumb_max: None,
        }
    }
}
fn tier_value(tier: &Tier) -> Value {
    match tier {
        Tier::Measured(s) => json!({
            "frames": s.frames,
            "mean_ms": round(s.mean_ms, 3),
            "p50_ms": round(s.p50_ms, 3),
            "p95_ms": round(s.p95_ms, 3),
            "max_ms": round(s.max_ms, 3),
            "first_ms": round(s.first_ms, 3),
        }),
        Tier::Aborted { reason, first_ms } => {
            json!({"skipped": reason, "first_ms": round(*first_ms, 3)})
        }
        Tier::Skipped(reason) => json!({"skipped": reason}),
    }
}
fn pixels_value(p: &Pixels) -> Value {
    json!({
        "mean_luma": round(p.mean_luma, 4),
        "spatial_std": round(p.spatial_std, 4),
        "temporal_diff": round(p.temporal_diff, 4),
    })
}
fn round(value: f64, digits: i32) -> Value {
    let scale = 10f64.powi(digits);
    json!((value * scale).round() / scale)
}
/// Keep driver logs on one bounded line.
pub fn clip_error(error: &str) -> String {
    error
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(MAX_ERROR_CHARS)
        .collect()
}
pub fn begin_line(entry: &Entry) -> String {
    json!({"event": "begin", "id": entry.id, "variant": entry.variant}).to_string()
}
pub fn result_line(entry: &Entry, outcome: &Outcome) -> String {
    let mut line = Map::new();
    line.insert("event".into(), json!("result"));
    line.insert("id".into(), json!(entry.id));
    line.insert("variant".into(), json!(entry.variant));
    line.insert("status".into(), json!(outcome.status));
    if let Some(ms) = outcome.compile_ms {
        line.insert("compile_ms".into(), round(ms, 1));
    }
    let mut tiers = Map::new();
    for (height, tier) in &outcome.tiers {
        tiers.insert(height.to_string(), tier_value(tier));
    }
    line.insert("tiers".into(), Value::Object(tiers));
    if let Some(p) = &outcome.pixels {
        line.insert("pixels".into(), pixels_value(p));
    }
    if let Some(thumb) = &outcome.thumb {
        line.insert("thumb".into(), json!(thumb));
    }
    if let Some(error) = &outcome.error {
        line.insert("error".into(), json!(clip_error(error)));
    }
    line.insert(
        "temp_c".into(),
        outcome.temp_c.map_or(Value::Null, |t| round(t, 1)),
    );
    line.insert("throttled".into(), json!(outcome.throttled));
    // Extension to plan section 3.2: present only when input textures were bound, so the
    // report can tell a texture shader measured as designed from one measured unbound.
    if !outcome.channels.is_empty() {
        let bound: Map<String, Value> = outcome
            .channels
            .iter()
            .map(|(channel, kind)| (channel.to_string(), json!(kind.name())))
            .collect();
        line.insert("channels".into(), Value::Object(bound));
    }
    // Second extension: the worst-case audio pattern. `tiers`, `pixels` and `thumb` keep
    // meaning the 128 BPM pattern; these fields sit beside them and are absent when no
    // tier was measured under worst-case audio, so such a line is exactly the old one.
    if !outcome.tiers_max.is_empty() {
        line.insert(
            "audio".into(),
            json!([AudioPattern::Beat.name(), AudioPattern::Max.name()]),
        );
        let tiers: Map<String, Value> = outcome
            .tiers_max
            .iter()
            .map(|(height, tier)| (height.to_string(), tier_value(tier)))
            .collect();
        line.insert("tiers_max".into(), Value::Object(tiers));
    }
    if let Some(p) = &outcome.pixels_max {
        line.insert("pixels_max".into(), pixels_value(p));
    }
    if let Some(thumb) = &outcome.thumb_max {
        line.insert("thumb_max".into(), json!(thumb));
    }
    Value::Object(line).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    const MANIFEST: &str = r#"{"version": 1,
        "out": "/var/lib/rpi-vj-bench/results.jsonl",
        "thumb_dir": "/var/lib/rpi-vj-bench/thumbs",
        "tiers": [[1280, 720], [1920, 1080]],
        "entries": [{"id": "33cGDj", "variant": "base", "path": "/srv/rpi-vj-bench/shaders/base/33cGDj.fs"}]}"#;
    fn manifest() -> Manifest {
        Manifest::parse(MANIFEST.as_bytes()).unwrap()
    }
    #[test]
    fn plan_manifest_parses_and_bad_ones_are_rejected() {
        let m = manifest();
        assert_eq!(m.tiers, vec![(1280, 720), (1920, 1080)]);
        assert_eq!(m.entries[0].key(), ("33cGDj".into(), "base".into()));
        assert_eq!(m.entries[0].thumb_name(), "33cGDj.base.ppm");
        // Extra fields from the tooling are tolerated.
        assert!(Manifest::parse(
            MANIFEST
                .replace("\"version\"", "\"note\": 1, \"version\"")
                .as_bytes()
        )
        .is_ok());
        for (from, to) in [
            ("\"version\": 1", "\"version\": 2"),
            ("[[1280, 720], [1920, 1080]]", "[]"),
            ("[1920, 1080]", "[1281, 720]"),
            ("[1920, 1080]", "[0, 0]"),
            ("\"id\": \"33cGDj\"", "\"id\": \"\""),
        ] {
            assert!(
                Manifest::parse(MANIFEST.replace(from, to).as_bytes()).is_err(),
                "{to}"
            );
        }
        let entry = r#"{"id":"a","variant":"base","path":"a.fs"}"#;
        let twice = format!(
            r#"{{"version":1,"out":"o","thumb_dir":"t","tiers":[[1280,720]],"entries":[{entry},{entry}]}}"#
        );
        assert!(Manifest::parse(twice.as_bytes()).is_err());
    }
    #[test]
    fn entries_carry_optional_channel_bindings() {
        use crate::texture::{Filter, Kind, Wrap};
        // The plan's manifest has no bindings and still loads.
        assert!(manifest().entries[0].channels.is_empty());
        let with = |channels: &str| {
            MANIFEST.replace(
                r#""path": "/srv/rpi-vj-bench/shaders/base/33cGDj.fs""#,
                &format!(r#""path": "a.fs", "channels": {channels}"#),
            )
        };
        let m = Manifest::parse(
            with(
                r#"[{"channel": 0, "path": "/srv/rpi-vj-bench/assets/n.png", "kind": "2d", "filter": "mipmap", "wrap": "repeat", "vflip": true},
                    {"channel": 1, "path": "/srv/rpi-vj-bench/assets/c.jpg", "kind": "cube", "filter": "linear", "wrap": "clamp", "vflip": false}]"#,
            )
            .as_bytes(),
        )
        .unwrap();
        assert!(m.entries[0].channels.error.is_none());
        let channels = &m.entries[0].channels.list;
        assert_eq!(channels.len(), 2);
        assert_eq!(channels[0].path, PathBuf::from("/srv/rpi-vj-bench/assets/n.png"));
        assert_eq!(
            (channels[1].channel, channels[1].kind, channels[1].filter, channels[1].wrap, channels[1].vflip),
            (1, Kind::Cube, Filter::Linear, Wrap::Clamp, false)
        );
        // A bad block is that entry's problem: the manifest loads, the entry carries the
        // reason (reported as its `prepare_error`), and the other entries are untouched.
        for bad in [
            r#"[{"channel": 4, "path": "a.png"}]"#,
            r#"[{"channel": 1, "path": "a.png"}, {"channel": 1, "path": "b.png"}]"#,
            r#"[{"channel": 0, "path": "a.png", "kind": "buffer"}]"#,
            r#"[{"channel": 0, "path": ""}]"#,
            r#"[{"channel": 0, "path": "a.png", "extra": 1}]"#,
            r#""a.png""#,
        ] {
            let json = with(bad).replace(
                r#""entries": ["#,
                r#""entries": [{"id": "good", "variant": "base", "path": "g.fs", "channels": [{"channel": 0, "path": "n.png"}]}, "#,
            );
            let m = Manifest::parse(json.as_bytes()).unwrap_or_else(|e| panic!("{bad}: {e:#}"));
            assert_eq!(m.entries.len(), 2, "{bad}");
            assert!(m.entries[0].channels.checked().is_ok());
            let error = format!("{:#}", m.entries[1].channels.checked().unwrap_err());
            assert!(error.starts_with("invalid channel bindings: "), "{error}");
            let failed = Outcome::failed("prepare_error", &error, &m);
            let line: Value = serde_json::from_str(&result_line(&m.entries[1], &failed)).unwrap();
            assert_eq!(line["status"], "prepare_error");
            assert!(line["error"].as_str().unwrap().contains("invalid channel bindings"));
            assert!(line.get("channels").is_none());
        }
    }
    #[test]
    fn thumb_names_cannot_escape_the_directory() {
        let entry = Entry {
            id: "../x y".into(),
            variant: "a/b".into(),
            path: "p".into(),
            channels: Bindings::default(),
        };
        assert_eq!(entry.thumb_name(), "___x_y.a_b.ppm");
    }
    #[test]
    fn resume_counts_only_result_lines() {
        let text = concat!(
            "{\"event\":\"begin\",\"id\":\"a\",\"variant\":\"base\"}\n",
            "{\"event\":\"result\",\"id\":\"a\",\"variant\":\"base\",\"status\":\"ok\"}\n",
            "{\"event\":\"begin\",\"id\":\"b\",\"variant\":\"base\"}\n",
            "{\"event\":\"result\",\"id\":\"c\",\"variant\":\"lite\",\"status\":\"hang\"}\n",
            "\n",
            "not json\n",
            "{\"event\":\"result\",\"id\":\"d\",\"varia",
        );
        let done = completed_keys(text);
        assert_eq!(done.len(), 2);
        assert!(done.contains(&("a".into(), "base".into())));
        assert!(done.contains(&("c".into(), "lite".into())));
        assert!(!done.contains(&("c".into(), "base".into())));
        assert!(completed_keys("").is_empty());
    }
    #[test]
    fn percentiles_use_nearest_rank() {
        let values: Vec<f64> = (1..=90).map(f64::from).collect();
        assert_eq!(percentile(&values, 0.50), 45.0);
        assert_eq!(percentile(&values, 0.95), 86.0);
        assert_eq!(percentile(&values, 1.0), 90.0);
        assert_eq!(percentile(&values, 0.0), 1.0);
        assert_eq!(percentile(&[7.0], 0.95), 7.0);
        assert_eq!(percentile(&[1.0, 2.0, 3.0], 0.95), 3.0);
        assert_eq!(percentile(&[], 0.95), 0.0);
    }
    #[test]
    fn tier_stats_sort_and_keep_first_frame_apart() {
        let s = tier_stats(40.0, &[4.0, 2.0, 9.0, 3.0]);
        assert_eq!((s.frames, s.first_ms, s.max_ms), (4, 40.0, 9.0));
        assert_eq!(s.mean_ms, 4.5);
        assert_eq!(s.p50_ms, 3.0);
        assert_eq!(s.p95_ms, 9.0);
    }
    #[test]
    fn sampling_stops_at_frames_or_budget_but_keeps_minimum() {
        assert!(keep_sampling(0, 0.0));
        assert!(keep_sampling(2, 9000.0));
        assert!(!keep_sampling(3, 9000.0));
        assert!(keep_sampling(89, 100.0));
        assert!(!keep_sampling(90, 100.0));
        assert!(!keep_sampling(50, 2500.0));
    }
    #[test]
    fn frame_times_are_deterministic_and_cover_the_span() {
        let times: Vec<f32> = (0..=MAX_FRAMES).map(frame_time).collect();
        assert_eq!(times, (0..=MAX_FRAMES).map(frame_time).collect::<Vec<_>>());
        assert!(times.iter().all(|t| (0.0..TIME_SPAN).contains(t)));
        // Even four frames reach both halves of the span.
        assert!(times[..4].iter().any(|t| *t < 20.0) && times[..4].iter().any(|t| *t > 20.0));
        for quarter in 0..4 {
            let range = quarter as f32 * 10.0..(quarter + 1) as f32 * 10.0;
            assert!(times.iter().filter(|t| range.contains(t)).count() >= 15);
        }
    }
    #[test]
    fn synthetic_audio_is_bounded_and_beats_at_128_bpm() {
        let beat = 60.0 / BPM;
        let on = synthetic_audio(beat * 8.0);
        let off = synthetic_audio(beat * 8.5);
        assert!(on.onset > 0.9 && off.onset < 0.05);
        assert!(on.bass > off.bass && off.treble > on.treble);
        for step in 0..400 {
            let a = synthetic_audio(step as f32 * 0.1);
            for v in [a.bass, a.mid, a.treble, a.volume, a.onset] {
                assert!((0.0..=1.0).contains(&v));
            }
            assert!(a.spectrum.iter().all(|v| (0.0..=1.0).contains(v)));
        }
        let (flow, spec) = synthetic_flow(10.0);
        assert!(flow.iter().chain(&spec).all(|v| v.is_finite() && *v > 0.0));
    }
    #[test]
    fn beat_pattern_feeds_five_distinct_bounded_bands() {
        let mut differs = (false, false);
        for step in 0..400 {
            let a = synthetic_audio(step as f32 * 0.1);
            for v in [a.sub, a.low_mid, a.high_mid] {
                assert!((0.0..=1.0).contains(&v));
            }
            differs.0 |= (a.sub - a.bass).abs() > 0.05;
            differs.1 |= (a.low_mid - a.high_mid).abs() > 0.05;
        }
        assert!(differs.0 && differs.1);
        assert_eq!(AudioPattern::Beat.audio(3.3).bass, synthetic_audio(3.3).bass);
        assert_eq!(AudioPattern::Beat.flow(3.3), synthetic_flow(3.3));
    }
    #[test]
    fn worst_case_pattern_pins_everything_and_runs_the_clocks_flat_out() {
        for time in [0.0, 7.3, 39.9] {
            let a = AudioPattern::Max.audio(time);
            for v in [
                a.bass, a.mid, a.treble, a.volume, a.onset, a.sub, a.low_mid, a.high_mid,
            ] {
                assert_eq!(v, 1.0);
            }
            assert!(a.spectrum.iter().all(|v| *v == 1.0));
        }
        let (flow, spec) = AudioPattern::Max.flow(10.0);
        // No level in 0..=1 can advance a clock faster than this.
        assert!(flow.iter().all(|v| (*v - 21.5).abs() < 1e-4));
        assert!(spec.iter().all(|v| (*v - 20.5).abs() < 1e-4));
        let (beat_flow, beat_spec) = AudioPattern::Beat.flow(10.0);
        assert!(beat_flow.iter().zip(&flow).all(|(a, b)| a < b));
        assert!(beat_spec.iter().zip(&spec).all(|(a, b)| a < b));
        assert_eq!(
            (AudioPattern::Beat.name(), AudioPattern::Max.name()),
            ("beat128", "max")
        );
        // Half the frames and half the time of the 128 BPM sample, same minimum.
        assert!(AudioPattern::Max.keep_sampling(2, 9000.0));
        assert!(!AudioPattern::Max.keep_sampling(3, 9000.0));
        assert!(AudioPattern::Max.keep_sampling(44, 100.0));
        assert!(!AudioPattern::Max.keep_sampling(45, 100.0));
        assert!(!AudioPattern::Max.keep_sampling(20, 1250.0));
        assert!(AudioPattern::Beat.keep_sampling(60, 1250.0));
    }
    #[test]
    fn downsample_flips_and_averages() {
        let (w, h) = (192, 108);
        let mut rgba = vec![0u8; w * h * 4];
        // GL bottom half white, top half black; left-most column pair mixes 0 and 200 in blue.
        for y in 0..h {
            for x in 0..w {
                let p = (y * w + x) * 4;
                if y < h / 2 {
                    rgba[p] = 255;
                    rgba[p + 1] = 255;
                }
                rgba[p + 2] = if x % 2 == 0 { 0 } else { 200 };
            }
        }
        let rgb = downsample(&rgba, w, h);
        assert_eq!(rgb.len(), THUMB.0 * THUMB.1 * 3);
        assert_eq!(&rgb[..3], &[0, 0, 100]);
        let last = (THUMB.1 - 1) * THUMB.0 * 3;
        assert_eq!(&rgb[last..last + 3], &[255, 255, 100]);
        // Sources smaller than the thumbnail and short buffers do not panic.
        assert_eq!(downsample(&[9, 8, 7, 255], 1, 1)[..3], [9, 8, 7]);
        assert!(downsample(&[], 1280, 720).iter().all(|v| *v == 0));
    }
    #[test]
    fn pixel_statistics() {
        let n = THUMB.0 * THUMB.1;
        let black = vec![0u8; n * 3];
        let white = vec![255u8; n * 3];
        let mut half = black.clone();
        half[..n * 3 / 2].fill(255);
        let flat = pixel_stats(&[black.clone(), black.clone(), black.clone()]);
        assert_eq!(
            (flat.mean_luma, flat.spatial_std, flat.temporal_diff),
            (0.0, 0.0, 0.0)
        );
        let blink = pixel_stats(&[black.clone(), white, black]);
        assert!((blink.mean_luma - 1.0 / 3.0).abs() < 1e-9);
        assert!(blink.spatial_std.abs() < 1e-9 && (blink.temporal_diff - 1.0).abs() < 1e-9);
        let still = pixel_stats(&[half.clone(), half.clone(), half]);
        assert!((still.mean_luma - 0.5).abs() < 1e-9 && (still.spatial_std - 0.5).abs() < 1e-9);
        assert_eq!(still.temporal_diff, 0.0);
    }
    #[test]
    fn killed_frames_are_recognised_by_surviving_sentinel_tiles() {
        let mut frame = vec![0u8; 1280 * 720 * 4];
        assert!(frame_completed(&frame));
        // A shader may hit the sentinel value on a handful of pixels.
        for pixel in frame.as_chunks_mut::<4>().0.iter_mut().take(100) {
            *pixel = SENTINEL;
        }
        assert!(frame_completed(&frame));
        // One untouched 32x32 tile is not a coincidence.
        for pixel in frame.as_chunks_mut::<4>().0.iter_mut().take(32 * 32) {
            *pixel = SENTINEL;
        }
        assert!(!frame_completed(&frame));
        assert!(!frame_completed(&SENTINEL.repeat(64 * 36)));
    }
    #[test]
    fn probe_scales_by_pixel_count() {
        assert_eq!(probe_estimate_ms(1.0, (64, 36), (1280, 720)), 400.0);
        assert_eq!(probe_estimate_ms(2.0, (64, 36), (1920, 1080)), 1800.0);
        // 15 ms on the probe is the 720p cut-off: 6 s against the 1.5 s limit.
        assert!(probe_estimate_ms(15.1, (64, 36), (1280, 720)) > PROBE_FACTOR * TOO_SLOW_FIRST_MS);
        assert!(probe_estimate_ms(14.9, (64, 36), (1280, 720)) < PROBE_FACTOR * TOO_SLOW_FIRST_MS);
    }
    #[test]
    fn ppm_header() {
        let image = ppm(&[1, 2, 3, 4, 5, 6], 2, 1);
        assert_eq!(image, b"P6\n2 1\n255\n\x01\x02\x03\x04\x05\x06");
    }
    #[test]
    fn result_lines_follow_the_contract() {
        let m = manifest();
        let entry = &m.entries[0];
        assert_eq!(
            begin_line(entry),
            r#"{"event":"begin","id":"33cGDj","variant":"base"}"#
        );
        let ok = Outcome {
            status: "ok",
            compile_ms: Some(412.04),
            tiers: vec![
                (720, Tier::Measured(tier_stats(9.8, &[4.0, 4.1234567, 4.6]))),
                (1080, Tier::Skipped("too_slow".into())),
            ],
            pixels: Some(Pixels {
                mean_luma: 0.31,
                spatial_std: 0.22,
                temporal_diff: 0.08,
            }),
            thumb: Some(entry.thumb_name()),
            error: None,
            temp_c: Some(61.23),
            throttled: Some("0x0".into()),
            channels: Vec::new(),
            tiers_max: Vec::new(),
            pixels_max: None,
            thumb_max: None,
        };
        let line = result_line(entry, &ok);
        assert!(!line.contains('\n'));
        let v: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["event"], "result");
        assert_eq!(v["status"], "ok");
        assert_eq!(v["compile_ms"], 412.0);
        assert_eq!(v["tiers"]["720"]["frames"], 3);
        assert_eq!(v["tiers"]["720"]["p50_ms"], 4.123);
        assert_eq!(v["tiers"]["720"]["first_ms"], 9.8);
        assert_eq!(v["tiers"]["1080"]["skipped"], "too_slow");
        let slow = Outcome {
            status: "too_slow",
            tiers: vec![
                (
                    720,
                    Tier::Aborted {
                        reason: "too_slow",
                        first_ms: 1800.5,
                    },
                ),
                (1080, Tier::Skipped("too_slow".into())),
            ],
            ..ok.clone()
        };
        let s: Value = serde_json::from_str(&result_line(entry, &slow)).unwrap();
        assert_eq!(s["tiers"]["720"]["skipped"], "too_slow");
        assert_eq!(s["tiers"]["720"]["first_ms"], 1800.5);
        assert!(s["tiers"]["1080"].get("first_ms").is_none());
        assert_eq!(v["pixels"]["mean_luma"], 0.31);
        assert_eq!(v["thumb"], "33cGDj.base.ppm");
        assert_eq!(v["temp_c"], 61.2);
        assert_eq!(v["throttled"], "0x0");
        assert!(v.get("error").is_none());
        assert_eq!(completed_keys(&line).len(), 1);
        // Unbound entries keep the exact pre-texture line; bound ones name their channels.
        assert!(v.get("channels").is_none());
        let bound = Outcome {
            channels: vec![(0, texture::Kind::Flat), (2, texture::Kind::Cube)],
            ..ok.clone()
        };
        let b: Value = serde_json::from_str(&result_line(entry, &bound)).unwrap();
        assert_eq!(b["channels"], json!({"0": "2d", "2": "cube"}));
        assert_eq!(b["tiers"], v["tiers"]);
        // A line without a worst-case measurement carries none of the new fields.
        for field in ["audio", "tiers_max", "pixels_max", "thumb_max"] {
            assert!(v.get(field).is_none(), "{field}");
        }
        let both = Outcome {
            tiers_max: vec![
                (720, Tier::Measured(tier_stats(9.0, &[5.0, 5.5, 6.0]))),
                (
                    1080,
                    Tier::Aborted {
                        reason: "gpu_reset",
                        first_ms: 1001.0,
                    },
                ),
            ],
            pixels_max: Some(Pixels {
                mean_luma: 0.6,
                spatial_std: 0.2,
                temporal_diff: 0.1,
            }),
            thumb_max: Some(entry.thumb_name_for(AudioPattern::Max)),
            ..ok.clone()
        };
        let m2: Value = serde_json::from_str(&result_line(entry, &both)).unwrap();
        // The existing fields keep their names and meaning.
        for field in ["tiers", "pixels", "thumb", "status", "compile_ms"] {
            assert_eq!(m2[field], v[field], "{field}");
        }
        assert_eq!(m2["audio"], json!(["beat128", "max"]));
        assert_eq!(m2["tiers_max"]["720"]["p95_ms"], 6.0);
        assert_eq!(m2["tiers_max"]["720"]["frames"], 3);
        assert_eq!(m2["tiers_max"]["1080"]["skipped"], "gpu_reset");
        assert_eq!(m2["pixels_max"]["mean_luma"], 0.6);
        assert_eq!(m2["thumb_max"], "33cGDj.base.max.ppm");
        assert_eq!(entry.thumb_name_for(AudioPattern::Beat), entry.thumb_name());

        let log = format!(
            "shader compile failed:\n0:12(3): error: {}",
            "x".repeat(900)
        );
        let failed = Outcome::failed("compile_error", &log, &m);
        let v: Value = serde_json::from_str(&result_line(entry, &failed)).unwrap();
        assert_eq!(v["status"], "compile_error");
        let error = v["error"].as_str().unwrap();
        assert_eq!(error.chars().count(), MAX_ERROR_CHARS);
        assert!(error.starts_with("shader compile failed: 0:12(3): error:"));
        assert_eq!(v["tiers"]["720"]["skipped"], "compile_error");
        assert_eq!(v["tiers"]["1080"]["skipped"], "compile_error");
        assert!(v["temp_c"].is_null() && v["throttled"].is_null());
        assert!(v.get("pixels").is_none() && v.get("thumb").is_none());
    }
}
