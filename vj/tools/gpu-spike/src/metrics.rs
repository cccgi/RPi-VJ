use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

pub struct MetricsLogger {
    writer: BufWriter<File>,
    adapter_name: String,
    backend: String,
}

impl MetricsLogger {
    pub fn new(path: &Path, adapter_name: String, backend: String) -> std::io::Result<Self> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        writeln!(writer, "frame,wall_time_s,frame_ms,fps,adapter_name,backend")?;
        writer.flush()?;
        Ok(Self {
            writer,
            adapter_name,
            backend,
        })
    }

    pub fn record(&mut self, frame: u64, wall_time_s: f32, frame_ms: f32) {
        let fps = if frame_ms > 0.0 { 1000.0 / frame_ms } else { 0.0 };
        let _ = writeln!(
            self.writer,
            "{},{:.4},{:.3},{:.2},{},{}",
            frame, wall_time_s, frame_ms, fps, self.adapter_name, self.backend
        );
    }

    pub fn flush(&mut self) {
        let _ = self.writer.flush();
    }
}
