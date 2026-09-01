//! Stream Deck XL device implementation and JPEG icon cache.

use vj_types::SurfaceId;

use crate::{ELGATO_VENDOR_ID, STREAM_DECK_XL_PRODUCT_IDS};

pub const STREAM_DECK_XL_KEY_COUNT: usize = 32;
pub const STREAM_DECK_XL_ICON_SIZE: u32 = 96;
pub const STREAM_DECK_XL_ROWS: usize = 4;
pub const STREAM_DECK_XL_COLS: usize = 8;

pub const STREAM_DECK_XL_REPORT_SIZE: usize = 1024;
pub const STREAM_DECK_XL_HEADER_SIZE: usize = 8;
pub const STREAM_DECK_XL_PAYLOAD_SIZE: usize = STREAM_DECK_XL_REPORT_SIZE - STREAM_DECK_XL_HEADER_SIZE;

/// Icon cache for Stream Deck XL 32 physical keys (96x96 JPEG).
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

    /// Update the icon for a specific key.
    /// Returns true if the icon content hash changed (dirty), false if identical.
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

    /// Update all icons, returning the indices of keys that changed.
    pub fn update_all(&mut self, new_icons: &[Option<Vec<u8>>; STREAM_DECK_XL_KEY_COUNT]) -> Vec<usize> {
        let mut dirty = Vec::new();
        for (i, icon) in new_icons.iter().enumerate() {
            if self.set_icon(i, icon.clone()) {
                dirty.push(i);
            }
        }
        dirty
    }

    /// Invalidate all cached hashes to force retransmission of all populated icons on reconnect / bank reload.
    pub fn mark_all_dirty(&mut self) -> Vec<usize> {
        (0..STREAM_DECK_XL_KEY_COUNT)
            .filter(|&i| self.entries[i].is_some())
            .collect()
    }

    /// Clear the icon cache.
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

    /// Get all entries.
    pub fn entries(&self) -> &[Option<Vec<u8>>; STREAM_DECK_XL_KEY_COUNT] {
        &self.entries
    }

    fn compute_hash(data: &[u8]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;
        let mut hasher = DefaultHasher::new();
        hasher.write(data);
        hasher.finish()
    }
}

