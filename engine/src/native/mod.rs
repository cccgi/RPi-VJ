pub mod bench;
mod capture;
mod control_link;
mod mpv;
mod render;
mod textures;

use crate::{
    audio::{AudioFrame, StereoAnalyzer, SAMPLE_RATE},
    control::{Command, State},
    ipc::{Failure, Message, Playing, ShaderFailure},
    notify,
    show::Show,
};
use anyhow::{Context, Result};
use sdl2::{event::Event, keyboard::Keycode, video::GLProfile};
use std::{
    path::PathBuf,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

struct Pending {
    clip: usize,
    started: Instant,
    rendered: bool,
}

pub fn run(
    show: Show,
    windowed: bool,
    allow_software: bool,
    socket: Option<PathBuf>,
) -> Result<()> {
    if show.clips.is_empty() {
        return run_shader_only(show, windowed, allow_software);
    }
    let mut link = control_link::Link::new(socket)?;
    let sdl = sdl2::init().map_err(anyhow::Error::msg)?;
    // The HDMI output is an appliance surface, never a desktop UI.
    sdl.mouse().show_cursor(false);
    let video = sdl.video().map_err(anyhow::Error::msg)?;
    video.gl_attr().set_context_profile(GLProfile::GLES);
    video.gl_attr().set_context_version(3, 0);
    video.gl_attr().set_double_buffer(true);
    let mut builder = video.window("Native VJ", 1920, 1080);
    builder.opengl();
    if !windowed {
        builder.fullscreen_desktop();
    }
    let window = builder.build()?;
    let _context = window.gl_create_context().map_err(anyhow::Error::msg)?;
    if let Err(error) = video.gl_set_swap_interval(1) {
        if allow_software {
            eprintln!("swap interval unavailable in diagnostic renderer: {error}");
            let _ = video.gl_set_swap_interval(0);
        } else {
            return Err(anyhow::Error::msg(error).context("required vsync unavailable"));
        }
    }
    sdl.mouse().show_cursor(false);
    // Drop order: mpv, renderer, current GL context, window.
    let mut renderer = render::Renderer::new(&video, &show)?;
    let shader_errors: Vec<ShaderFailure> = renderer
        .shader_errors()
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            e.as_ref().map(|e| ShaderFailure {
                shader: i,
                reason: e.chars().take(300).collect::<String>(),
            })
        })
        .take(64)
        .collect();
    let mut players = [mpv::Mpv::new()?, mpv::Mpv::new()?];
    for player in &mut players {
        player.attach()?;
    }
    let audio_system = sdl.audio().map_err(anyhow::Error::msg)?;
    let mut capture = None;
    let mut capture_retry = Instant::now() - Duration::from_secs(3);
    let mut audio = AudioFrame::default();
    let mut last_audio = Instant::now();
    let mut events = sdl.event_pump().map_err(anyhow::Error::msg)?;
    let mut state = State::from_show(&show);
    if !link.external() {
        if let Some(clip) = show.clips.iter().position(|c| c.path.is_file()) {
            state
                .apply(Command::TriggerClip { clip })
                .map_err(anyhow::Error::msg)?;
        }
    }
    let start = Instant::now();
    let mut active = 0;
    let mut pending: Option<Pending> = None;
    let mut warmed = None;
    let mut desired: Option<(usize, u64)> = None;
    let mut seen_generation = u64::MAX;
    let mut playing: Option<Playing> = None;
    let mut failed = None;
    let mut preload_attempted = None;
    let mut speed = 1.0;
    let mut report_time = Instant::now();
    let mut health_time = Instant::now();
    let mut ready = false;
    let mut frames = 0_u64;
    let mut late_frames = 0_u64;
    loop {
        let frame_start = Instant::now();
        if link.poll(&mut state, &show) {
            seen_generation = u64::MAX;
        }
        for event in events.poll_iter().take(64) {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => return Ok(()),
                Event::KeyDown {
                    keycode: Some(key),
                    repeat: false,
                    ..
                } if !link.external() => {
                    let command = match key {
                        Keycode::Left => Some(Command::Page { delta: -1 }),
                        Keycode::Right => Some(Command::Page { delta: 1 }),
                        Keycode::Space => Some(Command::Blackout {
                            enabled: !state.blackout,
                        }),
                        Keycode::S if !show.shaders.is_empty() => {
                            Some(Command::TriggerShader { shader: 0 })
                        }
                        Keycode::R => Some(Command::Reset),
                        Keycode::Num1 => state.key_in_show(&show, 0),
                        Keycode::Num2 => state.key_in_show(&show, 1),
                        Keycode::Num3 => state.key_in_show(&show, 2),
                        Keycode::Num4 => state.key_in_show(&show, 3),
                        _ => None,
                    };
                    if let Some(command) = command {
                        state.apply(command).map_err(anyhow::Error::msg)?;
                    }
                }
                _ => {}
            }
        }
        if seen_generation != state.generation {
            seen_generation = state.generation;
            failed = None;
            desired = state.requested.map(|clip| (clip, state.generation));
            if desired.is_none() {
                for player in &players {
                    player.command(&["stop"])?;
                }
                pending = None;
                warmed = None;
                playing = None;
                preload_attempted = None;
            }
        }
        capture::set_gain(state.input_gain);
        if state.speed != speed {
            speed = state.speed;
            let value = speed.to_string();
            for player in &players {
                player.command(&["set", "speed", &value])?;
            }
        }
        // Re-open a missing or stopped capture stream periodically; video never waits for audio.
        if capture.is_none() && capture_retry.elapsed() > Duration::from_secs(2) {
            capture_retry = Instant::now();
            if let Some(name) = show.audio_device.as_deref() {
                match capture::start(&audio_system, name) {
                    Ok(device) => {
                        capture = Some(device);
                        last_audio = Instant::now();
                    }
                    Err(error) => eprintln!("audio unavailable: {error:#}"),
                }
            }
        }
        if let Some((device, receiver, _)) = &capture {
            if let Ok(frame) = receiver.try_recv() {
                audio = frame;
                last_audio = Instant::now();
            }
            if device.status() == sdl2::audio::AudioStatus::Stopped
                || last_audio.elapsed() > Duration::from_secs(2)
            {
                capture = None;
            }
        }
        if last_audio.elapsed() > Duration::from_millis(250) {
            audio = AudioFrame::default();
        }
        let mut candidate_error = None;
        for (index, player) in players.iter_mut().enumerate() {
            if let Err(error) = player.poll() {
                if index == active && playing.is_some() {
                    return Err(error.context("active decoder failed"));
                }
                candidate_error = Some(format!("{error:#}"));
            }
        }
        if let Some(reason) = candidate_error {
            if let Some(load) = pending.take() {
                if let Some((clip, generation)) = desired {
                    if clip == load.clip {
                        failed = Some(Failure { generation, reason });
                        desired = None;
                    }
                }
            }
            warmed = None;
        }
        // Foreground request first. Background work warms only one next clip, never a whole page.
        if pending.is_none() {
            let next = if let Some((clip, _)) = desired {
                (warmed != Some(clip)).then_some(clip)
            } else if let Some(current) = &playing {
                if preload_attempted != Some(current.generation) {
                    preload_attempted = Some(current.generation);
                    (1..show.clips.len())
                        .map(|n| (current.clip + n) % show.clips.len())
                        .find(|i| show.clips[*i].path.is_file())
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(clip) = next {
                warmed = None;
                let result = show
                    .clips
                    .get(clip)
                    .context("unknown clip")
                    .and_then(|clip| clip.path.to_str().context("UTF-8 path required"))
                    .and_then(|path| players[1 - active].load(path));
                match result {
                    Ok(()) => {
                        pending = Some(Pending {
                            clip,
                            started: Instant::now(),
                            rendered: false,
                        })
                    }
                    Err(error) => {
                        eprintln!("candidate load failed: {error:#}");
                        if let Some((wanted, generation)) = desired {
                            if wanted == clip {
                                failed = Some(Failure {
                                    generation,
                                    reason: format!("{error:#}"),
                                });
                                desired = None;
                            }
                        }
                    }
                }
            }
        }
        for (index, player) in players.iter().enumerate() {
            let force = index != active
                && player.restarted
                && pending.as_ref().is_some_and(|p| !p.rendered);
            match player.render(renderer.targets[index].fbo.0.get(), 1920, 1080, force) {
                Ok(updated) => {
                    if index != active {
                        if let Some(load) = &mut pending {
                            load.rendered |= updated && player.file_loaded;
                        }
                    }
                }
                Err(error) if index == active => return Err(error.context("active render failed")),
                Err(error) => {
                    if let Some(load) = pending.take() {
                        if let Some((clip, generation)) = desired {
                            if clip == load.clip {
                                failed = Some(Failure {
                                    generation,
                                    reason: format!("{error:#}"),
                                });
                                desired = None;
                            }
                        }
                    }
                    warmed = None;
                }
            }
        }
        if let Some(load) = &pending {
            if load.rendered
                && players[1 - active].restarted
                && (allow_software || players[1 - active].hardware_active())
            {
                warmed = Some(load.clip);
                pending = None;
            } else if load.started.elapsed() > Duration::from_secs(5) {
                if let Some((clip, generation)) = desired {
                    if clip == load.clip {
                        failed = Some(Failure {
                            generation,
                            reason: "decode/hardware gate timeout".into(),
                        });
                        desired = None;
                    }
                }
                if let Err(error) = players[1 - active].command(&["stop"]) {
                    eprintln!("candidate stop: {error:#}");
                }
                pending = None;
                warmed = None;
            }
        }
        let mut promoted = None;
        if let Some((clip, generation)) = desired {
            if warmed == Some(clip) {
                players[1 - active].command(&["set", "pause", "no"])?;
                players[active].command(&["set", "pause", "yes"])?;
                active = 1 - active;
                promoted = Some(Playing { clip, generation });
                desired = None;
                warmed = None;
            }
        }
        // Clear/reset hides stale retained FBO pixels; feedback only advances after swap returns.
        let mut drawing = state.clone();
        drawing.playing = promoted.as_ref().or(playing.as_ref()).map(|p| p.clip);
        renderer.draw(
            active,
            &drawing,
            &audio,
            start.elapsed().as_secs_f32(),
            window.drawable_size(),
        );
        window.gl_swap_window();
        frames += 1;
        if let Some(presented) = promoted {
            state.presented(presented.clip, presented.generation);
            playing = Some(presented);
        }
        if !ready {
            notify::send(
                "READY=1\nSTATUS=Native renderer presenting; controller supervised independently",
            );
            ready = true;
        }
        if report_time.elapsed() > Duration::from_millis(100) {
            link.report(&Message::Report {
                session: link.session,
                frame: frames,
                playing: playing.clone(),
                failed: failed.clone(),
                shader_errors: shader_errors.clone(),
            });
            report_time = Instant::now();
        }
        if frame_start.elapsed() > Duration::from_millis(20) {
            late_frames += 1;
        }
        if health_time.elapsed() > Duration::from_secs(1) {
            notify::send("WATCHDOG=1");
            let dropped = capture
                .as_ref()
                .map_or(0, |(_, _, n)| n.load(Ordering::Relaxed));
            eprintln!("renderer frames={frames} late={late_frames} audio_dropped={dropped} warmed={warmed:?}");
            health_time = Instant::now();
        }
    }
}

