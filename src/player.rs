use anyhow::{Context, Result};
use serde_json::json;
use std::{
    io::Write,
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
};

pub struct Mpv {
    child: Child,
    input: ChildStdin,
}

impl Mpv {
    pub fn start() -> Result<Self> {
        let mut child = Command::new("mpv")
            .args([
                "--no-config",
                "--idle=yes",
                "--force-window=yes",
                "--input-ipc-client=fd://0",
                "--vo=gpu-next",
                "--gpu-context=drm",
                "--gpu-api=opengl",
                "--hwdec=drm,auto",
                "--vd-lavc-dr=yes",
                "--video-sync=display-resample",
                "--interpolation=no",
                "--audio=no",
                "--cache=yes",
                "--demuxer-max-bytes=268435456",
                "--demuxer-readahead-secs=15",
                "--osd-level=0",
                "--really-quiet",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .context("spawn mpv (is it installed and can this user open /dev/dri/card1?)")?;
        let input = child.stdin.take().context("open mpv IPC stdin")?;
        Ok(Self { child, input })
    }

    pub fn load(&mut self, path: &Path) -> Result<()> {
        let command = json!({"command": ["loadfile", path, "replace"]});
        serde_json::to_writer(&mut self.input, &command)?;
        self.input.write_all(b"\n")?;
        self.input.flush()?;
        Ok(())
    }

    pub fn check(&mut self) -> Result<()> {
        if let Some(status) = self.child.try_wait()? {
            anyhow::bail!("mpv exited unexpectedly: {status}")
        }
        Ok(())
    }
}

