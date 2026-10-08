//! Independently supervised controller. A dead GPU cannot stop its state machine.
#[cfg(any(unix, test))]
use crate::control::Command;
use crate::{
    control::State,
    ipc::{Message, Playing},
    show::Show,
};
use anyhow::Result;
use std::path::Path;

/// Applying a renderer report is monotonic and never changes the desired page or effects.
pub fn apply_report(state: &mut State, expected_session: u64, report: &Message) -> bool {
    if let Message::Report {
        session,
        playing,
        failed,
        shader_errors,
        ..
    } = report
    {
        if *session != expected_session {
            return false;
        }
        let before = (state.playing, state.requested, state.failed);
        let unavailable: Vec<_> = shader_errors.iter().map(|error| error.shader).collect();
        let shaders_changed = state.failed_shaders != unavailable;
        state.failed_shaders = unavailable;
        if let Some(Playing { clip, generation }) = playing {
            state.presented(*clip, *generation);
        }
        if let Some(failure) = failed {
            if state.generation == failure.generation && state.requested.is_some() {
                state.rejected(failure.generation);
            }
        }
        shaders_changed || before != (state.playing, state.requested, state.failed)
    } else {
        false
    }
}

#[cfg(not(unix))]
pub fn run(_show: Show, _path: &Path) -> Result<()> {
    anyhow::bail!("controller IPC requires Linux/Unix; portable controller tests remain available")
}

