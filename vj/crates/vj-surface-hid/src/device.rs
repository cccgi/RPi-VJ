//! Stream Deck XL device protocol, button mapping, report framing, and icon caching.

use crate::SurfaceId;

pub const ELGATO_VENDOR_ID: u16 = 0x0fd9;
/// Stream Deck XL product IDs (v1: 0x006c, v2: 0x008f).
pub const STREAM_DECK_XL_PRODUCT_IDS: [u16; 2] = [0x006c, 0x008f];

pub const STREAM_DECK_XL_KEY_COUNT: usize = 32;
pub const STREAM_DECK_XL_ICON_SIZE: u32 = 96;
pub const STREAM_DECK_XL_ROWS: usize = 4;
pub const STREAM_DECK_XL_COLS: usize = 8;

pub const STREAM_DECK_XL_REPORT_SIZE: usize = 1024;
pub const STREAM_DECK_XL_HEADER_SIZE: usize = 8;
pub const STREAM_DECK_XL_PAYLOAD_SIZE: usize = STREAM_DECK_XL_REPORT_SIZE - STREAM_DECK_XL_HEADER_SIZE; // 1016 bytes

/// Convert key index (0..31) to 2D grid coordinate (row, col).
pub fn key_to_row_col(key: u8) -> (u8, u8) {
    (key / STREAM_DECK_XL_COLS as u8, key % STREAM_DECK_XL_COLS as u8)
}

/// Convert 2D grid coordinate (row, col) to sequential key index (0..31).
pub fn row_col_to_key(row: u8, col: u8) -> u8 {
    row * STREAM_DECK_XL_COLS as u8 + col
}

/// Diff a fresh button state slice against previous state, emitting only 0->1 (Press) and 1->0 (Release) transitions.
pub fn diff_buttons(prev: &[bool], next: &[bool]) -> Vec<(usize, bool)> {
    prev.iter()
        .zip(next.iter())
        .enumerate()
        .filter_map(|(i, (&p, &n))| (p != n).then_some((i, n)))
        .collect()
}

/// 32-slot JPEG icon cache with hash-based dirty tracking.
#[derive(Debug, Clone, Default)]
pub struct IconCache {
    entries: [Option<Vec<u8>>; STREAM_DECK_XL_KEY_COUNT],
    hashes: [u64; STREAM_DECK_XL_KEY_COUNT],
}

impl IconCache {
    pub fn new() -> Self {
        Self {
            entries: Default::default(),
            hashes: [0; STREAM_DECK_XL_KEY_COUNT],
        }
    }

    /// Update icon for key. Returns true if dirty (content changed), false if unchanged (coalesced).
    pub fn set_icon(&mut self, key: usize, jpeg_data: Option<Vec<u8>>) -> bool {
        if key >= STREAM_DECK_XL_KEY_COUNT {
            return false;
        }
        let new_hash = match &jpeg_data {
            Some(bytes) => Self::compute_hash(bytes),
            None => 0,
        };

        let changed = self.hashes[key] != new_hash || self.entries[key] != jpeg_data;
        if changed {
            self.hashes[key] = new_hash;
            self.entries[key] = jpeg_data;
        }
        changed
    }

    /// Update all icons, returning list of dirty key indices.
    pub fn update_all(&mut self, new_icons: &[Option<Vec<u8>>; STREAM_DECK_XL_KEY_COUNT]) -> Vec<usize> {
        let mut dirty = Vec::new();
        for (i, icon) in new_icons.iter().enumerate() {
            if self.set_icon(i, icon.clone()) {
                dirty.push(i);
            }
        }
        dirty
    }

    /// Mark all currently populated icons dirty to trigger replay on reconnect.
    pub fn mark_all_dirty(&mut self) -> Vec<usize> {
        (0..STREAM_DECK_XL_KEY_COUNT)
            .filter(|&i| self.entries[i].is_some())
            .collect()
    }

    /// Reset cache.
    pub fn clear(&mut self) {
        self.entries = Default::default();
        self.hashes = [0; STREAM_DECK_XL_KEY_COUNT];
    }

