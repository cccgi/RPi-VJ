//! `--bench`: one KMS/GLES session that measures every manifest shader offscreen.
//! Contracts and statistics live in `crate::bench`; this file only drives the GPU.
use super::{
    render::{self, Program, ShaderFrame, Target},
    textures,
};
use crate::{
    audio::Tempo,
    bench::{
        begin_line, completed_keys, downsample, frame_completed, frame_time, pixel_stats, ppm,
        probe_estimate_ms, result_line, tier_stats, AudioPattern, Entry, Manifest, Outcome,
        Pixels, Tier, BPM, PIXEL_TIMES, PROBE_FACTOR, RESET_SUSPECT_MS, SENTINEL, THUMB,
        TIME_SPAN, TOO_SLOW_FIRST_MS,
    },
    shader,
};
use AudioPattern::{Beat, Max};
use anyhow::{Context, Result};
use glow::HasContext;
use sdl2::{event::Event, keyboard::Keycode, video::GLProfile};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// Draw-time shader variants are compiled against this small target so that
/// `compile_ms` holds the whole compile and `first_ms` is render cost.
const WARM_SIZE: (u32, u32) = (64, 36);
const CALIBRATION: &str = "void main(){gl_FragColor=vec4(0.25,0.5,0.75,1.0);}";

/// How a frame is known to have left the GPU. `Finish` is the qualified default; the
/// others exist to cross-check it (`RPI_VJ_BENCH_SYNC=readback|fence`).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Sync {
    Finish,
    Readback,
    Fence,
}