#[cfg(unix)]
pub fn run(show: Show, path: &Path) -> Result<()> {
    use crate::{controller::Deck, ipc::Connection, notify};
    use std::{
        os::unix::{
            fs::{FileTypeExt, PermissionsExt},
            net::{UnixListener, UnixStream},
        },
        sync::Arc,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };
    let show = Arc::new(show);
    for warning in &show.warnings {
        eprintln!("show: {warning}");
    }
    let mut state = State::from_show(&show);
    let checkpoint_path = std::env::var_os("RPIVJ_STATE_PATH").map(std::path::PathBuf::from);
    let restored =
        checkpoint_path
            .as_ref()
            .and_then(|path| match crate::checkpoint::load(path, &show) {
                Ok(state) => Some(state),
                Err(error) => {
                    eprintln!("starting without checkpoint: {error:#}");
                    None
                }
            });
    if let Some(saved) = restored {
        state = saved;
    } else if let Some(clip) = show.clips.iter().position(|c| c.path.is_file()) {
        state
            .apply(Command::TriggerClip { clip })
            .map_err(anyhow::Error::msg)?;
    }
    let checkpoint = if let Some(path) = checkpoint_path {
        let (tx, rx) = crossbeam_channel::bounded::<State>(1);
        let stale = rx.clone();
        let show = Arc::clone(&show);
        std::thread::Builder::new()
            .name("state-checkpoint".into())
            .spawn(move || {
                while let Ok(mut state) = rx.recv() {
                    std::thread::sleep(Duration::from_millis(500));
                    while let Ok(new) = rx.try_recv() {
                        state = new;
                    }
                    if let Err(error) = crate::checkpoint::save(&path, &show, &state) {
                        eprintln!("checkpoint failed: {error:#}");
                    }
                }
            })?;
        Some((tx, stale))
    } else {
        None
    };
    if let Ok(metadata) = std::fs::symlink_metadata(path) {
        anyhow::ensure!(
            metadata.file_type().is_socket(),
            "refusing to replace non-socket path"
        );
        match UnixStream::connect(path) {
            Ok(_) => anyhow::bail!("controller socket is already in use"),
            Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => {
                std::fs::remove_file(path)?
            }
            Err(error) => return Err(error.into()),
        }
    }
    let listener = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    let session = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos() as u64
        ^ u64::from(std::process::id());
    let deck = Deck::start(Arc::clone(&show))?;
    let mut bag = crate::control::ShuffleBag::new(session);
    let (mut sections_seen, mut shuffling) = (None, state.auto_shuffle);
    let midi = crate::midi::start();
    let mut midi_channel = crate::xtouch::CHANNEL;
    let mut midi_epoch = 0;
    let mut feedback = crate::xtouch::Feedback::default();
    let mut last_clip = None;
    deck.publish(&state, false);
    let mut peer: Option<Connection> = None;
    let mut revision = 1;
    let started = Instant::now();
    let mut report_time = Instant::now();
    let mut online = false;
    let mut last_frame = None;
    let mut ready = false;
    let mut health_time = Instant::now();
    let mut health = deck.health();
    let mut hid_progress_at = Instant::now();
    let mut image_progress_at = Instant::now();
    loop {
        // Accept at most one peer each tick. Existing renderer is never displaced by a stray client.
        match listener.accept() {
            Ok((stream, _)) if peer.is_none() => {
                let mut connection = Connection::new(stream)?;
                connection.queue(&Message::Snapshot {
                    session,
                    revision,
                    state: state.clone(),
                })?;
                peer = Some(connection);
                report_time = Instant::now();
                last_frame = None;
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(error.into()),
        }
        let mut dirty = false;
        for mut command in deck.commands.try_iter().take(64) {
            command = match command {
                Command::Blackout { .. } => Command::Blackout {
                    enabled: !state.blackout,
                },
                Command::SetBlend { mode, .. } => Command::SetBlend {
                    mode,
                    opacity: state.opacity,
                },
                other => other,
            };
            if matches!(command, Command::TapTempo { .. }) {
                command = Command::TapTempo {
                    time_ms: started.elapsed().as_millis() as u64,
                };
            }
            if let Err(error) = state.apply(command) {
                eprintln!("controller command rejected: {error}");
            } else {
                dirty = true;
            }
        }
        let mut refresh_leds = false;
        for (channel, event) in midi.events.try_iter().take(64) {
            use crate::xtouch::Event;
            midi_channel = channel;
            if matches!(event, Event::Note { .. }) {
                feedback.refresh_leds();
                refresh_leds = true;
            }
            let now = started.elapsed().as_millis() as u64;
            let event = match event {
                Event::Control { number, value } => match crate::xtouch::knob_index(number) {
                    Some(knob) => Event::Control {
                        number,
                        value: feedback.turned(knob, value, now),
                    },
                    None => event,
                },
                other => other,
            };
            for command in crate::xtouch::commands(event, &state, last_clip, now) {
                if let Err(error) = state.apply(command) {
                    eprintln!("midi command rejected: {error}");
                } else {
                    dirty = true;
                }
            }
        }
        if let Some(clip) = state.requested.or(state.playing) {
            last_clip = Some(clip);
        }
        let epoch = midi.connections();
        if epoch != midi_epoch || dirty || refresh_leds {
            if epoch != midi_epoch {
                midi_epoch = epoch;
                feedback = crate::xtouch::Feedback::default();
            }
            for (bytes, _) in feedback.update(&state, midi_channel) {
                midi.send(&bytes);
            }
        }
        let mut disconnect = false;
        let mut new_section = false;
        if let Some(connection) = &mut peer {
            match connection.poll() {
                Ok(messages) => {
                    for message in messages {
                        if let Message::Report {
                            session: source,
                            frame,
                            shader_errors,
                            sections,
                            ..
                        } = &message
                        {
                            if *source != session {
                                continue;
                            }
                            if last_frame != Some(*frame) {
                                report_time = Instant::now();
                                last_frame = Some(*frame);
                            }
                            new_section |= sections_seen.replace(*sections).is_some_and(|seen| seen != *sections);
                            if !online {
                                online = true;
                                dirty = true;
                            }
                            for error in shader_errors.iter().take(1) {
                                if dirty {
                                    eprintln!("renderer shader {}: {}", error.shader, error.reason);
                                }
                            }
                            dirty |= apply_report(&mut state, session, &message);
                        }
                    }
                }
                Err(error) => {
                    eprintln!("renderer disconnected: {error:#}");
                    disconnect = true;
                }
            }
            if report_time.elapsed() > Duration::from_secs(3) {
                disconnect = true;
            }
        }
        if disconnect {
            peer = None;
            sections_seen = None;
            if online {
                online = false;
                dirty = true;
            }
            // Keep the desired clip/effects for recovery but stop claiming presentation.
            if let Some(clip) = state.playing.take() {
                if state.requested.is_none() {
                    state
                        .apply(Command::TriggerClip { clip })
                        .map_err(anyhow::Error::msg)?;
                }
                dirty = true;
            }
        }
        // Auto shuffle deals the next shader at each new section of the music, and at once
        // when it is switched on with no shader showing. A shader the operator switched
        // off stays off.
        let switched_on = state.auto_shuffle && !shuffling;
        shuffling = state.auto_shuffle;
        if state.auto_shuffle && !state.blackout && (if state.shader { new_section } else { switched_on }) {
            if let Some(shader) = bag.next(&state) {
                if state.apply(Command::TriggerShader { shader }).is_ok() {
                    dirty = true;
                }
            }
        }
        if dirty {
            revision = revision.wrapping_add(1);
            deck.publish(&state, online);
            if let Some((tx, stale)) = &checkpoint {
                let _ = stale.try_recv();
                let _ = tx.try_send(state.clone());
            }
            if let Some(connection) = &mut peer {
                connection.queue(&Message::Snapshot {
                    session,
                    revision,
                    state: state.clone(),
                })?;
            }
        }
        if !ready && deck.ready() {
            notify::send("READY=1\nSTATUS=Controller online; renderer supervised separately");
            ready = true;
        }
        if health_time.elapsed() > Duration::from_secs(2) {
            let next = deck.health();
            if next.hid_progress != health.hid_progress {
                hid_progress_at = Instant::now();
            }
            if next.image_progress != health.image_progress {
                image_progress_at = Instant::now();
            }
            // Both workers must progress; missing hardware retries count as healthy work.
            anyhow::ensure!(
                hid_progress_at.elapsed() < Duration::from_secs(6),
                "HID worker stalled"
            );
            anyhow::ensure!(
                image_progress_at.elapsed() < Duration::from_secs(6),
                "image worker stalled"
            );
            notify::send("WATCHDOG=1");
            eprintln!(
                "controller connected={} renderer_online={online} page={} rejected={}",
                next.connected, state.page, next.rejected_commands
            );
            health = next;
            health_time = Instant::now();
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_renderer_session_cannot_acknowledge_new_controller() {
        let mut state = State::new(2);
        state.apply(Command::TriggerClip { clip: 1 }).unwrap();
        let report = Message::Report {
            session: 5,
            frame: 1,
            playing: Some(Playing {
                clip: 1,
                generation: 1,
            }),
            failed: None,
            shader_errors: Vec::new(),
            sections: 0,
        };
        assert!(!apply_report(&mut state, 6, &report));
        assert_eq!(state.playing, None);
        assert!(apply_report(&mut state, 5, &report));
        assert_eq!(state.playing, Some(1));
    }
}
