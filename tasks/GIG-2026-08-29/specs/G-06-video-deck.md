# G-06 — Dual Video Deck → WebGL Texture
**Model:** cheap · **Blocked by:** G-02 (codec decision) · **Blocks:** G-07

## Deliverable
`gig/src/deck.js` — ES module.

## API
```js
export class Deck {
  constructor(gl, id)          // id: 'A' | 'B'
  async load(url)              // returns when first frame is decodable
  play(); pause(); stop()
  set loop(bool)
  get texture()                // WebGLTexture, or null if nothing loaded
  update()                     // call once per frame; uploads current frame
  get ready()                  // has a presentable frame
  get aspect()
}
```

## Upload path
Use `texImage2D(TEXTURE_2D, 0, RGB, RGB, UNSIGNED_BYTE, videoEl)`.
- `RGB` not `RGBA` — saves bandwidth, and V3D is bandwidth-bound.
- Set `TEXTURE_MIN_FILTER = LINEAR` (no mipmaps — `generateMipmap` per frame
  will destroy your frame budget), `WRAP_S/T = CLAMP_TO_EDGE`.
- **Only upload when the frame is new.** Track `video.currentTime` or use
  `requestVideoFrameCallback` where available. Re-uploading an unchanged frame
  at 60Hz is pure waste and is the most common cause of a slow video path.

## Video element setup
`muted`, `playsInline`, `loop`, `preload="auto"`, `crossOrigin="anonymous"`.
Muted is required for autoplay. Append to DOM but `style.display='none'` —
some builds skip decoding for fully detached elements.

## Loop seam
Set `loop = true` and let the browser handle it. If G-02 showed a visible hitch
at the loop point, note it — do not build a dual-element crossfade to hide it
unless the runbook asks for it. Closed GOP 15 from the transcode should make
the seam clean.

## Observable
Test page: load two clips, play both, draw A and B side by side through the
G-03 shader host. On the **Pi**, report fps with 0, 1, and 2 decks running.
That delta is the video budget — G-07 and G-12 must fit in what's left.
Paste into `../RESULTS.md`.
