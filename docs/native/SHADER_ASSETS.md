# Shader bank input assets

State on 2026-10-07. Tool: `engine/tools/shader_bank/fetch_assets.py`.
Store: `target/shader-bank/assets/` (67 files, 44 MB) with `manifest.json`.

The store holds Shadertoy's built-in input media. Those files carry their own
licences; the store is for the owner's local use, is not redistributable, and
lives under the git-ignored `target/`. Nothing from it belongs in a commit.

## How acquisition worked

Verified from the scripts and logs:

- Shader metadata came from Shadertoy's legacy endpoint (`POST /shadertoy` with
  `s={"shaders":[id]}`), called with `fetch()` from inside a real Edge page.
  `target/ShadertoyTool/batch_persistent_edge.py` did the first 31 ids (GLSL
  only, into `target/shadertoy-persistent-edge*/`); 
  `target/ShadertoyTool/fetch_first15_assets.py` and
  `engine/tools/fetch_shadertoy_search.py` did the rest and also wrote
  `target/shadertoy-offline/<id>/shader.json` and `assets.json`, plus
  flattened GLSL in `target/shadertoy-downloads/<id>.glsl`.
- Assets were fetched the same way (in-page `fetch(url)`, base64 back to
  Python) and written **next to the metadata**, one copy per shader:
  `target/shadertoy-offline/<id>/<hash>.<ext>` (808 files, heavily duplicated).
- The logs (`target/fractals-crawl.log`, `fractals-retry.log`,
  `fractals-id-retry.log`, and `profile-retry.log` in the staging workspace)
  show the failures were all whole-shader HTTP 403s on the metadata endpoint
  (613, then 244 on retry; 300 for the profile crawl). No asset download of a
  real media file failed.