struct Session<'a> {
    gl: glow::Context,
    window: &'a sdl2::video::Window,
    vao: glow::VertexArray,
    tiers: Vec<((u32, u32), Target)>,
    warm: Target,
    /// Input textures of every manifest entry, uploaded before the first measurement.
    bank: textures::Bank,
    /// Synthetic stand-in for the deck video, for shaders that declare `u_video`.
    video: glow::Texture,
    sync: Sync,
    frame: i32,
    events: sdl2::EventPump,
    stop: bool,
    polled: Instant,
}
impl Session<'_> {
    /// SIGTERM/SIGINT arrive as SDL quit events. Polled between frames so the process
    /// leaves at a frame boundary, never with a render job in flight.
    fn stopping(&mut self) -> bool {
        if !self.stop && self.polled.elapsed() >= Duration::from_millis(100) {
            self.polled = Instant::now();
            self.stop = quit_requested(&mut self.events);
        }
        self.stop
    }
    unsafe fn wait(&self) {
        let gl = &self.gl;
        match self.sync {
            Sync::Finish => gl.finish(),
            Sync::Readback => {
                let mut pixel = [0u8; 4];
                gl.read_pixels(
                    0,
                    0,
                    1,
                    1,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelPackData::Slice(Some(&mut pixel)),
                );
            }
            Sync::Fence => match gl.fence_sync(glow::SYNC_GPU_COMMANDS_COMPLETE, 0) {
                Ok(fence) => {
                    while gl.client_wait_sync(fence, glow::SYNC_FLUSH_COMMANDS_BIT, 1_000_000_000)
                        == glow::TIMEOUT_EXPIRED
                    {}
                    gl.delete_sync(fence);
                }
                Err(_) => gl.finish(),
            },
        }
    }
    /// Tier target by index; any other index is the small warm-up target.
    fn target(&self, target: usize) -> ((u32, u32), glow::Framebuffer) {
        match self.tiers.get(target) {
            Some((size, target)) => (*size, target.fbo),
            None => (WARM_SIZE, self.warm.fbo),
        }
    }
    /// The V3D kernel driver resets the GPU when a render job stops making tile progress
    /// (about one second) and `glFinish` then returns as if the frame were done. Render
    /// `time` again over a sentinel fill and report whether the frame really completed.
    unsafe fn completes(
        &mut self,
        program: &Program,
        target: usize,
        time: f32,
        pattern: AudioPattern,
    ) -> bool {
        let ((width, height), fbo) = self.target(target);
        let [r, g, b, a] = SENTINEL.map(|v| f32::from(v) / 255.0);
        self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
        self.gl.viewport(0, 0, width as i32, height as i32);
        self.gl.clear_color(r, g, b, a);
        self.gl.clear(glow::COLOR_BUFFER_BIT);
        self.gl.finish();
        self.frame(program, target, time, pattern);
        let mut buffer = vec![0u8; width as usize * height as usize * 4];
        self.gl.read_pixels(
            0,
            0,
            width as i32,
            height as i32,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelPackData::Slice(Some(&mut buffer)),
        );
        frame_completed(&buffer)
    }
    /// One shader frame into `target`, bounded by the sync method. Returns milliseconds.
    unsafe fn frame(
        &mut self,
        program: &Program,
        target: usize,
        time: f32,
        pattern: AudioPattern,
    ) -> f64 {
        let audio = pattern.audio(time);
        let (flow, spec_flow) = pattern.flow(time);
        let (size, fbo) = self.target(target);
        let start = Instant::now();
        render::shader_pass(
            &self.gl,
            program,
            fbo,
            size,
            &ShaderFrame {
                time,
                dt: 1.0 / 60.0,
                frame: self.frame,
                bpm: BPM,
                // Both patterns keep time with the clock, so the lock is honest and the
                // phase-locked branches of a shader are part of what is measured.
                tempo: Tempo::free(time, BPM, 1.0),
                audio: &audio,
                flow: &flow,
                spec_flow: &spec_flow,
                video: Some(self.video),
                video_quad_mirror: false,
            },
        );
        self.wait();
        self.frame += 1;
        start.elapsed().as_secs_f64() * 1000.0
    }
    /// Show the tier's last frame on the output. Never inside a timed section.
    unsafe fn present(&self, tier: usize) {
        let gl = &self.gl;
        let ((width, height), target) = &self.tiers[tier];
        let (out_w, out_h) = self.window.drawable_size();
        gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(target.fbo));
        gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, None);
        gl.blit_framebuffer(
            0,
            0,
            *width as i32,
            *height as i32,
            0,
            0,
            out_w as i32,
            out_h as i32,
            glow::COLOR_BUFFER_BIT,
            glow::LINEAR,
        );
        self.window.gl_swap_window();
        gl.finish();
    }
    unsafe fn gl_errors(&self) -> Option<String> {
        let mut codes = Vec::new();
        for _ in 0..16 {
            match self.gl.get_error() {
                glow::NO_ERROR => break,
                code => codes.push(format!("0x{code:04x}")),
            }
        }
        (!codes.is_empty()).then(|| format!("glGetError {}", codes.join(" ")))
    }
    /// First frame, then frames up to the pattern's count or time budget. `None` when
    /// stopped.
    unsafe fn tier(
        &mut self,
        program: &Program,
        index: usize,
        pattern: AudioPattern,
    ) -> Option<Tier> {
        let first_ms = self.frame(program, index, frame_time(0), pattern);
        self.present(index);
        if index == 0 && first_ms > TOO_SLOW_FIRST_MS {
            return Some(Tier::Aborted {
                reason: "too_slow",
                first_ms,
            });
        }
        let reset = Tier::Aborted {
            reason: "gpu_reset",
            first_ms,
        };
        if first_ms >= RESET_SUSPECT_MS
            && !self.completes(program, index, frame_time(0), pattern)
        {
            return Some(reset);
        }
        let mut frames = Vec::new();
        let mut spent = 0.0;
        let mut slowest = (0.0, 0.0);
        while pattern.keep_sampling(frames.len(), spent) {
            if self.stopping() {
                return None;
            }
            let time = frame_time(frames.len() + 1);
            let ms = self.frame(program, index, time, pattern);
            if ms > slowest.0 {
                slowest = (ms, time);
            }
            spent += ms;
            frames.push(ms);
            if frames.len() % 10 == 0 {
                self.present(index);
            }
        }
        // Cost can depend on time: a frame later in the span may be the one that was killed.
        if first_ms < RESET_SUSPECT_MS
            && slowest.0 >= RESET_SUSPECT_MS
            && !self.completes(program, index, slowest.1, pattern)
        {
            return Some(reset);
        }
        Some(Tier::Measured(tier_stats(first_ms, &frames)))
    }
    /// Three readbacks of the first tier; the middle one becomes the thumbnail.
    unsafe fn pixels(
        &mut self,
        program: &Program,
        thumb: &Path,
        pattern: AudioPattern,
    ) -> (Pixels, bool) {
        let (width, height) = self.tiers[0].0;
        let mut buffer = vec![0u8; width as usize * height as usize * 4];
        let mut samples = Vec::new();
        for fraction in PIXEL_TIMES {
            self.frame(program, 0, TIME_SPAN * fraction, pattern);
            self.gl.read_pixels(
                0,
                0,
                width as i32,
                height as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut buffer)),
            );
            samples.push(downsample(&buffer, width as usize, height as usize));
            if samples.len() == 2 {
                self.present(0);
            }
        }
        let written = std::fs::write(thumb, ppm(&samples[1], THUMB.0, THUMB.1));
        if let Err(error) = &written {
            eprintln!("bench: thumbnail {}: {error}", thumb.display());
        }
        (pixel_stats(&samples), written.is_ok())
    }
    /// `None` when a stop request abandoned the entry; it then has no result line.
    unsafe fn measure(
        &mut self,
        manifest: &Manifest,
        entry: &Entry,
        label: &str,
    ) -> Option<Outcome> {
        self.frame = 0;
        // `shader_pass` gives every sampler a shader reads a defined texture (its binding
        // or none), so results do not depend on what the previous entry left bound.
        let kinds = entry.channels.kinds();
        // An invalid channel block, or a missing, undecodable or over-budget texture,
        // fails the entry; it is never measured with a wrong or absent input.
        let resolved = entry
            .channels
            .checked()
            .and_then(|list| self.bank.resolve(list));
        let channels = match resolved {
            Ok(channels) => channels,
            Err(error) => {
                return Some(Outcome::failed(
                    "prepare_error",
                    &format!("{error:#}"),
                    manifest,
                ))
            }
        };
        let prepared = match shader::load_with(&entry.path, &kinds) {
            Ok(prepared) => prepared,
            Err(error) => {
                return Some(Outcome::failed(
                    "prepare_error",
                    &format!("{error:#}"),
                    manifest,
                ))
            }
        };
        // Phase lines let the supervisor say where a hang happened.
        eprintln!("bench: {label} compile");
        let start = Instant::now();
        let mut program = match Program::new(&self.gl, &prepared.source) {
            Ok(program) => program,
            Err(error) => {
                self.gl_errors();
                return Some(Outcome::failed(
                    "compile_error",
                    &format!("{error:#}"),
                    manifest,
                ));
            }
        };
        // Reported after the timed sections, so the message costs no measured time.
        let unbound = match program.bind_channels(&self.gl, channels) {
            Ok(unbound) => Program::unbound_warning(&unbound),
            Err(error) => {
                self.gl.delete_program(program.id);
                return Some(Outcome::failed(
                    "compile_error",
                    &format!("{error:#}"),
                    manifest,
                ));
            }
        };
        self.gl.use_program(Some(program.id));
        for (name, value) in &prepared.defaults {
            program.float(&self.gl, name, *value);
        }
        let link_ms = start.elapsed().as_secs_f64() * 1000.0;
        let warm_ms = self.frame(&program, usize::MAX, frame_time(0), Beat);
        let compile_ms = start.elapsed().as_secs_f64() * 1000.0;
        if warm_ms >= RESET_SUSPECT_MS
            && !self.completes(&program, usize::MAX, frame_time(0), Beat)
        {
            // Not even a 64x36 frame finishes; the watchdog wait is not compile time.
            let mut outcome = Outcome::failed(
                "too_slow",
                "GPU hang watchdog reset the render job on the 64x36 warm-up frame",
                manifest,
            );
            for (_, tier) in &mut outcome.tiers {
                *tier = Tier::Skipped("gpu_reset".into());
            }
            outcome.compile_ms = Some(link_ms);
            outcome.channels = kinds;
            self.gl_errors();
            self.gl.use_program(None);
            self.gl.delete_program(program.id);
            eprintln!("bench: {label} too_slow (GPU reset in warm-up) link={link_ms:.0}ms");
            return Some(outcome);
        }
        // Second small frame, now free of compile work: its cost scaled to the first tier
        // tells whether the full-size job is worth submitting.
        let probe_ms = self.frame(&program, usize::MAX, frame_time(0), Beat);
        let estimate_ms = probe_estimate_ms(probe_ms, WARM_SIZE, self.tiers[0].0);
        if estimate_ms > PROBE_FACTOR * TOO_SLOW_FIRST_MS {
            let height = self.tiers[0].0 .1;
            let mut outcome = Outcome::failed(
                "too_slow",
                &format!(
                    "64x36 probe frame took {probe_ms:.1} ms: {height}p estimate {estimate_ms:.0} ms (limit {TOO_SLOW_FIRST_MS:.0} ms)"
                ),
                manifest,
            );
            outcome.compile_ms = Some(compile_ms);
            outcome.channels = kinds;
            self.gl_errors();
            self.gl.use_program(None);
            self.gl.delete_program(program.id);
            eprintln!("bench: {label} too_slow (probe {probe_ms:.1}ms, estimate {estimate_ms:.0}ms) compile={compile_ms:.0}ms");
            return Some(outcome);
        }
        let mut outcome = Outcome {
            status: "ok",
            compile_ms: Some(compile_ms),
            tiers: Vec::new(),
            pixels: None,
            thumb: None,
            error: self.gl_errors(),
            temp_c: None,
            throttled: None,
            channels: kinds,
            tiers_max: Vec::new(),
            pixels_max: None,
            thumb_max: None,
        };
        if outcome.error.is_some() {
            outcome.status = "gl_error";
        }
        let mut stopped = false;
        for index in 0..self.tiers.len() {
            let height = self.tiers[index].0 .1;
            // Each tier under the 128 BPM pattern, then under worst-case audio. Once
            // something failed, what was not measured is recorded as skipped: a beat
            // tier always, a worst-case tier only if one was measured before it.
            for pattern in [Beat, Max] {
                if outcome.status != "ok" {
                    let skipped = (height, Tier::Skipped(outcome.status.to_owned()));
                    match pattern {
                        Beat => outcome.tiers.push(skipped),
                        Max if !outcome.tiers_max.is_empty() => outcome.tiers_max.push(skipped),
                        Max => {}
                    }
                    continue;
                }
                eprintln!("bench: {label} tier {height} {}", pattern.name());
                let Some(tier) = self.tier(&program, index, pattern) else {
                    stopped = true;
                    break;
                };
                if let Tier::Aborted { reason, first_ms } = &tier {
                    let audio = match pattern {
                        Beat => "",
                        Max => " under worst-case audio",
                    };
                    outcome.status = "too_slow";
                    outcome.error = Some(if *reason == "gpu_reset" {
                        format!(
                            "GPU hang watchdog reset the render job at {height}p{audio}; frames never completed"
                        )
                    } else {
                        format!(
                            "first {height}p frame took {first_ms:.0} ms{audio} (limit {TOO_SLOW_FIRST_MS:.0} ms)"
                        )
                    });
                }
                match pattern {
                    Beat => outcome.tiers.push((height, tier)),
                    Max => outcome.tiers_max.push((height, tier)),
                }
                if let Some(error) = self.gl_errors() {
                    outcome.status = "gl_error";
                    outcome.error = Some(error);
                }
            }
            if stopped {
                break;
            }
        }
        if stopped || self.stopping() {
            self.gl.use_program(None);
            self.gl.delete_program(program.id);
            return None;
        }
        if outcome.status == "ok" {
            eprintln!("bench: {label} pixels");
            let name = entry.thumb_name();
            let (pixels, written) =
                self.pixels(&program, &manifest.thumb_dir.join(&name), Beat);
            outcome.pixels = Some(pixels);
            outcome.thumb = written.then_some(name);
            // The same three moments under worst-case audio: how the picture looks loud.
            let name = entry.thumb_name_for(Max);
            let (pixels, written) = self.pixels(&program, &manifest.thumb_dir.join(&name), Max);
            outcome.pixels_max = Some(pixels);
            outcome.thumb_max = written.then_some(name);
            if let Some(error) = self.gl_errors() {
                outcome.status = "gl_error";
                outcome.error = Some(error);
            }
        }
        self.gl.use_program(None);
        self.gl.delete_program(program.id);
        if let Some(warning) = unbound {
            eprintln!("bench: {label} {warning}");
        }
        let describe = |suffix: &str, tiers: &[(u32, Tier)]| -> Vec<String> {
            tiers
                .iter()
                .map(|(height, tier)| match tier {
                    Tier::Measured(s) => {
                        format!(
                            "{height}p{suffix} p50={:.2} p95={:.2} n={}",
                            s.p50_ms, s.p95_ms, s.frames
                        )
                    }
                    Tier::Aborted { reason, first_ms } => {
                        format!("{height}p{suffix} {reason} first={first_ms:.0}")
                    }
                    Tier::Skipped(reason) => format!("{height}p{suffix} skipped:{reason}"),
                })
                .collect()
        };
        let mut tiers = describe("", &outcome.tiers);
        tiers.extend(describe("/max", &outcome.tiers_max));
        eprintln!(
            "bench: {label} {} compile={compile_ms:.0}ms (link {link_ms:.0}ms) probe={probe_ms:.2}ms~{estimate_ms:.0}ms {}",
            outcome.status,
            tiers.join(" ")
        );
        Some(outcome)
    }
}

