use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShowState {
    pub version: u64,
    pub show_name: String,
    pub active_bank: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_clip: Option<u32>,
    pub blackout: bool,
    pub master_opacity: f32,
    pub tempo_bpm: f32,
    /// Raw key index (0-31) of the most recent physical press, regardless
    /// of what that key means on the currently active page. The gig web
    /// engine's own `handleKeyPress`/`handleSystemKey`/`SYS_PAGES` tables
    /// are the single source of truth for what each key does per page
    /// (RESET, BLACK, nav, SPEED/BPM/OPACITY/FX-AMOUNT presets on pages
    /// 1-5, plain clip/shader/effect pads elsewhere) -- the broker/native
    /// side does not attempt to duplicate that per-page logic, it only
    /// forwards which key was pressed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_key_press: Option<u32>,
    /// Increments ONLY on a `key.press` command, never on any other
    /// command (tempo.bpm, master.opacity, property.set, ...). Deliberately
    /// separate from `version` (which bumps on every state-changing
    /// command) so a poller can detect every individual press -- including
    /// repeats of the same key -- without being confused by an unrelated
    /// command bumping `version` and causing a stale `last_key_press` to be
    /// re-forwarded.
    #[serde(default)]
    pub key_press_seq: u64,
    #[serde(default)]
    pub properties: serde_json::Value,
}

impl Default for ShowState {
    fn default() -> Self {
        Self {
            version: 1,
            show_name: "default".to_string(),
            active_bank: 0,
            active_clip: None,
            blackout: false,
            master_opacity: 1.0,
            tempo_bpm: 120.0,
            last_key_press: None,
            key_press_seq: 0,
            properties: serde_json::json!({}),
        }
    }
}

pub struct StateStore {
    state: ShowState,
    persist_path: Option<PathBuf>,
}

impl StateStore {
    pub fn new(persist_path: Option<PathBuf>) -> Self {
        let mut initial_state = ShowState::default();

        if let Some(ref path) = persist_path {
            if path.is_file() {
                if let Ok(content) = fs::read_to_string(path) {
                    if let Ok(loaded) = serde_json::from_str::<ShowState>(&content) {
                        initial_state = loaded;
                    }
                }
            }
        }

        Self {
            state: initial_state,
            persist_path,
        }
    }

    pub fn get_state(&self) -> &ShowState {
        &self.state
    }

    /// Mutate state by applying a command and incrementing state version.
    /// Returns (success, detail)
    pub fn apply_command(&mut self, name: &str, args: &serde_json::Value) -> (bool, Option<String>) {
        let mut changed = false;

        match name {
            "key.press" => {
                if let Some(key) = args.get("key").and_then(|v| v.as_u64()) {
                    self.state.last_key_press = Some(key as u32);
                    self.state.key_press_seq += 1;
                    changed = true;
                } else {
                    return (false, Some("Missing or invalid 'key' argument".to_string()));
                }
            }
            "clip.trigger" => {
                if let Some(key) = args.get("key").and_then(|v| v.as_u64()) {
                    let key_u32 = key as u32;
                    if let Some(bank) = args.get("bank").and_then(|v| v.as_u64()) {
                        self.state.active_bank = bank as u32;
                    }
                    self.state.active_clip = Some(key_u32);
                    changed = true;
                } else {
                    return (false, Some("Missing or invalid 'key' argument".to_string()));
                }
            }
            "clip.stop" => {
                self.state.active_clip = None;
                changed = true;
            }
            "bank.select" => {
                if let Some(idx) = args.get("index").and_then(|v| v.as_u64()) {
                    self.state.active_bank = idx as u32;
                    changed = true;
                } else {
                    return (false, Some("Missing or invalid 'index' argument".to_string()));
                }
            }
            "blackout.toggle" => {
                self.state.blackout = !self.state.blackout;
                changed = true;
            }
            "blackout.set" => {
                if let Some(val) = args.get("value").and_then(|v| v.as_bool()) {
                    self.state.blackout = val;
                    changed = true;
                } else {
                    return (false, Some("Missing or invalid 'value' argument".to_string()));
                }
            }
            "master.opacity" => {
                if let Some(val) = args.get("value").and_then(|v| v.as_f64()) {
                    self.state.master_opacity = (val as f32).clamp(0.0, 1.0);
                    changed = true;
                } else {
                    return (false, Some("Missing or invalid 'value' argument".to_string()));
                }
            }
            "tempo.bpm" => {
                if let Some(val) = args.get("bpm").and_then(|v| v.as_f64()) {
                    self.state.tempo_bpm = (val as f32).clamp(20.0, 300.0);
                    changed = true;
                } else {
                    return (false, Some("Missing or invalid 'bpm' argument".to_string()));
                }
            }
            "show.load" => {
                if let Some(name_str) = args.get("name").and_then(|v| v.as_str()) {
                    self.state.show_name = name_str.to_string();
                    self.state.active_clip = None;
                    changed = true;
                } else {
                    return (false, Some("Missing or invalid 'name' argument".to_string()));
                }
            }
            "property.set" => {
                if let (Some(key), Some(val)) = (args.get("key").and_then(|v| v.as_str()), args.get("value")) {
                    if let Some(obj) = self.state.properties.as_object_mut() {
                        obj.insert(key.to_string(), val.clone());
                        changed = true;
                    }
                } else {
                    return (false, Some("Missing 'key' or 'value'".to_string()));
                }
            }
            _ => {
                return (false, Some(format!("Unknown command '{}'", name)));
            }
        }

        if changed {
            self.state.version += 1;
            let _ = self.persist();
        }

        (true, None)
    }

