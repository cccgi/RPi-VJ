use anyhow::{ensure, Context, Result};
use rpi_vj_native::{native, show::Show};
use std::path::Path;
const USAGE: &str = "rpi-vj-native SHOW.json [--control-socket PATH] [--windowed] [--allow-software]\nrpi-vj-native --bench MANIFEST.json [--resume] [--windowed]\nDefault: fullscreen, hardware decoder required. No browser backend.\n--bench measures every manifest shader offscreen and appends JSONL results; --resume skips entries that already have a result.";
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|s| s == "--help") {
        println!("{USAGE}");
        return Ok(());
    }
    if args.first().is_some_and(|s| s == "--bench") {
        let manifest = args
            .get(1)
            .context("usage: rpi-vj-native --bench MANIFEST.json [--resume]")?;
        for other in &args[2..] {
            ensure!(
                other == "--resume" || other == "--windowed",
                "unknown bench argument {other}"
            );
        }
        return native::bench::run(
            Path::new(manifest),
            args.iter().any(|s| s == "--resume"),
            args.iter().any(|s| s == "--windowed"),
        );
    }
    let path = args
        .first()
        .context("usage: rpi-vj-native SHOW.json [--windowed] [--allow-software]")?;
    let mut socket = None;
    let mut cursor = 1;
    while cursor < args.len() {
        match args[cursor].as_str() {
            "--windowed" | "--allow-software" => {}
            "--control-socket" => {
                cursor += 1;
                socket = Some(std::path::PathBuf::from(
                    args.get(cursor).context("socket path required")?,
                ));
            }
            other => ensure!(false, "unknown argument {other}"),
        }
        cursor += 1;
    }
    native::run(
        Show::load(Path::new(path))?,
        args.iter().any(|s| s == "--windowed"),
        args.iter().any(|s| s == "--allow-software"),
        socket,
    )
}
