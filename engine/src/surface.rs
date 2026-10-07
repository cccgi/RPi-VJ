//! Pure controller view; key labels and commands remain available without a GPU.
use crate::{
    control::{Command, State},
    show::Show,
};
use std::path::PathBuf;
#[derive(Debug, Clone)]
pub struct KeyDescriptor {
    pub label: String,
    pub command: Option<Command>,
    pub thumbnail: Option<PathBuf>,
    pub color: [u8; 3],
}
fn key(label: impl Into<String>, command: Option<Command>, color: [u8; 3]) -> KeyDescriptor {
    KeyDescriptor {
        label: label.into(),
        command,
        thumbnail: None,
        color,
    }
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
        let missing = match k.command {
            Some(Command::TriggerClip { clip }) => {
                if let Some(c) = show.clips.get(clip) {
                    k.thumbnail = c.thumbnail.as_ref().filter(|p| p.is_file()).cloned();
                    !c.path.is_file()
                } else {
                    true
                }
            }
            Some(Command::TriggerShader { shader }) => {
                show.shaders.get(shader).is_none_or(|s| !s.path.is_file())
                    || state.failed_shaders.contains(&shader)
            }
            _ => false,
        };
        if missing {
            k.command = None;
            k.label = format!("UNAVAIL {}", k.label);
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
        }
        if let Some(Command::TriggerClip { clip }) = k.command {
            if state.failed == Some(clip) {
                k.color = [200, 30, 30];
                k.label = format!("FAILED {}", k.label);
            } else if state.requested == Some(clip) && state.playing != Some(clip) {
                k.color = [180, 110, 15];
                k.label = format!("LOAD {}", k.label);
            }
        }
    }
    keys[24] = key(
        format!("PREV {}", state.page + 1),
        Some(Command::Page { delta: -1 }),
        [25, 90, 115],
    );
    keys[25] = key(
        show.pages
            .get(state.page)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "NEXT".into()),
        Some(Command::Page { delta: 1 }),
        [25, 90, 115],
    );
    keys[30] = key("RESET", Some(Command::Reset), [110, 55, 10]);
    keys[31] = key(
        if state.blackout { "BLACK ON" } else { "BLACK" },
        Some(Command::Blackout {
            enabled: !state.blackout,
        }),
        if state.blackout {
            [220, 20, 20]
        } else {
            [70, 15, 15]
        },
    );
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
        assert!(keys[0].label.contains("UNAVAIL"));
        assert_eq!(keys[30].command, Some(Command::Reset));
    }
}