    /// Get cached JPEG icon bytes by key index.
    pub fn get_icon(&self, key: usize) -> Option<&[u8]> {
        if key < STREAM_DECK_XL_KEY_COUNT {
            self.entries[key].as_deref()
        } else {
            None
        }
    }

    pub fn entries(&self) -> &[Option<Vec<u8>>; STREAM_DECK_XL_KEY_COUNT] {
        &self.entries
    }

    pub fn populated_count(&self) -> usize {
        self.entries.iter().filter(|e| e.is_some()).count()
    }

    fn compute_hash(data: &[u8]) -> u64 {
        let mut hash = 0xcbf29ce484222325u64; // FNV-1a 64-bit
        for &byte in data {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3u64);
        }
        hash
    }
}

/// Generate HID output reports for transmitting a JPEG image to a key on Stream Deck XL.
/// Report layout:
/// - byte 0: Report ID (0x02)
/// - byte 1: Command (0x07 = Set Image)
/// - byte 2: Key index (0..31, matching physical hardware layout)
/// - byte 3: Is last chunk (0x01 or 0x00)
/// - bytes 4..5: Chunk length in bytes (u16 little-endian)
/// - bytes 6..7: Chunk sequence index (u16 little-endian)
/// - bytes 8..8+len: JPEG payload slice
pub fn frame_key_image_report(key: u8, jpeg_data: &[u8]) -> Vec<Vec<u8>> {
    let mut reports = Vec::new();
    let total_len = jpeg_data.len();

    if total_len == 0 {
        // Clearing a key: emit single empty payload chunk
        let mut report = vec![0u8; STREAM_DECK_XL_REPORT_SIZE];
        report[0] = 0x02; // Report ID
        report[1] = 0x07; // Set Image Command
        report[2] = key;  // Key Index
        report[3] = 0x01; // Is last chunk
        reports.push(report);
        return reports;
    }

    let chunks: Vec<&[u8]> = jpeg_data.chunks(STREAM_DECK_XL_PAYLOAD_SIZE).collect();
    let num_chunks = chunks.len();

    for (chunk_idx, chunk) in chunks.iter().enumerate() {
        let is_last = if chunk_idx == num_chunks - 1 { 1u8 } else { 0u8 };
        let mut report = vec![0u8; STREAM_DECK_XL_REPORT_SIZE];
        report[0] = 0x02;
        report[1] = 0x07;
        report[2] = key;
        report[3] = is_last;
        let chunk_len = chunk.len() as u16;
        report[4] = (chunk_len & 0xFF) as u8;
        report[5] = ((chunk_len >> 8) & 0xFF) as u8;
        let chunk_num = chunk_idx as u16;
        report[6] = (chunk_num & 0xFF) as u8;
        report[7] = ((chunk_num >> 8) & 0xFF) as u8;
        report[8..8 + chunk.len()].copy_from_slice(chunk);
        reports.push(report);
    }

    reports
}

/// Generate HID feature report for setting Stream Deck XL brightness (0..100%).
pub fn frame_brightness_report(pct: u8) -> Vec<u8> {
    let mut report = vec![0u8; 32];
    report[0] = 0x03; // Feature Report ID
    report[1] = 0x08; // Set Brightness Command
    report[2] = pct.min(100);
    report
}

/// Generate HID feature report for resetting Stream Deck XL.
pub fn frame_reset_report() -> Vec<u8> {
    let mut report = vec![0u8; 32];
    report[0] = 0x03; // Feature Report ID
    report[1] = 0x02; // Reset Command
    report
}

/// Stream Deck XL device state engine.
pub struct StreamDeckXL {
    pub surface: SurfaceId,
    pub connected: bool,
    prev_state: [bool; STREAM_DECK_XL_KEY_COUNT],
    icon_cache: IconCache,
}

impl StreamDeckXL {
    pub fn new(surface: SurfaceId) -> Self {
        Self {
            surface,
            connected: false,
            prev_state: [false; STREAM_DECK_XL_KEY_COUNT],
            icon_cache: IconCache::new(),
        }
    }

