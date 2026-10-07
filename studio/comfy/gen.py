#!/usr/bin/env python3
"""Generate VJ assets with the owner's local ComfyUI (workstation only).

Presets, per the owner's choice:
  hero     Krea2 turbo   concept and hero art
  texture  Flux.2 klein  textures; --tile post-processes them to wrap seamlessly
  sprite   Flux.2 klein  single subject on black, for additive sprites

The graphs are API-format equivalents of the core of the owner's UI workflows
(models, encoder, VAE, sampler settings); personal LoRAs are not included and
the UI workflow files are never read or written by this tool.

  python studio/comfy/gen.py hero "prompt" --name stage_01 --size 1920x1088
  python studio/comfy/gen.py texture "prompt" --name chrome --tile --count 4
"""
import argparse
import json
import shutil
import time
import urllib.error
import urllib.request
from pathlib import Path

SERVER = "http://127.0.0.1:8188"
COMFY_OUTPUT = Path("D:/_krea2/output")
REPO = Path(__file__).resolve().parents[2]
DEST = REPO / "target" / "studio" / "generated"
SPRITE_SUFFIX = ", single centered subject, isolated on pure black background, no text"


def loaders(unet, clip, clip_type, vae):
    return {
        "unet": {"class_type": "UNETLoader", "inputs": {"unet_name": unet, "weight_dtype": "default"}},
        "clip": {"class_type": "CLIPLoader", "inputs": {"clip_name": clip, "type": clip_type}},
        "vae": {"class_type": "VAELoader", "inputs": {"vae_name": vae}},
    }


def graph(preset, prompt, negative, width, height, seed, prefix):
    if preset == "hero":
        g = loaders("krea2_turbo_int8_convrot.safetensors", "qwen3vl_4b_fp8_scaled.safetensors", "krea2", "qwen_image_vae.safetensors")
        latent, steps = "EmptyLatentImage", 18
    else:
        g = loaders("flux-2-klein-4b.safetensors", "qwen_3_4b.safetensors", "flux2", "flux2-vae.safetensors")
        latent, steps = "EmptyFlux2LatentImage", 4
        if preset == "sprite":
            prompt += SPRITE_SUFFIX
    model, vae = ["unet", 0], ["vae", 0]
    g["pos"] = {"class_type": "CLIPTextEncode", "inputs": {"clip": ["clip", 0], "text": prompt}}
    if negative:
        g["neg"] = {"class_type": "CLIPTextEncode", "inputs": {"clip": ["clip", 0], "text": negative}}
    else:
        g["neg"] = {"class_type": "ConditioningZeroOut", "inputs": {"conditioning": ["pos", 0]}}
    g["latent"] = {"class_type": latent, "inputs": {"width": width, "height": height, "batch_size": 1}}
    g["sample"] = {"class_type": "KSampler", "inputs": {
        "model": model, "positive": ["pos", 0], "negative": ["neg", 0], "latent_image": ["latent", 0],
        "seed": seed, "steps": steps, "cfg": 1.0, "sampler_name": "euler", "scheduler": "simple", "denoise": 1.0}}
    g["decode"] = {"class_type": "VAEDecode", "inputs": {"samples": ["sample", 0], "vae": vae}}
    g["save"] = {"class_type": "SaveImage", "inputs": {"images": ["decode", 0], "filename_prefix": prefix}}
    return g


def make_seamless(source, target, ramp):
    """Cross-fade the image with a half-offset copy of itself, one axis at a time.

    Near each edge the output becomes the offset copy, whose pixels there were
    neighbours in the middle of the original, so the wrap is exact. The offset
    copy's own seam sits mid-image where its weight is zero. Doing x then y keeps
    the first axis periodic. Diffusion-side tiling was dropped: Flux.2 klein is a
    transformer and circular convolution in the VAE alone does not close the seam.
    """
    import numpy as np
    from PIL import Image

    image = np.asarray(Image.open(source).convert("RGB"), dtype=np.float32)
    for axis in (1, 0):
        size = image.shape[axis]
        edge = np.minimum(np.arange(size), size - 1 - np.arange(size)) / (size * ramp)
        weight = np.clip(edge, 0.0, 1.0)
        weight = weight * weight * (3.0 - 2.0 * weight)
        weight = weight.reshape((1, size, 1) if axis == 1 else (size, 1, 1))
        image = image * weight + np.roll(image, size // 2, axis=axis) * (1.0 - weight)
    Image.fromarray(np.clip(image + 0.5, 0, 255).astype(np.uint8)).save(target)


def call(path, payload=None):
    data = json.dumps(payload).encode() if payload is not None else None
    request = urllib.request.Request(SERVER + path, data=data, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(request, timeout=60) as response:
        return json.load(response)


def run(workflow, timeout):
    try:
        prompt_id = call("/prompt", {"prompt": workflow})["prompt_id"]
    except urllib.error.HTTPError as error:
        raise SystemExit(f"ComfyUI rejected the graph: {error.read().decode()[:1500]}")
    start = time.monotonic()
    while time.monotonic() - start < timeout:
        entry = call(f"/history/{prompt_id}").get(prompt_id)
        if entry:
            status = entry["status"]
            if status["status_str"] != "success":
                raise SystemExit(f"generation failed: {json.dumps(status)[:1500]}")
            return entry["outputs"]["save"]["images"], time.monotonic() - start
        time.sleep(2)
    raise SystemExit(f"timed out after {timeout}s")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("preset", choices=["hero", "texture", "sprite"])
    ap.add_argument("prompt")
    ap.add_argument("--name", required=True, help="output base name")
    ap.add_argument("--negative", default="")
    ap.add_argument("--size", default=None, help="WxH, multiples of 16; default 1920x1088 hero, 1024x1024 otherwise")
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--count", type=int, default=1, help="images, seeds increment")
    ap.add_argument("--tile", action="store_true", help="make the saved image wrap seamlessly (post-process)")
    ap.add_argument("--tile-ramp", type=float, default=0.25, help="blend band at each edge as a fraction of the size")
    ap.add_argument("--timeout", type=int, default=900)
    args = ap.parse_args()
    width, height = map(int, (args.size or ("1920x1088" if args.preset == "hero" else "1024x1024")).split("x"))
    if width % 16 or height % 16:
        raise SystemExit("size must be multiples of 16")
    out_dir = DEST / args.preset
    out_dir.mkdir(parents=True, exist_ok=True)
    for index in range(args.count):
        seed = args.seed + index
        prefix = f"rpi-vj/{args.preset}/{args.name}_s{seed}"
        images, seconds = run(graph(args.preset, args.prompt, args.negative, width, height, seed, prefix), args.timeout)
        for image in images:
            source = COMFY_OUTPUT / image["subfolder"] / image["filename"]
            target = out_dir / f"{args.name}_s{seed}.png"
            if args.tile:
                make_seamless(source, target, args.tile_ramp)
            else:
                shutil.copyfile(source, target)
            target.with_suffix(".json").write_text(json.dumps({
                "preset": args.preset, "prompt": args.prompt, "negative": args.negative, "seed": seed,
                "size": [width, height], "tile": args.tile, "source": str(source)}, indent=2), encoding="utf-8")
            print(f"{target}  ({seconds:.0f}s)")


if __name__ == "__main__":
    main()