fn temperature() -> Option<f64> {
    let text = std::fs::read_to_string("/sys/class/thermal/thermal_zone0/temp").ok()?;
    Some(text.trim().parse::<f64>().ok()? / 1000.0)
}
fn throttled() -> Option<String> {
    let output = std::process::Command::new("vcgencmd")
        .arg("get_throttled")
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    Some(text.trim().strip_prefix("throttled=")?.to_owned())
}
/// Kernel-accounted V3D render-queue time in nanoseconds, when the driver exposes it.
fn gpu_render_ns() -> Option<u64> {
    for entry in std::fs::read_dir("/sys/devices/platform/axi")
        .ok()?
        .flatten()
    {
        if entry.file_name().to_string_lossy().ends_with(".v3d") {
            let stats = std::fs::read_to_string(entry.path().join("gpu_stats")).ok()?;
            let line = stats.lines().find(|line| line.starts_with("render"))?;
            return line.split_whitespace().nth(3)?.parse().ok();
        }
    }
    None
}
/// Durable append: a line that was written survives the process being killed.
fn append(out: &mut File, line: &str) -> Result<()> {
    out.write_all(format!("{line}\n").as_bytes())?;
    out.sync_data()?;
    Ok(())
}
fn pause_path(out: &Path) -> PathBuf {
    let mut name = out.as_os_str().to_owned();
    name.push(".pause");
    PathBuf::from(name)
}

