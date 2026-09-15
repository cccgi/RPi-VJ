//! Multi-layer linear color space alpha compositor.

use std::collections::HashMap;

use vj_types::{BlendMode, LayerId};

use crate::{visible_layers, LayerDraw};

/// Blend two straight (un-premultiplied) linear RGBA colors according to the specified BlendMode.
pub fn blend_colors(bottom: [f32; 4], top: [f32; 4], mode: BlendMode, opacity: f32) -> [f32; 4] {
    let ab = bottom[3];
    let at = top[3] * opacity;
    if at <= 0.0 {
        return bottom;
    }

    let a_out = match mode {
        BlendMode::Alpha | BlendMode::LumaKey { .. } | BlendMode::ChromaKey { .. } => {
            at + ab * (1.0 - at)
        }
        BlendMode::Additive | BlendMode::Multiply | BlendMode::Screen => (ab + at).min(1.0),
    };

    if a_out <= 0.0 {
        return [0.0, 0.0, 0.0, 0.0];
    }

    let mut out = [0.0; 4];
    out[3] = a_out;

    match mode {
        BlendMode::Alpha | BlendMode::LumaKey { .. } | BlendMode::ChromaKey { .. } => {
            for i in 0..3 {
                out[i] = (top[i] * at + bottom[i] * ab * (1.0 - at)) / a_out;
            }
        }
        BlendMode::Additive => {
            for i in 0..3 {
                out[i] = ((bottom[i] * ab + top[i] * at) / a_out).min(1.0);
            }
        }
        BlendMode::Multiply => {
            for i in 0..3 {
                let multiplied = bottom[i] * top[i];
                out[i] = (multiplied * at + bottom[i] * (1.0 - at)).min(1.0);
            }
        }
        BlendMode::Screen => {
            for i in 0..3 {
                let screened = 1.0 - (1.0 - bottom[i]) * (1.0 - top[i]);
                out[i] = (screened * at + bottom[i] * (1.0 - at)).min(1.0);
            }
        }
    }

    out
}

/// Composite a stack of layer drawings back-to-front using straight linear alpha blending.
pub fn composite_stack(
    layers: &[LayerDraw],
    layer_colors: &HashMap<LayerId, [f32; 4]>,
) -> [f32; 4] {
    let visible = visible_layers(layers);
    if visible.is_empty() {
        return [0.0, 0.0, 0.0, 0.0];
    }

    let first_id = visible[0].layer;
    let first_color = layer_colors
        .get(&first_id)
        .copied()
        .unwrap_or([0.0, 0.0, 0.0, 0.0]);
    let mut current = [
        first_color[0],
        first_color[1],
        first_color[2],
        first_color[3] * visible[0].opacity,
    ];

    for draw in &visible[1..] {
        let top_color = layer_colors
            .get(&draw.layer)
            .copied()
            .unwrap_or([0.0, 0.0, 0.0, 0.0]);
        current = blend_colors(current, top_color, draw.blend, draw.opacity);
    }

    current
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_blending_math() {
        let bottom = [0.2, 0.4, 0.6, 1.0];
        let top = [0.8, 0.2, 0.1, 0.5];

        let alpha_out = blend_colors(bottom, top, BlendMode::Alpha, 1.0);
        assert!((alpha_out[0] - 0.5).abs() < 1e-5);
        assert!((alpha_out[1] - 0.3).abs() < 1e-5);
        assert!((alpha_out[2] - 0.35).abs() < 1e-5);
        assert_eq!(alpha_out[3], 1.0);

        let add_out = blend_colors(bottom, top, BlendMode::Additive, 1.0);
        assert!((add_out[0] - 0.6).abs() < 1e-5);
        assert!((add_out[1] - 0.5).abs() < 1e-5);
        assert!((add_out[2] - 0.65).abs() < 1e-5);
        assert_eq!(add_out[3], 1.0);
    }
}
