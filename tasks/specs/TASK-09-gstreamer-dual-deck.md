# Task Specification: TASK-09 — Dual-Deck GStreamer Hardware HEVC Loop

## 1. Goal
Implement the concrete `VideoDeck` backend in `vj-video` utilizing GStreamer's V4L2 stateless HEVC decoder (`v4l2slh265dec`) for Raspberry Pi 5 zero-copy playback with seamless looping and deck pre-roll.

## 2. Target Files
*   **Modify**: `vj/crates/vj-video/Cargo.toml`
*   **New**: `vj/crates/vj-video/src/gstreamer_deck.rs`
*   **Modify**: `vj/crates/vj-video/src/lib.rs`

## 3. Requirements & Constraints
1. **Pipeline Specification**:
   - `filesrc location=<path> ! qtdemux ! h265parse ! v4l2slh265dec ! appsink name=sink`
   - Configure `appsink` with `drop=true`, `max-buffers=2`, `sync=true`.
2. **Deck State Lifecycle**:
   - `prepare(clip)`: Spawns decoder pipeline, pre-rolls to frame 0, holds `DeckState::Ready`.
   - `activate(token, at)`: Commences clock playback, transitions to `DeckState::Playing`.
   - `seek(position)`: Accurate keyframe seek.
   - `latest_frame()`: Extracts current frame DMA-BUF / texture handle without CPU memcpy.
3. **Seamless Looping**:
   - On End-Of-Stream (EOS), seek back to position 0 without dropping the deck state or causing visual glitch.

## 4. Acceptance Criteria & Unit Tests
*   `cargo test -p vj-video` passes with:
    1. `test_deck_prepare_and_activate_lifecycle`: Verify token transitions from `Idle` -> `Preparing` -> `Ready` -> `Playing`.
    2. `test_deck_error_on_invalid_file`: Non-existent or corrupt clip returns `VideoError::NotFound` or `VideoError::Decoder`.