- The short "assets n/m" counts in `SHADER_CATALOG.md` are not failed
  downloads. The old collector treated every input as a file, including
  placeholders that are not files: `/presets/tex00.jpg` (keyboard),
  `/presets/mic.png`, `/presets/webcam.png`, `/media/previz/cubemap00.png`
  (the shader's own Cube A pass) and SoundCloud page URLs.
- What the old collector did miss: it downloaded only the first face of each
  cubemap. Shadertoy stores a cubemap as `<hash>.<ext>` plus `<hash>_1` ...
  `<hash>_5`. Those 30 face files were the only missing texture data.

Fetch method that works today:

- Plain HTTPS GET of `https://www.shadertoy.com/media/a/...` is refused:
  HTTP 403, body `Bad request`, from Cloudflare, with or without browser-like
  headers.
- A separate Edge window (own profile directory, outside the repository),
  one load of a real `/view/<id>` page, then in-page `fetch()` works. On this
  run no cookie import, login or manual check was needed.

## Re-running

```sh
python engine/tools/shader_bank/fetch_assets.py                 # auto: HTTPS, then browser
python engine/tools/shader_bank/fetch_assets.py --method none   # inventory + validate only
```

Run it after adding shaders to `target/native-13-shaders/` or the fractal
library (their `shader.json` must be in `target/shadertoy-offline/<id>/`). It
re-reads the bank, copies any valid per-shader download into the store, fetches
only what is still missing, validates every file by decoding it, and rewrites
the manifest. Valid files are never fetched again. Requests are sequential
with a delay (`--delay`, default 1.5 s) and stop after six consecutive
failures. Exit code 2 means a texture asset is still missing.

Needs Pillow; the browser method needs Playwright and Edge. Options:
`--profile-dir` (or `SHADER_BANK_BROWSER_PROFILE`) for the session profile,
default under the system temp directory; `--interactive` to wait for a manual
Cloudflare check; `--import-cookies` to copy Shadertoy cookies from Edge into
the session profile (off by default, not needed so far); `--media` to also
fetch music and video.

## Counts

Bank: 1,164 shaders (the 366 fractal-library ids are a subset). Inputs are
taken from Image, Buffer and Cubemap passes; Sound passes are ignored.

| Asset type | Distinct | Files | Already local and valid | Fetched | Missing |
|---|---|---|---|---|---|
| texture (2D) | 21 | 21 | 21 | 0 | 0 |
| cubemap | 6 | 36 | 6 (face 0 only) | 30 | 0 |
| volume | 2 | 2 | 2 | 0 | 0 |
| music (not a texture) | 6 | 6 | 6 | 0 | 0 |
| video (not a texture) | 2 | 2 | 2 | 0 | 0 |

- 251 shaders bind at least one texture, cubemap or volume file; all 251 now
  have every file. 190 of them are single-pass with no other input kind.
- 764 shaders have no inputs at all. 151 have Buffer or Cubemap passes.
- 9 bank shaders have no `shader.json`, so their inputs are unknown.

## Manifest

`manifest.json` has `counts`, `sampler_usage`, `assets[]` and `shaders{}`.

- `assets[]`: `source_path`, `type`, `layout` (`2d`, `cube6`, `volume3d`,
  `media`), `status`, `local_file`, `sha256`, `bytes`, dimensions, and
  `files[]` (per file: `file`, `url`, `sha256`, `bytes`, `width`, `height`,
  `channels`, `mode`, `face` for cubemaps, `origin`), plus `used_by[]` with
  shader, pass, channel and sampler.
- `shaders{}`: per id, `passes[]` with `channels[]`: `channel`, `type`,
  `sampler` (`filter`, `wrap`, `vflip`, `srgb`, `internal`), and either
  `asset` (key into `assets[]`) or, for buffers, `from_pass`.

## What the renderer has to support

- Filters: `mipmap` (trilinear, needs generated mipmaps; the large majority),
  `linear`, `nearest`. Wraps: `repeat`, `clamp`.
- `vflip` is true on most 2D textures and volumes, false on 30 2D bindings and
  on every cubemap. `srgb` is false everywhere; `internal` is always `byte`.
- 2D textures: 64x64 to 1024x1024, all power of two, one is 256x32. PNGs are
  RGBA, RGB or single-channel grey; JPEGs are RGB. Shadertoy uploads all of
  them as RGBA8, so grey must be replicated to RGB with alpha 1.
- Cubemaps: six square faces of 64, 256 or 512 px, RGB. Face order is
  `<hash>` then `_1`..`_5` = +X, -X, +Y, -Y, +Z, -Z. That order is from
  Shadertoy's loader as remembered, not re-verified here; check it visually
  with one skybox shader.
- Volumes: two 32x32x32 files, one and four channels, 8-bit. Format: 20-byte
  header (`BIN\0`, u32 width, height, depth, u8 channels, u8 layout, u16
  format) then packed texels. Needs `sampler3D` and 3D mipmaps.
- The same file is bound with different sampler settings by different
  shaders, so sampler state belongs to the binding, not the texture.

## Still not satisfiable

Not asset problems; they need engine features or a decision:

- No metadata (9): `MdX3zr Ml2XRD MsKcRh MsXGWr XsBXWt sX3Xzn tsXBzS ttKGDt
  wtcSzN`. Their source references channels in only four: `MsKcRh` (0),
  `MsXGWr` (0), `XsBXWt` (0, 1), `wtcSzN` (0). Which textures those are is
  unknown until their `shader.json` is fetched; the other five use no channel.
- Buffer passes (166 shaders) and Cube A passes (4: `X3XfRM llVBRt ls3GWj
  ltVBRt`): need multipass rendering.
- Keyboard (30 shaders): needs a 256x3 key-state texture, or a blank one.
- SoundCloud streams (29) and Shadertoy music (12): the audio texture should
  come from the appliance's own audio analysis. The six music files are in
  the store but are not needed for that.
- Video (3: `MtsSWs NtXyD4 ld3GD2`): files are in the store; needs a video
  texture source or a still frame.
- Webcam (3: `4sKBDc MXKyRV ftfSzl`) and microphone (3: `73GGDd 73V3Dt
  sXV3Wt`): no local equivalent.

The full per-shader lists are in the manifest (`non_texture_inputs`).