/// Diagnostic path used for shader qualification on hosts without libmpv/media.
fn run_shader_only(show: Show, windowed: bool, allow_software: bool) -> Result<()> {
    let sdl = sdl2::init().map_err(anyhow::Error::msg)?;
    // The benchmark/gallery uses the same appliance presentation rules.
    sdl.mouse().show_cursor(false);
    let video = sdl.video().map_err(anyhow::Error::msg)?;
    video.gl_attr().set_context_profile(GLProfile::GLES);
    video.gl_attr().set_context_version(3, 0);
    video.gl_attr().set_double_buffer(true);
    let display_index: i32 = std::env::var("RPI_VJ_DISPLAY_INDEX").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let mode = video.desktop_display_mode(display_index).map_err(anyhow::Error::msg)?;
    let mut builder = video.window("Native VJ Shader Gallery", mode.w.max(1) as u32, mode.h.max(1) as u32);
    builder.opengl();
    if !windowed {
        if let Ok(bounds) = video.display_bounds(display_index) { builder.position(bounds.x, bounds.y); }
        builder.fullscreen_desktop();
    }
    let mut window = builder.build()?;
    let _context = window.gl_create_context().map_err(anyhow::Error::msg)?;
    if let Err(error) = video.gl_set_swap_interval(1) {
        if allow_software { let _ = video.gl_set_swap_interval(0); }
        else { return Err(anyhow::Error::msg(error).context("required vsync unavailable")); }
    }
    let mut renderer = render::Renderer::new(&video, &show)?;
    for (index, error) in renderer.shader_errors().iter().enumerate() {
        if let Some(error) = error { eprintln!("Shader disabled [{index}]: {error}"); }
    }
    let mut events = sdl.event_pump().map_err(anyhow::Error::msg)?;
    let wav_samples = std::env::var_os("RPI_VJ_AUDIO_WAV")
        .and_then(|path| load_wav_stereo(&path).ok())
        .unwrap_or_default();
    let _audio_device = if wav_samples.is_empty() {
        None
    } else {
        let audio = sdl.audio().map_err(anyhow::Error::msg)?;
        let desired = sdl2::audio::AudioSpecDesired {
            freq: Some(SAMPLE_RATE as i32), channels: Some(2), samples: Some(1024),
        };
        let device = audio.open_queue::<f32, _>(None, &desired).map_err(anyhow::Error::msg)?;
        device.queue_audio(&wav_samples).map_err(anyhow::Error::msg)?;
        device.resume();
        eprintln!("SDL audio playback: {} samples queued, device='{}'", device.size(), device.spec().freq);
        Some(device)
    };
    let mut analyzer = StereoAnalyzer::default();
    let mut state = State::from_show(&show);
    state.shader = !show.shaders.is_empty();
    state.selected_shader = (!show.shaders.is_empty()).then_some(0);
    let mut audio = AudioFrame::default();
    let start = Instant::now();
    let mut selected = 0usize;
    let mut last_shader = Instant::now();
    let mut frames = 0u64;
    let mut late_frames = 0u64;
    let mut health_time = Instant::now();
    if !show.shaders.is_empty() { let _ = window.set_title(&format!("Shader Gallery [{}/{}] {} | Left/Right: select, Esc: quit", 1, show.shaders.len(), show.shaders[0].label)); }
    loop {
        let frame_start = Instant::now();
        for event in events.poll_iter() {
            match event {
                Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => return Ok(()),
                Event::KeyDown { keycode: Some(Keycode::Right), repeat: false, .. } if !show.shaders.is_empty() => {
                    selected = (selected + 1) % show.shaders.len();
                    state.selected_shader = Some(selected);
                    last_shader = Instant::now();
                    let _ = window.set_title(&format!("Shader Gallery [{}/{}] {} | Left/Right: select, Esc: quit", selected + 1, show.shaders.len(), show.shaders[selected].label));
                }
                Event::KeyDown { keycode: Some(Keycode::Left), repeat: false, .. } if !show.shaders.is_empty() => {
                    selected = (selected + show.shaders.len() - 1) % show.shaders.len();
                    state.selected_shader = Some(selected);
                    last_shader = Instant::now();
                    let _ = window.set_title(&format!("Shader Gallery [{}/{}] {} | Left/Right: select, Esc: quit", selected + 1, show.shaders.len(), show.shaders[selected].label));
                }
                _ => {}
            }
        }
        if last_shader.elapsed() >= Duration::from_secs(10) && !show.shaders.is_empty() {
            selected = (selected + 1) % show.shaders.len();
            state.selected_shader = Some(selected);
            state.shader = true;
            last_shader = Instant::now();
        }
        if !wav_samples.is_empty() {
            let base = ((start.elapsed().as_secs_f32() * SAMPLE_RATE) as usize * 2) % wav_samples.len();
            let block = std::array::from_fn(|i| wav_samples[(base + i) % wav_samples.len()]);
            audio = analyzer.process(&block);
        }
        renderer.draw(0, &state, &audio, start.elapsed().as_secs_f32(), window.drawable_size());
        window.gl_swap_window();
        frames += 1;
        // Same rule as the show loop: a frame that misses the 60 Hz slot with margin is late.
        if frame_start.elapsed() > Duration::from_millis(20) {
            late_frames += 1;
        }
        if health_time.elapsed() >= Duration::from_secs(1) {
            // Keep the diagnostic path observable too; cost qualification itself is `--bench`.
            eprintln!("renderer frames={frames} late={late_frames} audio_dropped=0 warmed=None");
            health_time = Instant::now();
        }
    }
}

