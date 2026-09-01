//! ISF 2 parsing, embedded-profile validation, and pipeline caching.
//!
//! This crate has no access to a GPU device by construction, which is what
//! guarantees that compilation cannot happen on the render thread.

use serde::{Deserialize, Serialize};
use vj_types::Tier;

pub mod parser;
pub use parser::{inject_audio_uniforms, parse_isf_metadata};

/// The subset of ISF 2 this engine accepts. Anything outside it is rejected
/// at index time with diagnostics, never at trigger time.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EmbeddedProfile {
    pub max_passes: u8,
    pub allow_persistent_buffers: bool,
    /// Permitted intermediate render scales.
    pub allowed_scales: [f32; 3],
}

impl EmbeddedProfile {
    pub fn for_tier(tier: Tier) -> Self {
        Self {
            max_passes: match tier {
                Tier::Appliance => 4,
                _ => 8,
            },
            allow_persistent_buffers: true,
            allowed_scales: [0.25, 0.5, 1.0],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputType {
    #[serde(alias = "float", alias = "FLOAT")]
    Float,
    #[serde(alias = "bool", alias = "BOOL")]
    Bool,
    #[serde(alias = "long", alias = "LONG")]
    Long,
    #[serde(alias = "color", alias = "COLOR")]
    Color,
    #[serde(alias = "point2d", alias = "point2D", alias = "POINT2D")]
    Point2D,
    #[serde(alias = "event", alias = "EVENT")]
    Event,
    #[serde(alias = "image", alias = "IMAGE")]
    Image,
    #[serde(alias = "audioFft", alias = "audiofft", alias = "AUDIOFFT")]
    AudioFft,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IsfInput {
    #[serde(rename = "NAME")]
    pub name: String,
    #[serde(rename = "TYPE")]
    pub ty: InputType,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IsfPass {
    #[serde(rename = "TARGET", default)]
    pub target: Option<String>,
    #[serde(rename = "PERSISTENT", default)]
    pub persistent: bool,
    #[serde(rename = "SCALE", default = "one")]
    pub scale: f32,
}

fn one() -> f32 {
    1.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShaderMetadata {
    #[serde(rename = "DESCRIPTION", default)]
    pub description: Option<String>,
    #[serde(rename = "CATEGORIES", default)]
    pub categories: Vec<String>,
    #[serde(rename = "INPUTS", default)]
    pub inputs: Vec<IsfInput>,
    #[serde(rename = "PASSES", default)]
    pub passes: Vec<IsfPass>,
}

#[derive(Debug, thiserror::Error)]
pub enum ShaderError {
    #[error("too many passes: {found} > {max}")]
    TooManyPasses { found: u8, max: u8 },
    #[error("intermediate scale {0} is not permitted")]
    BadScale(f32),
    #[error("persistent buffers are disabled on this tier")]
    PersistentNotAllowed,
    #[error("parse error: {0}")]
    Parse(String),
    #[error("compile error: {0}")]
    Compile(String),
}

/// Validate a shader against a tier before it can ever be selected live.
pub fn validate(meta: &ShaderMetadata, profile: &EmbeddedProfile) -> Result<(), ShaderError> {
    let found = meta.passes.len() as u8;
    if found > profile.max_passes {
        return Err(ShaderError::TooManyPasses {
            found,
            max: profile.max_passes,
        });
    }
    for p in &meta.passes {
        if p.persistent && !profile.allow_persistent_buffers {
            return Err(ShaderError::PersistentNotAllowed);
        }
        if !profile
            .allowed_scales
            .iter()
            .any(|s| (s - p.scale).abs() < 1e-6)
        {
            return Err(ShaderError::BadScale(p.scale));
        }
    }
    Ok(())
}

/// Cache key. A shader compiled for one backend or tier is not valid for another.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub source_hash: u64,
    pub profile_version: u32,
    pub backend: &'static str,
    pub tier: Tier,
}

#[derive(Clone, Debug)]
pub struct CompiledShader {
    pub key: CacheKey,
    pub spirv: Vec<u32>,
    pub metadata: ShaderMetadata,
}

pub trait ShaderRepository: Send + Sync {
    fn metadata(&self, id: &str) -> Option<&ShaderMetadata>;
    fn compiled(&self, id: &str, tier: Tier) -> Result<CompiledShader, ShaderError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(passes: Vec<IsfPass>) -> ShaderMetadata {
        ShaderMetadata {
            description: None,
            categories: vec![],
            inputs: vec![],
            passes,
        }
    }

    fn pass(scale: f32, persistent: bool) -> IsfPass {
        IsfPass {
            target: None,
            persistent,
            scale,
        }
    }

    #[test]
    fn appliance_caps_passes_at_four() {
        let p = EmbeddedProfile::for_tier(Tier::Appliance);
        let m = meta((0..5).map(|_| pass(1.0, false)).collect());
        assert!(matches!(
            validate(&m, &p),
            Err(ShaderError::TooManyPasses { found: 5, max: 4 })
        ));
    }

    #[test]
    fn desktop_tiers_allow_eight_passes() {
        let p = EmbeddedProfile::for_tier(Tier::Workstation);
        let m = meta((0..8).map(|_| pass(0.5, false)).collect());
        assert!(validate(&m, &p).is_ok());
    }

    #[test]
    fn arbitrary_intermediate_scales_are_rejected() {
        let p = EmbeddedProfile::for_tier(Tier::Portable);
        let m = meta(vec![pass(0.37, false)]);
        assert!(matches!(validate(&m, &p), Err(ShaderError::BadScale(_))));
    }
}
