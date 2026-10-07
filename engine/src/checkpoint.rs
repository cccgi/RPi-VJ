//! Bounded atomic desired-state checkpoints, never on the USB or renderer thread.
use crate::{control::State, show::Show};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
#[derive(Serialize, Deserialize)]
struct Saved {
    version: u32,
    show: u64,
    state: State,
}
fn identity(show: &Show) -> Result<u64> {
    // Compatibility fingerprint, not a security signature.
    Ok(serde_json::to_vec(show)?
        .iter()
        .fold(0xcbf29ce484222325, |h, b| {
            (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
        }))
}
pub fn load(path: &Path, show: &Show) -> Result<State> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(65537).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 65536, "checkpoint too large");
    let saved: Saved = serde_json::from_slice(&bytes)?;
    ensure!(
        saved.version == 1 && saved.show == identity(show)? && saved.state.validate_for(show),
        "checkpoint incompatible with show"
    );
    let mut state = saved.state;
    state.recover();
    Ok(state)
}
pub fn save(path: &Path, show: &Show, state: &State) -> Result<()> {
    use std::io::Write;
    ensure!(state.validate_for(show), "invalid checkpoint state");
    let bytes = serde_json::to_vec(&Saved {
        version: 1,
        show: identity(show)?,
        state: state.clone(),
    })?;
    ensure!(bytes.len() <= 65536, "checkpoint too large");
    let temporary = path.with_extension("tmp");
    let mut file = std::fs::File::create(&temporary)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&temporary, path)?;
    #[cfg(unix)]
    if let Some(parent) = path.parent() {
        std::fs::File::open(parent)?.sync_all()?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checkpoint_restores_intent_not_false_playing() {
        let show: Show =
            serde_json::from_str(r#"{"version":1,"clips":[{"label":"A","path":"a.mp4"}]}"#)
                .unwrap();
        let path = std::env::temp_dir().join(format!("vj-checkpoint-{}.json", std::process::id()));
        let mut state = State::from_show(&show);
        state
            .apply(crate::control::Command::TriggerClip { clip: 0 })
            .unwrap();
        state.presented(0, 1);
        state.blackout = true;
        save(&path, &show, &state).unwrap();
        let recovered = load(&path, &show).unwrap();
        assert_eq!(recovered.requested, Some(0));
        assert_eq!(recovered.playing, None);
        assert!(recovered.blackout);
        let mut other = show.clone();
        other.bpm = 130.0;
        assert!(load(&path, &other).is_err());
        std::fs::remove_file(path).unwrap();
    }
}