    pub fn is_supported(vid: u16, pid: u16) -> bool {
        vid == ELGATO_VENDOR_ID && STREAM_DECK_XL_PRODUCT_IDS.contains(&pid)
    }

    /// Reset internal key tracking to all released.
    /// Emits release transitions for any keys that were held before disconnect.
    pub fn reset_button_state(&mut self) -> Vec<(usize, bool)> {
        let released = [false; STREAM_DECK_XL_KEY_COUNT];
        self.ingest_buttons(&released)
    }

    /// Ingest a raw 32-button boolean array from a poll tick.
    /// Returns only the edge transitions: (key_index, is_pressed).
    pub fn ingest_buttons(&mut self, next: &[bool; STREAM_DECK_XL_KEY_COUNT]) -> Vec<(usize, bool)> {
        let edges = diff_buttons(&self.prev_state, next);
        self.prev_state = *next;
        edges
    }

    /// Process a raw HID input report from the Stream Deck XL.
    ///
    /// Handles both:
    /// 1. Linux hidraw 511-byte report:
    ///    - byte 0 = 0x01 (Report ID)
    ///    - byte 1..3 = header metadata (byte 2 = 0x20 = 32 keys)
    ///    - bytes 4..36 = 32 key state bytes (offset = 4)
    /// 2. Stripped report ID (e.g. WebHID / some platform drivers):
    ///    - byte 1 = 32, offset = 3
    pub fn parse_input_report(&mut self, report: &[u8]) -> Option<Vec<(usize, bool)>> {
        if report.is_empty() {
            return None;
        }

        let offset = if report.len() >= 36 && report[0] == 0x01 {
            4
        } else if report.len() >= 35 && report.get(1) == Some(&32) {
            3
        } else {
            // Fallback scan for 32-key count marker
            let mut found = None;
            for (i, &b) in report.iter().take(8).enumerate() {
                if b == STREAM_DECK_XL_KEY_COUNT as u8 {
                    found = Some(i + 2);
                    break;
                }
            }
            match found {
                Some(off) if report.len() >= off + STREAM_DECK_XL_KEY_COUNT => off,
                _ => return None,
            }
        };

        if report.len() < offset + STREAM_DECK_XL_KEY_COUNT {
            return None;
        }

        let mut state = [false; STREAM_DECK_XL_KEY_COUNT];
        for i in 0..STREAM_DECK_XL_KEY_COUNT {
            state[i] = report[offset + i] != 0;
        }

        Some(self.ingest_buttons(&state))
    }

    pub fn icon_cache(&self) -> &IconCache {
        &self.icon_cache
    }

    pub fn icon_cache_mut(&mut self) -> &mut IconCache {
        &mut self.icon_cache
    }

    pub fn set_key_icon(&mut self, key: usize, jpeg: Option<Vec<u8>>) -> bool {
        self.icon_cache.set_icon(key, jpeg)
    }

    pub fn update_icons(&mut self, new_icons: &[Option<Vec<u8>>; STREAM_DECK_XL_KEY_COUNT]) -> Vec<usize> {
        self.icon_cache.update_all(new_icons)
    }