pub fn run(path: &Path, resume: bool, windowed: bool) -> Result<()> {
    let manifest = Manifest::load(path)?;
    if let Some(parent) = manifest.out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::create_dir_all(&manifest.thumb_dir)?;
    let mut out = OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .open(&manifest.out)
        .with_context(|| format!("open results {}", manifest.out.display()))?;
    let mut existing = String::new();
    let mut bytes = Vec::new();
    out.read_to_end(&mut bytes)?;
    existing.push_str(&String::from_utf8_lossy(&bytes));
    // A line torn by a crash must not swallow the next one.
    if !existing.is_empty() && !existing.ends_with('\n') {
        out.seek(SeekFrom::End(0))?;
        out.write_all(b"\n")?;
    }
    let done = if resume {
        completed_keys(&existing)
    } else {
        Default::default()
    };
    let todo: Vec<&Entry> = manifest
        .entries
        .iter()
        .filter(|entry| !done.contains(&entry.key()))
        .collect();
    eprintln!(
        "bench: {} entries, {} to measure, tiers {:?}",
        manifest.entries.len(),
        todo.len(),
        manifest.tiers
    );
    if todo.is_empty() {
        return Ok(());
    }
    // `compile_ms` must be the cold compile, not a hit in Mesa's on-disk shader cache.
    if std::env::var_os("MESA_SHADER_CACHE_DISABLE").is_none() {
        std::env::set_var("MESA_SHADER_CACHE_DISABLE", "true");
    }
    let sync = match std::env::var("RPI_VJ_BENCH_SYNC").as_deref() {
        Ok("readback") => Sync::Readback,
        Ok("fence") => Sync::Fence,
        _ => Sync::Finish,
    };

    let sdl = sdl2::init().map_err(anyhow::Error::msg)?;
    sdl.mouse().show_cursor(false);
    let video = sdl.video().map_err(anyhow::Error::msg)?;
    video.gl_attr().set_context_profile(GLProfile::GLES);
    video.gl_attr().set_context_version(3, 0);
    video.gl_attr().set_double_buffer(true);
    let mode = video.desktop_display_mode(0).map_err(anyhow::Error::msg)?;
    let mut builder = video.window(
        "Native VJ Shader Bench",
        mode.w.max(1) as u32,
        mode.h.max(1) as u32,
    );
    builder.opengl();
    if !windowed {
        builder.fullscreen_desktop();
    }
    let window = builder.build()?;
    let _context = window.gl_create_context().map_err(anyhow::Error::msg)?;
    // Frames are bounded by the sync call, not by vblank.
    let _ = video.gl_set_swap_interval(0);
    let events = sdl.event_pump().map_err(anyhow::Error::msg)?;
    // SAFETY: the GLES context above stays current on this thread for the whole run and
    // every GL object is created and deleted here.
    unsafe {
        let gl = glow::Context::from_loader_function(|name| video.gl_get_proc_address(name).cast());
        eprintln!(
            "bench: GPU {} / {} sync={sync:?}",
            gl.get_parameter_string(glow::RENDERER),
            gl.get_parameter_string(glow::VERSION)
        );
        let vao = gl.create_vertex_array().map_err(anyhow::Error::msg)?;
        let mut tiers = Vec::new();
        for size in &manifest.tiers {
            tiers.push((*size, render::target(&gl, *size)?));
        }
        let warm = render::target(&gl, WARM_SIZE)?;
        // Only the entries still to measure decide what is uploaded.
        let wanted: Vec<&crate::texture::Channel> =
            todo.iter().flat_map(|entry| &entry.channels.list).collect();
        let bank = textures::Bank::load(&gl, &wanted);
        let video = textures::test_video(&gl)?;
        render::fullscreen_state(&gl, vao);
        let mut session = Session {
            gl,
            window: &window,
            vao,
            tiers,
            warm,
            bank,
            video,
            sync,
            frame: 0,
            events,
            stop: false,
            polled: Instant::now(),
        };
        // A trivial shader proves the pipeline and waits out any GPU job a killed
        // predecessor left running, before an entry can be blamed for the delay.
        let trivial = Program::new(&session.gl, &shader::prepare(CALIBRATION)?.source)?;
        session.frame(&trivial, usize::MAX, 0.0, Beat);
        for index in 0..session.tiers.len() {
            let before = gpu_render_ns();
            let frames: Vec<f64> = (0..31)
                .map(|i| session.frame(&trivial, index, i as f32, Beat))
                .collect();
            let stats = tier_stats(frames[0], &frames[1..]);
            let gpu = before
                .zip(gpu_render_ns())
                .and_then(|(a, b)| b.checked_sub(a))
                .map_or("n/a".into(), |ns| format!("{:.3}", ns as f64 / 31e6));
            eprintln!(
                "bench: calibration {}p p50={:.3}ms p95={:.3}ms kernel_render={gpu}ms",
                session.tiers[index].0 .1, stats.p50_ms, stats.p95_ms
            );
        }
        session.gl.delete_program(trivial.id);
        anyhow::ensure!(session.gl_errors().is_none(), "GL error during calibration");
        eprintln!("bench: ready");

        let pause = pause_path(&manifest.out);
        let total = manifest.entries.len();
        for (number, entry) in todo.iter().enumerate() {
            // The supervisor holds this file while the SoC cools down.
            let mut announced = false;
            while pause.exists() && !session.stopping() {
                if !announced {
                    eprintln!("bench: paused by {}", pause.display());
                    announced = true;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
            if session.stopping() {
                break;
            }
            let label = format!(
                "[{}/{total}] {} {}",
                total - todo.len() + number + 1,
                entry.id,
                entry.variant
            );
            append(&mut out, &begin_line(entry))?;
            let before = gpu_render_ns();
            let started = Instant::now();
            let Some(mut outcome) = session.measure(&manifest, entry, &label) else {
                break;
            };
            outcome.temp_c = temperature();
            outcome.throttled = throttled();
            append(&mut out, &result_line(entry, &outcome))?;
            // The kernel counter is an independent check on the timed frames; it can step
            // backwards across a GPU reset.
            let kernel = before
                .zip(gpu_render_ns())
                .and_then(|(a, b)| b.checked_sub(a))
                .map_or("n/a".into(), |ns| format!("{:.2}s", ns as f64 / 1e9));
            eprintln!(
                "bench: {label} wall={:.2}s kernel_render={kernel} frames={}",
                started.elapsed().as_secs_f64(),
                session.frame
            );
        }
        for target in session
            .tiers
            .iter()
            .map(|(_, target)| target)
            .chain(std::iter::once(&session.warm))
        {
            session.gl.delete_framebuffer(target.fbo);
            session.gl.delete_texture(target.texture);
        }
        session.bank.delete(&session.gl);
        session.gl.delete_texture(session.video);
        session.gl.delete_vertex_array(session.vao);
        session.gl.finish();
        if session.stop {
            eprintln!("bench: stop requested, leaving at a frame boundary");
        }
    }
    eprintln!("bench: done");
    Ok(())
}

fn quit_requested(events: &mut sdl2::EventPump) -> bool {
    events.poll_iter().take(64).any(|event| {
        matches!(
            event,
            Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                }
        )
    })
}
