//! Renders Stream Deck XL pages to contact-sheet PNGs without hardware.
//!
//! Every sheet goes through the controller's real pipeline (descriptor -> 96x96 JPEG,
//! rotated for the device) and is then decoded and turned back, so it shows what the
//! operator sees, JPEG artefacts included.
//!
//! cargo run --release --features controller --example deck_sheet -- \
//!     <shader-show.json> <legacy-show.json> <out-dir> <prefix> [bench-iterations]
#[cfg(not(feature = "controller"))]
fn main() {
    eprintln!("deck_sheet needs --features controller");
    std::process::exit(2);
}
#[cfg(feature = "controller")]
fn main() -> anyhow::Result<()> {
    sheet::run()
}
#[cfg(feature = "controller")]
mod sheet {
    use anyhow::{ensure, Context, Result};
    use image::{Rgb, RgbImage};
    use rpi_vj_native::{
        control::{BlendMode, Command, State},
        controller::Painter,
        show::Show,
        surface,
    };
    use std::{
        path::{Path, PathBuf},
        time::Instant,
    };

    const KEY: u32 = 96;
    /// The XL has roughly a fifth of a key of bezel between displays.
    const GAP: u32 = 20;
    const BODY: Rgb<u8> = Rgb([10, 10, 12]);

    struct Scene {
        name: &'static str,
        show: Show,
        state: State,
        online: bool,
    }

    fn scenes(shaders: &Show, legacy: &Show) -> Result<Vec<Scene>> {
        let apply = |state: &mut State, command| state.apply(command).map_err(anyhow::Error::msg);
        let mut out = Vec::new();
        // 1. Deployed shader bank, first page, one shader running.
        let mut state = State::from_show(shaders);
        apply(&mut state, Command::TriggerShader { shader: 2 })?;
        out.push(Scene {
            name: "1-shader-page",
            show: shaders.clone(),
            state,
            online: true,
        });
        // 2. Clip page: one clip playing, one loading, one shader running.
        let mut clips = State::from_show(legacy);
        clips.playing = Some(2);
        clips.requested = Some(5);
        apply(&mut clips, Command::TriggerShader { shader: 0 })?;
        out.push(Scene {
            name: "2-clip-page",
            show: legacy.clone(),
            state: clips.clone(),
            online: true,
        });
        // 3. Control pages.
        let mut state = State::from_show(legacy);
        state.page = 3;
        apply(&mut state, Command::SetVideoEffect { effect: 9 })?;
        out.push(Scene {
            name: "3a-control-video-fx",
            show: legacy.clone(),
            state,
            online: true,
        });
        let mut state = State::from_show(legacy);
        state.page = 4;
        apply(&mut state, Command::SetShaderEffect { effect: 5 })?;
        out.push(Scene {
            name: "3b-control-effect-fx",
            show: legacy.clone(),
            state,
            online: true,
        });
        let mut state = State::from_show(legacy);
        state.page = 1;
        apply(
            &mut state,
            Command::SetBlend {
                mode: BlendMode::Screen,
                opacity: 1.0,
            },
        )?;
        apply(&mut state, Command::ToggleHud)?;
        out.push(Scene {
            name: "3c-control-blend",
            show: legacy.clone(),
            state,
            online: true,
        });
        let mut state = State::from_show(legacy);
        state.page = 2;
        apply(&mut state, Command::SetOpacity { opacity: 0.5 })?;
        out.push(Scene {
            name: "3d-control-system",
            show: legacy.clone(),
            state,
            online: true,
        });
        // 4. Renderer offline, same content as scene 2.
        out.push(Scene {
            name: "4-renderer-offline",
            show: legacy.clone(),
            state: clips,
            online: false,
        });
        // 5. Faults: failed clip, missing clip files, failed and missing shaders, blackout on.
        let mut broken = legacy.clone();
        for clip in [3, 4, 12] {
            broken.clips[clip].path = PathBuf::from("/missing/clip.mp4");
        }
        broken.shaders[3].path = PathBuf::from("/missing/shader.fs");
        let mut state = State::from_show(&broken);
        state.playing = Some(0);
        state.failed = Some(1);
        state.failed_shaders.push(1);
        state.blackout = true;
        out.push(Scene {
            name: "5-failed-unavailable",
            show: broken,
            state,
            online: true,
        });
        // 6. Sparse page with long labels and many unbound keys.
        let mut state = State::from_show(legacy);
        state.page = 6;
        out.push(Scene {
            name: "6-sparse-long-labels",
            show: legacy.clone(),
            state,
            online: true,
        });
        // 7. Clip page whose clips have no thumbnails: one playing, one loading.
        let mut state = State::from_show(legacy);
        state.page = 5;
        state.playing = Some(27);
        state.requested = Some(30);
        out.push(Scene {
            name: "7-clips-no-thumbnails",
            show: legacy.clone(),
            state,
            online: true,
        });
        Ok(out)
    }

