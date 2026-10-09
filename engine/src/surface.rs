//! Pure controller view; key labels and commands remain available without a GPU.
use crate::{
    control::{Command, State},
    show::Show,
};
use std::path::PathBuf;
/// What a key is for; selects its anatomy on the deck. Presentation only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyKind {
    /// Unbound: drawn deliberately blank.
    Empty,
    /// A manifest label without a command.
    Info,
    Clip,
    Shader,
    /// Blend, effect, tempo and other adjustments.
    Setting,
    Navigate,
    /// Reset and blackout.
    Danger,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    Idle,
    Active,
    Loading,
    Failed,
    Unavailable,
}
#[derive(Debug, Clone)]
pub struct KeyDescriptor {
    pub label: String,
    pub command: Option<Command>,
    pub thumbnail: Option<PathBuf>,
    pub color: [u8; 3],
    pub kind: KeyKind,
    pub state: KeyState,
    /// Status word or secondary line for the key's foot strip; may be empty.
    pub caption: String,
}
fn key(label: impl Into<String>, command: Option<Command>, color: [u8; 3]) -> KeyDescriptor {
    let label = label.into();
    let kind = match command {
        Some(Command::TriggerClip { .. }) => KeyKind::Clip,
        Some(Command::TriggerShader { .. }) => KeyKind::Shader,
        Some(Command::Page { .. }) => KeyKind::Navigate,
        Some(Command::Reset | Command::Blackout { .. } | Command::DeleteShader) => KeyKind::Danger,
        Some(_) => KeyKind::Setting,
        None if label.trim().is_empty() => KeyKind::Empty,
        None => KeyKind::Info,
    };
    KeyDescriptor {
        label,
        command,
        thumbnail: None,
        color,
        kind,
        state: KeyState::Idle,
        caption: String::new(),
    }
}
/// "SHADERS 01 / 04" on page 1 of 4 becomes "SHADERS": the keys already show the position.
fn page_title(name: &str, page: usize, pages: usize) -> String {
    let mut words: Vec<&str> = name.split_whitespace().collect();
    if let [_, .., current, "/", total] = words[..] {
        if current.parse() == Ok(page + 1) && total.parse() == Ok(pages) {
            words.truncate(words.len() - 3);
        }
    }
    words.join(" ")
}
pub fn page(show: &Show, state: &State) -> Vec<KeyDescriptor> {
    let mut keys = vec![key("", None, [0, 0, 0]); 32];
    if let Some(page) = show.pages.get(state.page) {
        for binding in &page.bindings {
            if let Some(k) = keys.get_mut(binding.key as usize) {
                *k = key(&binding.label, binding.command, binding.color);
            }
        }
    } else {
        for (i, k) in keys.iter_mut().enumerate() {
            let command = state.key(i as u8);
            if let Some(Command::TriggerClip { clip }) = command {
                if let Some(c) = show.clips.get(clip) {
                    *k = key(&c.label, command, [35, 65, 95]);
                }
            }
        }
    }
    for k in &mut keys {
        // `Some(reason)` disables the key; the reason is shown in its foot strip.
        let missing = match k.command {
            Some(Command::TriggerClip { clip }) => {
                if let Some(c) = show.clips.get(clip) {
                    k.thumbnail = c.thumbnail.as_ref().filter(|p| p.is_file()).cloned();
                    (!c.path.is_file()).then_some("NO FILE")
                } else {
                    Some("NO FILE")
                }
            }
            Some(Command::TriggerShader { shader }) => {
                let entry = show.shaders.get(shader);
                k.thumbnail = entry
                    .and_then(|s| s.thumbnail.as_ref())
                    .filter(|p| p.is_file())
                    .cloned();
                if entry.is_none_or(|s| !s.path.is_file()) {
                    Some("NO FILE")
                } else {
                    state.failed_shaders.contains(&shader).then_some("ERROR")
                }
            }
            _ => None,
        };
        if let Some(reason) = missing {
            k.command = None;
            k.state = KeyState::Unavailable;
            k.caption = reason.into();
            k.color = [100, 20, 20];
        }
        // Blend buttons preserve current opacity; TAP timestamp is supplied by the input worker.
        if let Some(Command::SetBlend { mode, .. }) = k.command {
            k.command = Some(Command::SetBlend {
                mode,
                opacity: state.opacity,
            });
        }
        let active = match k.command {
            Some(Command::TriggerClip { clip }) => state.playing == Some(clip),
            Some(Command::TriggerShader { shader }) => {
                state.shader && state.selected_shader == Some(shader)
            }
            Some(Command::SetBlend { mode, .. }) => state.blend == mode,
            Some(Command::SetVideoEffect { effect }) => state.video_effect == effect,
            Some(Command::SetShaderEffect { effect }) => state.shader_effect == effect,
            Some(Command::SetOpacity { opacity }) => state.opacity == opacity,
            Some(Command::ToggleHud) => state.hud,
            _ => false,
        };
        if active {
            k.color = [30, 170, 65];
            k.state = KeyState::Active;
            k.caption = match k.kind {
                KeyKind::Clip => "PLAYING",
                KeyKind::Shader => "LIVE",
                _ => "ON",
            }
            .into();
            // A live shader's key is a toggle: pressing it again switches the shader off.
            if k.kind == KeyKind::Shader {
                k.command = Some(Command::SelectShader { enabled: false });
            }
        }
        if let Some(Command::TriggerClip { clip }) = k.command {
            if state.failed == Some(clip) {
                k.color = [200, 30, 30];
                k.state = KeyState::Failed;
                k.caption = "FAILED".into();
            } else if state.requested == Some(clip) && state.playing != Some(clip) {
                k.color = [180, 110, 15];
                k.state = KeyState::Loading;
                k.caption = "LOADING".into();
            }
        }
    }
    // PREV and NEXT each name the page they lead to: an arrow, its position, its title.
    let pages = state.page_count();
    for (index, delta) in [(24, -1i32), (25, 1)] {
        let target = (state.page as i64 + i64::from(delta)).rem_euclid(pages.max(1) as i64) as usize;
        keys[index] = key(
            format!("{} / {}", target + 1, pages),
            Some(Command::Page { delta }),
            [25, 90, 115],
        );
        keys[index].caption = show
            .pages
            .get(target)
            .map(|p| page_title(&p.name, target, pages))
            .unwrap_or_else(|| "CLIPS".into());
    }
    keys[30] = key("RESET", Some(Command::Reset), [110, 55, 10]);
    keys[31] = key(
        "BLACK",
        Some(Command::Blackout {
            enabled: !state.blackout,
        }),
        if state.blackout {
            [220, 20, 20]
        } else {
            [70, 15, 15]
        },
    );
    if state.blackout {
        keys[31].state = KeyState::Active;
        keys[31].caption = "ON".into();
    }
    // Auto shuffle takes the next system key, on every page that leaves it free.
    if keys[26].kind == KeyKind::Empty {
        let color = if state.auto_shuffle { [30, 170, 65] } else { [70, 50, 120] };
        keys[26] = key("SHUFFLE", Some(Command::ToggleAutoShuffle), color);
        if state.auto_shuffle {
            keys[26].state = KeyState::Active;
            keys[26].caption = "ON".into();
        }
    }
    // The next two name what is set now; a press moves on to the other or the next.
    if keys[27].kind == KeyKind::Empty {
        let (label, color) = if state.line_input { ("LINE", [20, 110, 120]) } else { ("MIC", [120, 85, 20]) };
        keys[27] = key(label, Some(Command::ToggleInputLevel), color);
        keys[27].caption = "INPUT".into();
    }
    if keys[28].kind == KeyKind::Empty {
        let label = crate::audio::REACT_MODES.get(usize::from(state.react)).copied().unwrap_or("PUMP");
        keys[28] = key(label, Some(Command::CycleReact), [110, 40, 95]);
        keys[28].caption = "REACT".into();
    }
    keys
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_assets_do_not_remove_system_controls() {
        let show:Show=serde_json::from_str(r#"{"version":1,"clips":[{"label":"TEST","path":"/missing/media.mp4","thumbnail":null}]}"#).unwrap();
        let keys = page(&show, &State::from_show(&show));
        assert_eq!(keys.len(), 32);
        assert!(keys[0].command.is_none());
        assert_eq!(keys[0].state, KeyState::Unavailable);
        assert_eq!((keys[0].label.as_str(), keys[0].caption.as_str()), ("TEST", "NO FILE"));
        assert_eq!(keys[1].kind, KeyKind::Empty);
        assert_eq!(keys[30].command, Some(Command::Reset));
    }
    #[test]
    fn a_live_shader_key_switches_its_shader_off() {
        let show: Show = serde_json::from_str(
            r#"{"version":2,"audio_device":null,
                "shaders":[{"label":"a","path":"Cargo.toml"},{"label":"b","path":"Cargo.toml"}],
                "pages":[{"name":"S","bindings":[
                    {"key":0,"label":"A","command":{"type":"TriggerShader","shader":0}},
                    {"key":1,"label":"B","command":{"type":"TriggerShader","shader":1}}]}]}"#,
        )
        .unwrap();
        let mut state = State::from_show(&show);
        state.apply(Command::TriggerShader { shader: 0 }).unwrap();
        let keys = page(&show, &state);
        assert_eq!(keys[0].command, Some(Command::SelectShader { enabled: false }));
        assert_eq!((keys[0].state, keys[0].caption.as_str()), (KeyState::Active, "LIVE"));
        assert_eq!(keys[1].command, Some(Command::TriggerShader { shader: 1 }));
        // Off again: the same key triggers the shader as before.
        state.apply(keys[0].command.unwrap()).unwrap();
        let keys = page(&show, &state);
        assert_eq!(keys[0].command, Some(Command::TriggerShader { shader: 0 }));
        assert_eq!(keys[0].state, KeyState::Idle);
    }
    #[test]
    fn the_shuffle_key_toggles_and_leaves_a_bound_key_alone() {
        let show: Show = serde_json::from_str(
            r#"{"version":2,"audio_device":null,"pages":[
                {"name":"A","bindings":[]},
                {"name":"B","bindings":[{"key":26,"label":"MINE","command":{"type":"ToggleHud"}}]}]}"#,
        )
        .unwrap();
        let mut state = State::from_show(&show);
        let keys = page(&show, &state);
        assert_eq!(keys[26].command, Some(Command::ToggleAutoShuffle));
        assert_eq!(keys[26].state, KeyState::Idle);
        state.apply(Command::ToggleAutoShuffle).unwrap();
        let keys = page(&show, &state);
        assert_eq!((keys[26].state, keys[26].caption.as_str()), (KeyState::Active, "ON"));
        // Its neighbours show the current setting and step on when pressed.
        assert_eq!((keys[27].label.as_str(), keys[28].label.as_str()), ("MIC", "PUMP"));
        state.apply(keys[27].command.unwrap()).unwrap();
        for name in ["STROBE", "STEP", "RIDE", "TWIST", "PUMP"] {
            state.apply(keys[28].command.unwrap()).unwrap();
            let keys = page(&show, &state);
            assert_eq!((keys[27].label.as_str(), keys[28].label.as_str()), ("LINE", name));
        }
        // A reset returns the look, not the venue's input setting.
        state.apply(Command::CycleReact).unwrap();
        state.apply(Command::Reset).unwrap();
        assert_eq!((state.react, state.line_input), (0, true));
        state.page = 1;
        assert_eq!(page(&show, &state)[26].command, Some(Command::ToggleHud));
    }
    #[test]
    fn navigation_keys_name_the_page_they_lead_to() {
        let show: Show = serde_json::from_str(
            r#"{"version":2,"audio_device":null,"pages":[
                {"name":"SHADERS 01 / 03","bindings":[{"key":30,"label":"MINE","command":{"type":"ToggleHud"}}]},
                {"name":"Video FX","bindings":[]},
                {"name":"TAKE 2 / 3","bindings":[]}]}"#,
        )
        .unwrap();
        let mut state = State::from_show(&show);
        for (index, previous, next) in [
            (0, ("3 / 3", "TAKE 2 / 3"), ("2 / 3", "Video FX")),
            (1, ("1 / 3", "SHADERS"), ("3 / 3", "TAKE 2 / 3")),
            (2, ("2 / 3", "Video FX"), ("1 / 3", "SHADERS")),
        ] {
            state.page = index;
            let keys = page(&show, &state);
            for (key, delta, expected) in [(24, -1, previous), (25, 1, next)] {
                assert_eq!(keys[key].command, Some(Command::Page { delta }));
                assert_eq!(keys[key].kind, KeyKind::Navigate);
                assert_eq!(
                    (keys[key].label.as_str(), keys[key].caption.as_str()),
                    expected
                );
            }
        }
        // The system keys still replace whatever the manifest binds to 30 and 31.
        state.page = 0;
        state.blackout = true;
        let keys = page(&show, &state);
        assert_eq!(keys[30].label, "RESET");
        assert_eq!(keys[30].command, Some(Command::Reset));
        assert_eq!(keys[31].command, Some(Command::Blackout { enabled: false }));
        assert_eq!((keys[31].state, keys[31].caption.as_str()), (KeyState::Active, "ON"));
    }
}
