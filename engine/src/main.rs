use anyhow::{ensure, Context, Result};
use rpi_vj_native::{native, show::Show};
use std::path::Path;
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|s| s == "--help") {
        println!("rpi-vj-native SHOW.json [--control-socket PATH] [--windowed] [--allow-software]\nDefault: fullscreen, hardware decoder required. No browser backend.");
        return Ok(());
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