    fn sheet(jpegs: &[Vec<u8>], scale: u32) -> Result<RgbImage> {
        ensure!(jpegs.len() == 32, "a page has 32 keys");
        let step = KEY + GAP;
        let mut out = RgbImage::from_pixel(
            (8 * step + GAP) * scale,
            (4 * step + GAP) * scale,
            BODY,
        );
        for (index, jpeg) in jpegs.iter().enumerate() {
            // Undo the device rotation so the sheet reads as the operator sees it.
            let key = image::imageops::rotate180(&image::load_from_memory(jpeg)?.to_rgb8());
            ensure!(key.dimensions() == (KEY, KEY), "key image is not 96x96");
            let left = (GAP + (index as u32 % 8) * step) * scale;
            let top = (GAP + (index as u32 / 8) * step) * scale;
            for (x, y, pixel) in key.enumerate_pixels() {
                for dy in 0..scale {
                    for dx in 0..scale {
                        out.put_pixel(left + x * scale + dx, top + y * scale + dy, *pixel);
                    }
                }
            }
        }
        Ok(out)
    }

    pub fn run() -> Result<()> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        ensure!(
            args.len() >= 4,
            "usage: deck_sheet <shader-show.json> <legacy-show.json> <out-dir> <prefix> [bench-iterations]"
        );
        let shaders = Show::load(Path::new(&args[0]))?;
        let legacy = Show::load(Path::new(&args[1]))?;
        let out = Path::new(&args[2]);
        let prefix = &args[3];
        let iterations: usize = args.get(4).map_or(Ok(0), |n| n.parse())?;
        std::fs::create_dir_all(out).with_context(|| format!("create {}", out.display()))?;
        // Every page of the shader show as it stands, as a record of the layout.
        for index in 0..shaders.pages.len() {
            let mut state = State::from_show(&shaders);
            state.page = index;
            let jpegs = Painter::default().page(&surface::page(&shaders, &state), true)?;
            sheet(&jpegs, 2)?.save(out.join(format!("{prefix}-page-{}.png", index + 1)))?;
        }
        for scene in scenes(&shaders, &legacy)? {
            let keys = surface::page(&scene.show, &scene.state);
            let mut painter = Painter::default();
            let started = Instant::now();
            let jpegs = painter.page(&keys, scene.online)?;
            let cold = started.elapsed();
            for scale in [1, 2] {
                let path = out.join(format!("{prefix}-{}@{scale}x.png", scene.name));
                sheet(&jpegs, scale)?.save(&path)?;
            }
            let bytes: usize = jpegs.iter().map(Vec::len).sum();
            // A full repaint costs one 1024-byte HID report per 1016 JPEG bytes of each key.
            let packets: usize = jpegs.iter().map(|jpeg| jpeg.len().div_ceil(1016)).sum();
            print!(
                "{prefix} {:<24} first {:>7.2} ms, {:>6} JPEG bytes in {:>3} HID packets",
                scene.name,
                cold.as_secs_f64() * 1000.0,
                bytes,
                packets
            );
            if iterations > 0 {
                // Repaints of an already shown page: what a state change costs.
                let mut times = Vec::with_capacity(iterations);
                for _ in 0..iterations {
                    let started = Instant::now();
                    std::hint::black_box(painter.page(&keys, scene.online)?);
                    times.push(started.elapsed().as_secs_f64() * 1000.0);
                }
                times.sort_by(f64::total_cmp);
                print!(
                    ", repaint median {:.2} ms, worst {:.2} ms over {iterations}",
                    times[times.len() / 2],
                    times[times.len() - 1]
                );
            }
            println!();
        }
        Ok(())
    }
}