fn load_wav_stereo(path: &std::ffi::OsStr) -> Result<Vec<f32>> {
    let bytes = std::fs::read(path)?;
    anyhow::ensure!(bytes.len() > 44 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE", "unsupported WAV");
    let channels = u16::from_le_bytes([bytes[22], bytes[23]]) as usize;
    let bits = u16::from_le_bytes([bytes[34], bytes[35]]);
    anyhow::ensure!(channels > 0 && bits == 16, "WAV must be 16-bit PCM");
    let mut data = 36;
    while data + 8 <= bytes.len() && &bytes[data..data + 4] != b"data" {
        let size = u32::from_le_bytes(bytes[data + 4..data + 8].try_into()?) as usize;
        data += 8 + size;
    }
    anyhow::ensure!(data + 8 <= bytes.len(), "WAV data chunk missing");
    let start = data + 8;
    let count = u32::from_le_bytes(bytes[data + 4..data + 8].try_into()?) as usize / (2 * channels);
    let mut out = Vec::with_capacity(count * 2);
    for frame in 0..count {
        let p = start + frame * channels * 2;
        let left = i16::from_le_bytes([bytes[p], bytes[p + 1]]) as f32 / 32768.0;
        let right = if channels > 1 { i16::from_le_bytes([bytes[p + 2], bytes[p + 3]]) as f32 / 32768.0 } else { left };
        out.extend([left, right]);
    }
    Ok(out)
}





