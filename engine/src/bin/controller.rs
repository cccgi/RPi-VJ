use anyhow::{Context, Result};
use rpi_vj_native::{controller_runtime, show::Show};
use std::path::Path;
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help") {
        println!("rpi-vj-controller SHOW.json SOCKET_PATH\nNative USB HID controller; Linux/Unix local IPC; no graphics dependencies.");
        return Ok(());
    }
    anyhow::ensure!(
        args.len() == 2,
        "usage: rpi-vj-controller SHOW.json SOCKET_PATH"
    );
    let show = Show::load(Path::new(args.first().context("show required")?))?;
    controller_runtime::run(show, Path::new(&args[1]))
}
