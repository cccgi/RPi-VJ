//! Emit prepared shader sources for real driver/compiler validation.
use anyhow::Result;
use std::path::Path;
fn main() -> Result<()> {
    let output = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/prepared-shaders".into());
    std::fs::create_dir_all(&output)?;
    for entry in std::fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/shaders"))?
    {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "fs") {
            let shader = rpi_vj_native::shader::load(&path)?;
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| anyhow::anyhow!("invalid shader path"))?;
            std::fs::write(
                Path::new(&output).join(format!("{name}.frag")),
                shader.source,
            )?;
        }
    }
    Ok(())
}