    pub fn persist(&self) -> std::io::Result<()> {
        if let Some(ref path) = self.persist_path {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let tmp_path = path.with_extension("tmp");
            let json = serde_json::to_string_pretty(&self.state)?;
            {
                let mut f = File::create(&tmp_path)?;
                f.write_all(json.as_bytes())?;
                f.sync_all()?;
            }
            fs::rename(&tmp_path, path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_mutation() {
        let mut store = StateStore::new(None);
        assert_eq!(store.get_state().version, 1);
        assert_eq!(store.get_state().active_clip, None);

        let (ok, err) = store.apply_command("clip.trigger", &serde_json::json!({"key": 7, "bank": 1}));
        assert!(ok);
        assert!(err.is_none());
        assert_eq!(store.get_state().version, 2);
        assert_eq!(store.get_state().active_clip, Some(7));
        assert_eq!(store.get_state().active_bank, 1);

        let (ok, _) = store.apply_command("blackout.toggle", &serde_json::json!({}));
        assert!(ok);
        assert!(store.get_state().blackout);
        assert_eq!(store.get_state().version, 3);
    }

    #[test]
    fn test_key_press_records_raw_key_and_always_bumps_seq_on_repeat() {
        let mut store = StateStore::new(None);
        assert_eq!(store.get_state().last_key_press, None);
        assert_eq!(store.get_state().key_press_seq, 0);

        let (ok, _) = store.apply_command("key.press", &serde_json::json!({"key": 26}));
        assert!(ok);
        assert_eq!(store.get_state().last_key_press, Some(26));
        assert_eq!(store.get_state().key_press_seq, 1);

        // Pressing the SAME key again must still bump key_press_seq -- the
        // bridge relies on this to detect repeat presses, not just value
        // changes.
        let (ok, _) = store.apply_command("key.press", &serde_json::json!({"key": 26}));
        assert!(ok);
        assert_eq!(store.get_state().last_key_press, Some(26));
        assert_eq!(store.get_state().key_press_seq, 2);

        // An unrelated command must NOT bump key_press_seq (that would make
        // the bridge re-forward a stale last_key_press).
        let (ok, _) = store.apply_command("blackout.toggle", &serde_json::json!({}));
        assert!(ok);
        assert_eq!(store.get_state().key_press_seq, 2);

        let (ok, err) = store.apply_command("key.press", &serde_json::json!({}));
        assert!(!ok);
        assert!(err.is_some());
    }
}
