//! Deliberately bounded single-pass ISF generator profile (no ISF image inputs or
//! persistent passes). Shadertoy-style `iChannel0..3` samplers are plain uniforms whose
//! textures come from the manifest's channel bindings.
use crate::texture::Kind;
use anyhow::{bail, ensure, Context, Result};
use serde_json::Value;
use std::{collections::HashSet, io::Read, path::Path};
pub const MAX_SOURCE_BYTES: usize = 256 * 1024;
/// Every shader reads its pixel position through this, so the renderer can turn and
/// shift the picture inside the shader: the shader then draws its own unbounded canvas
/// and nothing has to be tiled. With the default uniforms it returns `gl_FragCoord`.
const CANVAS_FRAG: &str = "vec4 rpivj_frag() {\n    vec2 c = vec2(u_canvas_cx, u_canvas_cy);\n    return vec4(c + mat2(u_canvas_cos, -u_canvas_sin, u_canvas_sin, u_canvas_cos) * (gl_FragCoord.xy - c) + vec2(u_canvas_x, u_canvas_y), gl_FragCoord.zw);\n}\n#define gl_FragCoord rpivj_frag()\n";
pub const FLOAT_UNIFORMS: &str = "u_canvas_cos u_canvas_sin u_canvas_x u_canvas_y u_canvas_cx u_canvas_cy u_time u_bass u_mid u_treble u_volume u_onset u_beat_onset u_beat_phase u_bar_phase u_energy u_kick u_sub u_low_mid u_high_mid u_hit_low u_hit_mid u_hit_high u_locked u_flow u_flow_low u_flow_mid u_flow_high uSubBass uBass uLowMid uHighMid uTreble uEnergy uKick uBPM uBeatPhase uBarPhase TIME TIMEDELTA";
#[derive(Debug)]
pub struct PreparedShader {
    pub source: String,
    pub defaults: Vec<(String, f32)>,
}
pub fn load(path: &Path) -> Result<PreparedShader> {
    load_with(path, &[])
}
/// `channels` are the `(iChannel index, kind)` pairs bound by the manifest.
pub fn load_with(path: &Path, channels: &[(u8, Kind)]) -> Result<PreparedShader> {
    let file = std::fs::File::open(path)?;
    ensure!(file.metadata()?.is_file(), "shader must be a regular file");
    let mut source = String::new();
    file.take((MAX_SOURCE_BYTES + 1) as u64)
        .read_to_string(&mut source)?;
    prepare_with(&source, channels)
}
pub fn prepare(source: &str) -> Result<PreparedShader> {
    prepare_with(source, &[])
}
/// A whole-line `uniform [precision] sampler* iChannelN;` declaration of a bound channel,
/// rewritten to the sampler type its binding needs. The converted Shadertoy bank declares
/// every channel `sampler2D` because the converter did not know the inputs; the manifest
/// binding does, so it decides. Anything else is returned unchanged.
fn channel_declaration(line: &str, channels: &[(u8, Kind)]) -> Option<String> {
    let code = line.split("//").next().unwrap_or("");
    let statement = code.trim().strip_suffix(';')?;
    let mut words = statement.split_whitespace();
    if words.next() != Some("uniform") {
        return None;
    }
    let mut sampler = words.next()?;
    if ["lowp", "mediump", "highp"].contains(&sampler) {
        sampler = words.next()?;
    }
    let name = words.next()?;
    if words.next().is_some() || !["sampler2D", "samplerCube", "sampler3D"].contains(&sampler) {
        return None;
    }
    let index: u8 = name.strip_prefix("iChannel")?.parse().ok()?;
    let (_, kind) = channels.iter().find(|(channel, _)| *channel == index)?;
    (kind.sampler() != sampler).then(|| format!("uniform {} {name};", kind.sampler()))
}
pub fn prepare_with(source: &str, channels: &[(u8, Kind)]) -> Result<PreparedShader> {
    ensure!(source.len() <= MAX_SOURCE_BYTES, "shader exceeds 256 KiB");
    let mut body = source.trim_start_matches('\u{feff}').trim_start();
    let mut meta = Value::Null;
    if body.starts_with("/*") {
        let end = body.find("*/").context("unterminated header")?;
        let header = body[2..end].trim();
        if header.starts_with('{') {
            meta = serde_json::from_str(header).context("invalid ISF metadata")?;
        }
        body = &body[end + 2..];
    }
    for key in ["PASSES", "IMPORTED"] {
        ensure!(
            meta.get(key).is_none(),
            "unsupported ISF {key}; single-pass generators only"
        );
    }
    ensure!(
        !body.contains("#include") && !body.contains("#extension"),
        "includes and extensions are unsupported"
    );
    let body = body
        .lines()
        .filter(|line| !line.trim_start().starts_with("#version"))
        .map(|line| channel_declaration(line, channels).unwrap_or_else(|| line.to_owned()))
        .collect::<Vec<_>>()
        .join("\n");
    let tokens: Vec<_> = body
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|s| !s.is_empty())
        .collect();
    let mut declared = HashSet::new();
    for triple in tokens.windows(3) {
        if triple[0] == "uniform" {
            declared.insert(triple[2].to_owned());
        }
    }
    let mut header = String::from("#version 300 es\nprecision highp float;\nprecision highp int;\n");
    // GLSL ES 3.00 gives sampler3D no default precision; without this line a volume
    // shader does not compile. Other shaders keep their exact previous source.
    if tokens.contains(&"sampler3D") {
        header.push_str("precision highp sampler3D;\n");
    }
    header.push_str("out vec4 isf_out;\n#define gl_FragColor isf_out\n#define texture2D texture\n#define isf_FragNormCoord (gl_FragCoord.xy / u_resolution)\n#define vv_FragNormCoord isf_FragNormCoord\n");
    let mut defaults = Vec::new();
    if let Some(inputs) = meta.get("INPUTS") {
        let inputs = inputs.as_array().context("INPUTS must be an array")?;
        ensure!(inputs.len() <= 64, "too many ISF inputs");
        let mut names = HashSet::new();
        for input in inputs {
            let name = input["NAME"].as_str().context("input requires NAME")?;
            ensure!(
                !name.is_empty()
                    && !name.as_bytes()[0].is_ascii_digit()
                    && name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_'),
                "invalid input identifier"
            );
            ensure!(names.insert(name), "duplicate input {name}");
            ensure!(
                !FLOAT_UNIFORMS.split_whitespace().any(|s| s == name)
                    && !name.starts_with("gl_")
                    && ![
                        "u_resolution",
                        "u_spectrum",
                        "u_spec_flow",
                        "RENDERSIZE",
                        "FRAMEINDEX",
                        "DATE"
                    ]
                    .contains(&name),
                "reserved input {name}"
            );
            if input["TYPE"].as_str() != Some("float") {
                bail!("unsupported input {name}: only float inputs are supported");
            }
            let value = input
                .get("DEFAULT")
                .map(|v| v.as_f64().context("float default must be numeric"))
                .transpose()?
                .unwrap_or(0.0) as f32;
            ensure!(value.is_finite(), "nonfinite default");
            defaults.push((name.to_owned(), value));
            if declared.insert(name.to_owned()) {
                header.push_str(&format!("uniform float {name};\n"));
            }
        }
    }
    for name in FLOAT_UNIFORMS.split_whitespace() {
        if declared.insert(name.to_owned()) {
            header.push_str(&format!("uniform float {name};\n"));
        }
    }
    for (name, kind) in [
        ("u_resolution", "vec2"),
        ("RENDERSIZE", "vec2"),
        ("FRAMEINDEX", "int"),
        ("DATE", "vec4"),
    ] {
        if declared.insert(name.to_owned()) {
            header.push_str(&format!("uniform {kind} {name};\n"));
        }
    }
    for name in ["u_spectrum", "u_spec_flow"] {
        if declared.insert(name.to_owned()) {
            header.push_str(&format!("uniform float {name}[32];\n"));
        }
    }
    header.push_str(CANVAS_FRAG);
    header.push_str("#line 1\n");
    header.push_str(&body);
    Ok(PreparedShader {
        source: header,
        defaults,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_shader_reads_its_position_through_the_canvas() {
        let p = prepare("void main(){gl_FragColor=vec4(gl_FragCoord.xy,0.,1.);}").unwrap();
        let canvas = p.source.find("#define gl_FragCoord rpivj_frag()").unwrap();
        // The helper itself must see the real position, so it comes before the define,
        // and the shader body after it.
        assert!(p.source.find("vec4 rpivj_frag()").unwrap() < canvas);
        assert!(p.source.find("void main()").unwrap() > canvas);
        assert!(p.source.find("uniform float u_canvas_cx;").unwrap() < canvas);
    }
    #[test]
    fn original_library_prepares() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/shaders");
        let mut count = 0;
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "fs") {
                let prepared = load(&path).unwrap();
                assert!(prepared.source.starts_with("#version 300 es"));
                count += 1;
            }
        }
        assert_eq!(count, 15);
    }
    #[test]
    fn rejects_unsupported_and_oversized() {
        for source in [
            "/*{\"PASSES\":[]}*/void main(){}",
            "/*{\"INPUTS\":[{\"NAME\":\"inputImage\",\"TYPE\":\"image\"}]}*/",
            "/*{oops}*/",
        ] {
            assert!(prepare(source).is_err());
        }
        assert!(prepare(&" ".repeat(MAX_SOURCE_BYTES + 1)).is_err());
    }
    #[test]
    fn bound_channels_get_their_sampler_type() {
        let source = "/* {\"LABEL\":\"x\"} */\nprecision highp float;\nuniform sampler2D iChannel0;\nuniform sampler2D iChannel1;\nuniform highp sampler2D iChannel2; // noise\nuniform sampler2D iChannel3;\nvoid main(){gl_FragColor=texture(iChannel1,vec3(1.0));}";
        let plain = prepare(source).unwrap();
        assert_eq!(plain.source.matches("sampler2D iChannel").count(), 4);
        assert!(!plain.source.contains("samplerCube") && !plain.source.contains("sampler3D"));
        assert_eq!(prepare_with(source, &[]).unwrap().source, plain.source);
        // A 2D binding changes nothing at all.
        assert_eq!(
            prepare_with(source, &[(0, Kind::Flat), (3, Kind::Flat)])
                .unwrap()
                .source,
            plain.source
        );
        let bound = prepare_with(source, &[(1, Kind::Cube), (2, Kind::Volume)])
            .unwrap()
            .source;
        assert!(bound.contains("uniform sampler2D iChannel0;"));
        assert!(bound.contains("uniform samplerCube iChannel1;"));
        assert!(bound.contains("uniform sampler3D iChannel2;"));
        assert!(bound.contains("uniform sampler2D iChannel3;"));
        assert!(bound.contains("precision highp sampler3D;"));
        assert!(!prepare_with(source, &[(1, Kind::Cube)])
            .unwrap()
            .source
            .contains("sampler3D"));
        // Line numbers in driver logs still point at the file.
        assert_eq!(bound.lines().count(), plain.source.lines().count() + 1);
        // A shader that declares its own type still follows the binding.
        let cube = "uniform samplerCube iChannel0;\nvoid main(){}";
        assert!(prepare_with(cube, &[(0, Kind::Cube)])
            .unwrap()
            .source
            .contains("uniform samplerCube iChannel0;"));
        assert!(prepare_with(cube, &[(0, Kind::Flat)])
            .unwrap()
            .source
            .contains("uniform sampler2D iChannel0;"));
        // A self-declared volume sampler gets its precision without any binding.
        assert!(prepare("uniform sampler3D iChannel0;\nvoid main(){}")
            .unwrap()
            .source
            .contains("precision highp sampler3D;"));
        // Only whole-line channel declarations are touched.
        for untouched in [
            "uniform sampler2D iChannel0, iChannel1;",
            "uniform sampler2D u_video;",
            "uniform float iChannel0;",
            "vec4 f(sampler2D iChannel0);",
            "// uniform sampler2D iChannel0;",
        ] {
            assert!(
                channel_declaration(untouched, &[(0, Kind::Cube), (1, Kind::Cube)]).is_none(),
                "{untouched}"
            );
        }
    }
    #[test]
    fn defaults_and_declarations() {
        let p=prepare("/*{\"INPUTS\":[{\"NAME\":\"gain\",\"TYPE\":\"float\",\"DEFAULT\":1.5}]}*/\nuniform float gain;\nuniform float u_bass;\nvoid main(){gl_FragColor=vec4(gain*u_bass);}").unwrap();
        assert_eq!(p.defaults, vec![("gain".into(), 1.5)]);
        assert_eq!(p.source.matches("uniform float gain;").count(), 1);
        assert_eq!(p.source.matches("uniform float u_bass;").count(), 1);
    }
}
