#!/usr/bin/env python3
"""Convert an official Shadertoy JSON export into the native single-pass shader profile.

The tool is deliberately API/file based. It never launches a browser and never makes
the appliance depend on Shadertoy. Obtain JSON with the official API, then run:
  python import_shadertoy.py shader.json -o examples/shaders --name ttKGDt
"""
import argparse
import json
import re
from pathlib import Path

MAX_SOURCE = 256 * 1024
SHADERTOY_IDS = [
    "ttKGDt", "tsXBzS", "tlVGDt", "sX3Xzn", "733Xzn", "3sySRK",
    "tdG3Rd", "Ml2XRD", "3sc3z4", "wtcSzN", "MsKcRh", "4tcGDr",
    "3lsSzf", "NslGRN", "MdX3zr", "mtyGWy", "lsl3RH", "MdXyzX",
    "lscczl", "4dfGzs", "3l23Rh", "ltffzl", "XsX3RB", "ldl3W8",
    "XsBXWt", "4sfGWX", "MsXGWr", "4tjSDt", "ldlcRf", "ftt3R7",
    "ll2GD3",
]


def image_pass(document):
    passes = document.get("renderpass") or document.get("renderPasses")
    if not isinstance(passes, list):
        raise ValueError("JSON does not contain renderpass[]; use the official API export")
    images = [item for item in passes if item.get("type", "").lower() == "image"]
    if len(images) != 1 or len(passes) != 1:
        raise ValueError("only one pass of type Image is supported; buffers and sound passes are rejected")
    source = images[0].get("code")
    if not isinstance(source, str) or not source.strip():
        raise ValueError("Image pass has no GLSL source")
    for item in images[0].get("inputs", []):
        channel = item.get("channel", item.get("id"))
        if channel is not None:
            raise ValueError("iChannel inputs require an explicit native texture binding and are rejected")
    return source


def convert(source, label):
    if len(source.encode("utf-8")) > MAX_SOURCE:
        raise ValueError("source exceeds native 256 KiB limit")
    if "#include" in source or "#extension" in source:
        raise ValueError("includes and extensions are unsupported on the appliance")
    if not re.search(r"\bvoid\s+mainImage\s*\(", source):
        raise ValueError("source must define mainImage(out vec4, in vec2)")
    source = re.sub(r"^\s*#version[^\n]*\n", "", source, flags=re.MULTILINE)
    source = re.sub(r"\biResolution\b", "rp_iResolution", source)
    source = re.sub(r"\biTimeDelta\b", "rp_iTimeDelta", source)
    source = re.sub(r"\biTime\b", "rp_iTime", source)
    source = re.sub(r"\biFrame\b", "rp_iFrame", source)
    source = re.sub(r"\biDate\b", "rp_iDate", source)
    source = re.sub(r"\biMouse\b", "rp_iMouse", source)
    header = f"""/* {{\"LABEL\":\"{label}\",\"SOURCE\":\"Shadertoy API export\"}} */
#version 300 es
precision highp float;
precision highp int;
out vec4 isf_out;
uniform vec2 u_resolution;
uniform float u_time, TIMEDELTA, u_volume, u_bass, u_mid, u_treble;
uniform float u_beat_onset, u_beat_phase, u_bpm;
uniform int FRAMEINDEX;
#define rp_iResolution vec3(u_resolution, 1.0)
#define rp_iTime u_time
#define rp_iTimeDelta TIMEDELTA
#define rp_iFrame FRAMEINDEX
#define rp_iDate vec4(0.0)
#define rp_iMouse vec4(0.0)
{source}
void main() {{
    vec4 rp_color = vec4(0.0);
    mainImage(rp_color, gl_FragCoord.xy);
    float rp_energy = clamp(0.72 + 0.30 * u_volume + 0.22 * u_bass + 0.12 * u_mid, 0.35, 1.35);
    float rp_pulse = 1.0 + 0.16 * u_beat_onset + 0.08 * sin(u_beat_phase * 6.2831853);
    rp_color.rgb *= rp_energy * rp_pulse;
    isf_out = vec4(max(rp_color.rgb, vec3(0.0)), clamp(rp_color.a, 0.0, 1.0));
}}
"""
    return header


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("document", type=Path, help="official /api/v1/shaders/<id> JSON export")
    parser.add_argument("-o", "--output-dir", type=Path, required=True)
    parser.add_argument("--name", help="six-character Shadertoy ID; defaults to JSON info.id")
    args = parser.parse_args()
    document = json.loads(args.document.read_text(encoding="utf-8"))
    info = document.get("info", {})
    shader_id = args.name or info.get("id") or document.get("id")
    if not isinstance(shader_id, str) or not re.fullmatch(r"[A-Za-z0-9]{6}", shader_id):
        raise SystemExit("--name must be a six-character Shadertoy ID")
    label = info.get("name") or f"Shadertoy {shader_id}"
    output = args.output_dir / f"shadertoy-{shader_id}.fs"
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(convert(image_pass(document), label), encoding="utf-8")
    metadata = {
        "id": shader_id,
        "label": label,
        "url": f"https://www.shadertoy.com/view/{shader_id}",
        "author": info.get("username"),
        "license": info.get("license"),
        "audio_adapter": "energy+pulse",
        "path": output.as_posix(),
    }
    print(json.dumps(metadata, indent=2))


if __name__ == "__main__":
    main()
