//! The only crate that owns a GPU device.
//!
//! Render graph, compositor passes, texture/target pool, frame import, and
//! presentation. The Pi GLES escape hatch, if Gate B forces it, lives entirely
//! in a backend submodule here and does not leak upward.

use vj_types::{BlendMode, LayerId, Tier};

pub mod compositor;
pub use compositor::{blend_colors, composite_stack};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    Vulkan,
    Metal,
    Dx12,
    /// Pi-only fallback, selected by measurement, never by preference.
    GlesEgl,
}

impl Backend {
    pub fn name(self) -> &'static str {
        match self {
            Backend::Vulkan => "vulkan",
            Backend::Metal => "metal",
            Backend::Dx12 => "dx12",
            Backend::GlesEgl => "gles",
        }
    }

    /// Preferred backend for a tier, before any measured fallback.
    pub fn preferred_for(tier: Tier) -> Self {
        match tier {
            Tier::Appliance => Backend::Vulkan,
            Tier::Portable => Backend::Metal,
            Tier::Workstation => Backend::Vulkan,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayerDraw {
    pub layer: LayerId,
    pub opacity: f32,
    pub blend: BlendMode,
    pub enabled: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("no adapter for backend {0:?}")]
    NoAdapter(Backend),
    #[error("frame import failed: {0}")]
    Import(String),
    #[error("device lost")]
    DeviceLost,
    #[error("surface error: {0}")]
    Surface(String),
}

/// Frame timing sample, fed to the percentile tracking the gates require.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameTiming {
    pub cpu_ns: u64,
    pub gpu_ns: u64,
    pub present_interval_ns: u64,
    pub missed_vblank: bool,
}

/// A layer stack ready to composite. Sorted back-to-front; disabled layers
/// are skipped without disturbing the order.
pub fn visible_layers(layers: &[LayerDraw]) -> Vec<LayerDraw> {
    let mut v: Vec<_> = layers
        .iter()
        .copied()
        .filter(|l| l.enabled && l.opacity > 0.0)
        .collect();
    v.sort_by_key(|l| l.layer.0);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pi_prefers_vulkan_and_falls_back_only_by_measurement() {
        assert_eq!(Backend::preferred_for(Tier::Appliance), Backend::Vulkan);
        assert_eq!(Backend::preferred_for(Tier::Portable), Backend::Metal);
    }

    #[test]
    fn invisible_layers_are_skipped_but_order_is_preserved() {
        let layers = vec![
            LayerDraw {
                layer: LayerId(2),
                opacity: 0.5,
                blend: BlendMode::Alpha,
                enabled: true,
            },
            LayerDraw {
                layer: LayerId(0),
                opacity: 1.0,
                blend: BlendMode::Alpha,
                enabled: true,
            },
            LayerDraw {
                layer: LayerId(1),
                opacity: 0.0,
                blend: BlendMode::Alpha,
                enabled: true,
            },
            LayerDraw {
                layer: LayerId(3),
                opacity: 1.0,
                blend: BlendMode::Alpha,
                enabled: false,
            },
        ];
        let v = visible_layers(&layers);
        assert_eq!(
            v.iter().map(|l| l.layer.0).collect::<Vec<_>>(),
            vec![0, 2]
        );
    }
}