/// Generate the HID output report packets for transmitting a JPEG image to a given key on Stream Deck XL.
pub fn frame_key_image_report(key: u8, jpeg_data: &[u8]) -> Vec<Vec<u8>> {
    let mut reports = Vec::new();
    let total_len = jpeg_data.len();
    if total_len == 0 {
        return reports;
    }

    let chunks: Vec<&[u8]> = jpeg_data.chunks(STREAM_DECK_XL_PAYLOAD_SIZE).collect();
    let num_chunks = chunks.len();

    for (chunk_idx, chunk) in chunks.iter().enumerate() {
        let is_last = if chunk_idx == num_chunks - 1 { 1u8 } else { 0u8 };
        let mut report = vec![0u8; STREAM_DECK_XL_REPORT_SIZE];
        report[0] = 0x02; // Report ID
        report[1] = 0x07; // Set Image Command
        report[2] = key;  // Key Index
        report[3] = is_last; // Is Last Chunk
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

/// Stream Deck XL controller driver representation.
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

    pub fn vendor_id() -> u16 {
        ELGATO_VENDOR_ID
    }

    pub fn is_supported_product_id(pid: u16) -> bool {
        STREAM_DECK_XL_PRODUCT_IDS.contains(&pid)
    }

    /// Ingest a raw 32-button boolean array from a poll tick.
    /// Returns a list of (key_index, is_pressed) transitions.
    /// 0 -> 1 transitions represent Press, and 1 -> 0 represent Release.
    pub fn ingest_buttons(&mut self, next: &[bool; STREAM_DECK_XL_KEY_COUNT]) -> Vec<(usize, bool)> {
        let mut edges = Vec::new();
        for (i, (&p, &n)) in self.prev_state.iter().zip(next.iter()).enumerate() {
            if p != n {
                edges.push((i, n));
            }
        }
        self.prev_state = *next;
        edges
    }

    /// Process a raw HID input report from Stream Deck XL.
    /// Standard input report header is 4 bytes followed by 32 button state bytes.
    pub fn parse_input_report(&mut self, report: &[u8]) -> Option<Vec<(usize, bool)>> {
        if report.len() >= 36 && report[0] == 0x01 {
            let mut state = [false; STREAM_DECK_XL_KEY_COUNT];
            for i in 0..STREAM_DECK_XL_KEY_COUNT {
                state[i] = report[4 + i] != 0;
            }
            Some(self.ingest_buttons(&state))
        } else if report.len() >= STREAM_DECK_XL_KEY_COUNT {
            let mut state = [false; STREAM_DECK_XL_KEY_COUNT];
            for i in 0..STREAM_DECK_XL_KEY_COUNT {
                state[i] = report[i] != 0;
            }
            Some(self.ingest_buttons(&state))
        } else {
            None
        }
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
    fn test_held_key_fires_only_once() {
        let mut dev = StreamDeckXL::new(SurfaceId::new("sdxl-test"));
        let mut held_state = [false; STREAM_DECK_XL_KEY_COUNT];
        held_state[3] = true; // Key 3 is pressed

        // First poll: 0 -> 1 should emit 1 press event for key 3
        let first_events = dev.ingest_buttons(&held_state);
        assert_eq!(first_events, vec![(3, true)]);

        // Polling 100 times with the held key should produce NO duplicate press events
        for _ in 0..100 {
            let events = dev.ingest_buttons(&held_state);
            assert!(events.is_empty(), "Held key must not fire duplicate press events");
        }

        // Releasing key: 1 -> 0 should emit 1 release event for key 3
        let release_state = [false; STREAM_DECK_XL_KEY_COUNT];
        let release_events = dev.ingest_buttons(&release_state);
        assert_eq!(release_events, vec![(3, false)]);

        // Subsequent polls with released key produce no events
        for _ in 0..10 {
            assert!(dev.ingest_buttons(&release_state).is_empty());
        }
    }

    #[test]
    fn test_simultaneous_presses() {
        let mut dev = StreamDeckXL::new(SurfaceId::new("sdxl-test"));
        let mut simultaneous = [false; STREAM_DECK_XL_KEY_COUNT];
        simultaneous[0] = true;
        simultaneous[7] = true;
        simultaneous[15] = true;
        simultaneous[31] = true;

        let events = dev.ingest_buttons(&simultaneous);
        assert_eq!(events, vec![(0, true), (7, true), (15, true), (31, true)]);

        // Release two, hold two
        let mut partial_release = simultaneous;
        partial_release[0] = false;
        partial_release[31] = false;
        let release_events = dev.ingest_buttons(&partial_release);
        assert_eq!(release_events, vec![(0, false), (31, false)]);
    }

    #[test]
    fn test_icon_cache_diffing() {
        let mut dev = StreamDeckXL::new(SurfaceId::new("sdxl-test"));

        // Initial setup with 32 dummy JPEG icon payloads
        let mut initial_icons: [Option<Vec<u8>>; STREAM_DECK_XL_KEY_COUNT] = std::array::from_fn(|i| {
            Some(vec![0xFF, 0xD8, 0xFF, i as u8, 0x00, 0x96])
        });

        // First load marks all 32 dirty
        let dirty_initial = dev.update_icons(&initial_icons);
        assert_eq!(dirty_initial.len(), 32);

        // Re-applying identical icons should mark 0 dirty
        let dirty_noop = dev.update_icons(&initial_icons);
        assert!(dirty_noop.is_empty());

        // Modify exactly 1 icon out of 32 (key 12)
        initial_icons[12] = Some(vec![0xFF, 0xD8, 0xFF, 0x99, 0xAA, 0xBB]);
        let dirty_changed = dev.update_icons(&initial_icons);
        assert_eq!(dirty_changed, vec![12], "Changing 1 icon out of 32 only marks 1 icon dirty");

        // Verify framed report generation for that 1 dirty key
        let reports = dev.emit_dirty_image_reports(&dirty_changed);
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].0, 12);
        assert!(!reports[0].1.is_empty());
        assert_eq!(reports[0].1[0][0], 0x02); // Report ID
        assert_eq!(reports[0].1[0][1], 0x07); // Set Image cmd
        assert_eq!(reports[0].1[0][2], 12);   // Key index
    }
}
