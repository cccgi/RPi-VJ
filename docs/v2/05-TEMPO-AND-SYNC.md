# Tempo and Sync

Beat-quantized clip launching is the difference between a VJ rig and a video player. It requires tempo *and phase*, and it requires knowing when to stop trusting them.

## 1. Tempo sources

| Source | Provides | Authority | Availability |
|---|---|---|---|
| **Audio analysis** | BPM estimate, beat phase estimate, confidence | Derived — never authoritative | Always, on every platform |
| **Ableton Link** | Session tempo, beat phase, bar phase, peer count | Authoritative when a peer is present | rekordbox 6+, Serato DJ Pro, and most DAWs. Requires a network path between the engine and the DJ software |
| **MIDI clock** | 24 PPQN tick stream, start/stop | Authoritative for tempo, weak for phase | Only if a device or bridge emits it. rekordbox does not natively |
| **Manual tap** | BPM, phase reset | Authoritative until overridden | Always |
| **Pro DJ Link** | Beat grid, bar position, master player | Authoritative | CDJ/XDJ only — not the FLX10. Designed for, not implemented |

You selected audio analysis and Ableton Link. The arbiter below accommodates the others without redesign.

## 2. Why Link matters for your rig

The DDJ-FLX10 in rekordbox or Serato runs in the vendor's native mode and is not available as a MIDI source to a third application, so there is no wire to take clock from. What both applications *do* support is Ableton Link, which carries tempo **and phase** over the local network with peer-to-peer clock alignment.

Practical consequence for the mobile rig: the Pi joins the Link session over Ethernet (preferred — a small unmanaged switch between the laptop and the Pi is more dependable at a gig than venue Wi-Fi, and Link works fine on a link-local subnet with no router). The Pi's visuals then lock to the deck the DJ software is playing, and audio analysis becomes a fallback rather than the primary clock.

Confidence note: rekordbox's Link support and its exact behaviour across versions should be confirmed on your installed version before you depend on it at a show — the community has produced bridge tools precisely because rekordbox's Link/clock story has been inconsistent historically. Treat "Link works from rekordbox on my machine" as a Gate J item, not an assumption.

## 3. Tempo arbiter

One component owns the answer to "what beat is it." Everything else reads it.

```rust
pub enum TempoSource { Audio, Link, MidiClock, Manual, ProDjLink }

pub struct TempoState {
    pub bpm: f32,
    pub beat_phase: f32,       // 0..1 within the beat
    pub bar_phase: f32,        // 0..1 within the bar
    pub beats_per_bar: u8,     // 4 unless configured otherwise
    pub confidence: f32,       // 0..1
    pub source: TempoSource,   // who is currently authoritative
    pub since_ns: u64,         // when this source took authority
}

pub enum AuthorityPolicy {
    /// Highest-priority available source with confidence above its floor wins.
    Priority(Vec<TempoSource>),
    /// One source, no failover. For a rehearsed show.
    Pinned(TempoSource),
    /// Link when a peer is present; audio otherwise. The recommended default.
    LinkWithAudioFallback,
}
```

Default policy: `Priority([Manual, Link, MidiClock, Audio])`. A manual tap always wins because it is the operator overriding the machine, and it expires back to the next source after a configurable timeout.

### Handover discipline

Switching authority mid-set must not visibly jump the phase. On handover:

- Ramp `bpm` toward the new source over ~1 beat rather than snapping.
- Align phase at the *next* bar boundary of the outgoing source, not immediately.
- Emit a `TempoAuthorityChanged` event so the surfaces can show it — a Fire OLED line reading `LINK 128.0` versus `AUD 127.6~` tells you at a glance whether you are locked.

### Confidence floors

Audio-derived tempo is admitted only above a confidence floor (start at 0.6) and its confidence decays smoothly rather than snapping to zero during a breakdown. This is the same requirement as the shader `u_beat_confidence` contract: a BPM-locked effect must degrade gracefully, not stutter, when the DJ drops the beat.

## 4. Quantized launch scheduler

`TriggerClip { quantize }` does not fire immediately. The scheduler computes the target presentation time and holds the command.

```
Quantize::Off        → next vblank
Quantize::Beat       → next beat boundary
Quantize::HalfBar    → next half-bar
Quantize::Bar        → next bar
Quantize::Phrase(n)  → next n-bar boundary (n = 4, 8, 16)
```

Three rules that make this feel right rather than late:

1. **Preroll ahead of the boundary.** The scheduler issues `CueClip` as soon as the trigger is received, so the decoder slot is hot well before the boundary arrives. A quantized trigger that misses its boundary because decode started late is worse than an unquantized one.
2. **A launch window, not a launch instant.** If the boundary is within ~40 ms, fire on *this* boundary. If it has just passed within ~80 ms, fire immediately rather than waiting a full beat — the operator pressed late and meant this beat. Both windows are configurable; the "just missed" window is what makes hardware feel forgiving.
3. **Show the pending state.** A pad awaiting its boundary pulses at the beat rate on devices that support hardware pulse (Launchpad), or alternates colour at ≤10 Hz elsewhere. The Stream Deck shows a cued border. This is the single highest-value piece of feedback in the whole surface layer.

Pending launches are cancellable by pressing the pad again before the boundary.

## 5. Beat-derived modulation

Beyond launching, tempo feeds the shader ABI directly as `u_beat_phase`, `u_bar_phase`, and `u_bpm`, and feeds an LFO bank in the parameter system:

```
lfo.<n>.rate    = 1/4, 1/2, 1, 2, 4 bars  (tempo-synced) or free-running Hz
lfo.<n>.shape   = sine | triangle | saw | ramp | square | sample-and-hold
lfo.<n>.phase   = offset 0..1
```

An LFO can be bound to any `ParameterPath`, which is how you get bar-synced strobes, phrase-length colour sweeps, and beat-locked feedback zoom without writing a shader for each. Tempo-synced LFOs read phase from the arbiter, so they inherit the same handover discipline and never free-run away from the music.

## 6. Validation

Gate J in [`08-VALIDATION.md`](08-VALIDATION.md):

- Link peer discovered and tempo matched within one beat of the DJ software changing tempo.
- Phase error between the engine's `bar_phase` and the DJ software's bar, measured over 10 minutes, stays within a documented tolerance.
- Authority handover Link→Audio (unplug the network) and Audio→Link (replug) produces no visible phase jump.
- A quantized launch lands within one frame of its target boundary at p95, measured from the presentation timestamp.
- Audio-only tempo tracking holds through a 32-bar section of a track with a sparse breakdown without `u_bpm` going to zero.

## Sources

- [Serato DJ Pro: four ways to sync with external MIDI hardware — DJ TechTools](https://djtechtools.com/2018/06/27/serato-dj-pro-four-ways-for-syncing-with-external-gear/)
- [Virtual MIDI clock or Ableton Link support in rekordbox — Pioneer DJ Community](https://community.pioneerdj.com/hc/en-us/community/posts/22977304833049-Virtual-MIDI-Clock-or-Ableton-Link-support-in-Rekordbox-DJ)
- [JamesMartinGithub/Rekordbox-Midi-Clock](https://github.com/JamesMartinGithub/Rekordbox-Midi-Clock)
- [Pro DJ Link: 5 "secret" features of Pioneer DJ's protocol — DJ TechTools](https://djtechtools.com/2018/10/08/pro-dj-link-5-secret-features-of-pioneer-djs-protocol/)
