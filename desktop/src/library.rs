//! The shader library: every shader the catalog knows, with where its source lives.
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub struct Entry {
    pub id: String,
    pub name: String,
    pub author: String,
    /// `selfcontained`, `original18`, `needs_texture` or `multipass`.
    pub class: String,
    pub path: PathBuf,
}
impl Entry {
    /// Multipass shaders need buffer passes, which the single-pass engine does not have.
    pub fn runnable(&self) -> bool {
        self.class != "multipass"
    }
    pub fn title(&self) -> &str {
        if self.name.is_empty() { &self.id } else { &self.name }
    }
}
/// The repository root: `RPI_VJ_ROOT`, else the nearest folder above the working
/// directory or the executable that holds the catalog, else where this was built.
pub fn root() -> PathBuf {
    const CATALOG: &str = "target/shader-bank/catalog.json";
    if let Some(root) = std::env::var_os("RPI_VJ_ROOT") {
        return root.into();
    }
    let starts = [std::env::current_dir().ok(), std::env::current_exe().ok()];
    for start in starts.into_iter().flatten() {
        if let Some(found) = start.ancestors().find(|dir| dir.join(CATALOG).is_file()) {
            return found.to_owned();
        }
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap_or(Path::new(".")).to_owned()
}
pub fn load(root: &Path) -> Result<Vec<Entry>> {
    let path = root.join("target/shader-bank/catalog.json");
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let catalog: serde_json::Value = serde_json::from_str(&text)?;
    let shaders = catalog["shaders"].as_array().context("catalog has no shader list")?;
    let text = |value: &serde_json::Value| value.as_str().unwrap_or("").trim().to_owned();
    let mut entries: Vec<Entry> = shaders
        .iter()
        .filter_map(|shader| {
            let path = root.join(shader["path"].as_str()?);
            Some(Entry {
                id: text(&shader["id"]),
                name: text(&shader["name"]),
                author: text(&shader["author"]),
                class: text(&shader["class"]),
                path,
            })
        })
        .collect();
    entries.sort_by_key(|entry| entry.title().to_lowercase());
    Ok(entries)
}