    pub fn emit_dirty_image_reports(&self, dirty_keys: &[usize]) -> Vec<(usize, Vec<Vec<u8>>)> {
        let mut framed = Vec::new();
        for &key in dirty_keys {
            if let Some(jpeg) = self.icon_cache.get_icon(key) {
                let reports = frame_key_image_report(key as u8, jpeg);
                framed.push((key, reports));
            }
        }
        framed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_conversion() {
        assert_eq!(key_to_row_col(0), (0, 0));
        assert_eq!(key_to_row_col(7), (0, 7));
        assert_eq!(key_to_row_col(8), (1, 0));
        assert_eq!(key_to_row_col(31), (3, 7));

        for key in 0..32u8 {
            let (r, c) = key_to_row_col(key);
            assert_eq!(row_col_to_key(r, c), key);
        }
    }

    #[test]
    fn test_held_key_fires_exactly_once() {
        let mut dev = StreamDeckXL::new(SurfaceId::new("test"));
        let mut state = [false; STREAM_DECK_XL_KEY_COUNT];
        state[15] = true;

        let events = dev.ingest_buttons(&state);
        assert_eq!(events, vec![(15, true)]);

        // 100 consecutive polls while key is held: NO events emitted!
        for _ in 0..100 {
            assert!(dev.ingest_buttons(&state).is_empty());
        }

        // Release key: exactly one release event
        state[15] = false;
        let release = dev.ingest_buttons(&state);
        assert_eq!(release, vec![(15, false)]);

        // 10 consecutive polls while released: NO events emitted
        for _ in 0..10 {
            assert!(dev.ingest_buttons(&state).is_empty());
        }
    }

    #[test]
    fn test_all_32_keys_produce_correct_index_exactly_once() {
        let mut dev = StreamDeckXL::new(SurfaceId::new("test"));
        for k in 0..32 {
            let mut state = [false; STREAM_DECK_XL_KEY_COUNT];
            state[k] = true;
            let press = dev.ingest_buttons(&state);
            assert_eq!(press, vec![(k, true)], "Key {k} must produce exact logical index {k}");

            state[k] = false;
            let rel = dev.ingest_buttons(&state);
            assert_eq!(rel, vec![(k, false)], "Key {k} release must produce exact logical index {k}");
        }
    }

    #[test]
    fn test_simultaneous_multi_key_presses() {
        let mut dev = StreamDeckXL::new(SurfaceId::new("test"));
        let mut state = [false; STREAM_DECK_XL_KEY_COUNT];
        state[0] = true;
        state[10] = true;
        state[20] = true;
        state[30] = true;

        let events = dev.ingest_buttons(&state);
        assert_eq!(events, vec![(0, true), (10, true), (20, true), (30, true)]);

        // Release 10 and 30, keep 0 and 20 held
        state[10] = false;
        state[30] = false;
        let rel = dev.ingest_buttons(&state);
        assert_eq!(rel, vec![(10, false), (30, false)]);
    }

    #[test]
    fn test_parse_input_report_standard_hidraw() {
        let mut dev = StreamDeckXL::new(SurfaceId::new("test"));
        let mut report = vec![0u8; 511];
        report[0] = 0x01; // Report ID
        report[1] = 0x00;
        report[2] = 0x20; // 32 keys
        report[3] = 0x00;
        report[4 + 27] = 0x01; // Key 27 pressed (as measured on production hardware)

        let parsed = dev.parse_input_report(&report).expect("parse successful");
        assert_eq!(parsed, vec![(27, true)]);
    }

    #[test]
    fn test_icon_framing_matches_keys() {
        for k in 0..32u8 {
            let jpeg_payload = vec![0xFF, 0xD8, 0xFF, 0xE0, k, 0x00, 0x55];
            let reports = frame_key_image_report(k, &jpeg_payload);
            assert_eq!(reports.len(), 1);
            assert_eq!(reports[0][0], 0x02, "Report ID must be 0x02");
            assert_eq!(reports[0][1], 0x07, "Command must be 0x07 (Set Image)");
            assert_eq!(reports[0][2], k, "Key in packet must match logical index {k}");
            assert_eq!(reports[0][3], 0x01, "Is last chunk must be 1");
            assert_eq!(&reports[0][8..8 + jpeg_payload.len()], &jpeg_payload[..]);
        }
    }

    #[test]
    fn test_large_jpeg_chunking() {
        let large_payload = vec![0xAA; 2500]; // spans 3 chunks (1016 + 1016 + 468)
        let reports = frame_key_image_report(5, &large_payload);
        assert_eq!(reports.len(), 3);
        assert_eq!(reports[0][3], 0x00); // not last
        assert_eq!(reports[1][3], 0x00); // not last
        assert_eq!(reports[2][3], 0x01); // last
        assert_eq!(reports[2][2], 5);
    }
}
