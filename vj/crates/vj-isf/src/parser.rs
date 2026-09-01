//! Parser for Interactive Shader Format (ISF) 2.0 fragment shaders.

use crate::{InputType, IsfInput, IsfPass, ShaderError, ShaderMetadata};

/// Extract JSON frontmatter block `/*{ ... }*/` from `.fs` shader files and parse it.
pub fn parse_isf_metadata(source: &str) -> Result<ShaderMetadata, ShaderError> {
    let start_idx = source
        .find("/*")
        .ok_or_else(|| ShaderError::Parse("Could not find start of comment block".into()))?;

    let remainder = &source[start_idx + 2..];

    let end_idx = remainder
        .find("*/")
        .ok_or_else(|| ShaderError::Parse("Could not find end of comment block".into()))?;

    let comment_content = remainder[..end_idx].trim();

    let json_start = comment_content
        .find('{')
        .ok_or_else(|| ShaderError::Parse("No JSON block found in header".into()))?;

    let json_end = comment_content
        .rfind('}')
        .ok_or_else(|| ShaderError::Parse("Unterminated JSON block in header".into()))?;

    let json_str = &comment_content[json_start..=json_end];

    let metadata: ShaderMetadata = serde_json::from_str(json_str)
        .map_err(|e| ShaderError::Parse(format!("Failed to parse JSON: {}", e)))?;

    Ok(metadata)
}

/// Injects standard audio and time uniforms into the fragment shader source.
pub fn inject_audio_uniforms(source: &str) -> String {
    let uniforms = "\nuniform float u_time;\nuniform vec2 u_resolution;\nuniform float u_bass;\nuniform float u_mid;\nuniform float u_treble;\nuniform float u_volume;\nuniform float u_onset;\nuniform float u_beat_phase;\n";

    if let Some(version_idx) = source.find("#version") {
        if let Some(newline_idx) = source[version_idx..].find('\n') {
            let split_pos = version_idx + newline_idx + 1;
            let mut result = source[..split_pos].to_string();
            result.push_str(uniforms);
            result.push_str(&source[split_pos..]);
            return result;
        }
    }

    let mut result = uniforms.to_string();
    result.push_str(source);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{validate, EmbeddedProfile};
    use vj_types::Tier;

    #[test]
    fn test_isf_parser_extracts_inputs() {
        let source = r#"
        /*{
            "DESCRIPTION": "Neon Tunnel",
            "CATEGORIES": ["Generator"],
            "INPUTS": [
                { "NAME": "speed", "TYPE": "float" },
                { "NAME": "colorVal", "TYPE": "color" },
                { "NAME": "enable", "TYPE": "bool" }
            ],
            "PASSES": [
                { "TARGET": "buffer1", "PERSISTENT": true }
            ]
        }*/
        void main() {
            gl_FragColor = vec4(u_bass, u_mid, u_treble, 1.0);
        }
        "#;

        let meta = parse_isf_metadata(source).expect("successful parse");
        assert_eq!(meta.description, Some("Neon Tunnel".to_string()));
        assert_eq!(meta.categories, vec!["Generator".to_string()]);
        assert_eq!(meta.inputs.len(), 3);
        assert_eq!(meta.inputs[0].name, "speed");
        assert_eq!(meta.inputs[0].ty, InputType::Float);
        assert_eq!(meta.inputs[1].name, "colorVal");
        assert_eq!(meta.inputs[1].ty, InputType::Color);
        assert_eq!(meta.inputs[2].name, "enable");
        assert_eq!(meta.inputs[2].ty, InputType::Bool);
        assert_eq!(meta.passes.len(), 1);
        assert_eq!(meta.passes[0].target, Some("buffer1".to_string()));
        assert!(meta.passes[0].persistent);

        // Verify uniform injection
        let injected = inject_audio_uniforms(source);
        assert!(injected.contains("uniform float u_bass;"));
        assert!(injected.contains("uniform float u_beat_phase;"));
    }

    #[test]
    fn test_over_budget_shader_rejected() {
        let p = EmbeddedProfile::for_tier(Tier::Appliance);
        let passes = (0..6)
            .map(|_| IsfPass {
                target: None,
                persistent: false,
                scale: 1.0,
            })
            .collect();
        let m = ShaderMetadata {
            description: None,
            categories: vec![],
            inputs: vec![],
            passes,
        };
        assert!(matches!(
            validate(&m, &p),
            Err(ShaderError::TooManyPasses { found: 6, max: 4 })
        ));
    }
}
