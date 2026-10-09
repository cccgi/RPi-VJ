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
/// Input textures per shader id, from the asset store's manifest: the `iChannelN` inputs
/// of each shader's image pass that a file in the store satisfies.
pub fn channels(root: &Path) -> std::collections::HashMap<String, Vec<rpi_vj_native::texture::Channel>> {
    let store = root.join("target/shader-bank/assets");
    let mut found = std::collections::HashMap::new();
    let Ok(text) = std::fs::read_to_string(store.join("manifest.json")) else { return found };
    let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&text) else { return found };
    let mut files = std::collections::HashMap::new();
    for asset in manifest["assets"].as_array().into_iter().flatten() {
        if let (Some(source), Some("ok"), Some(file)) =
            (asset["source_path"].as_str(), asset["status"].as_str(), asset["local_file"].as_str())
        {
            files.insert(source, file);
        }
    }
    for (id, shader) in manifest["shaders"].as_object().into_iter().flatten() {
        let passes = shader["passes"].as_array().into_iter().flatten();
        let Some(image) = passes.into_iter().find(|pass| pass["type"] == "image") else { continue };
        let mut list = Vec::new();
        for input in image["channels"].as_array().into_iter().flatten() {
            let kind = match input["type"].as_str() {
                Some("texture") => "2d",
                Some("cubemap") => "cube",
                Some("volume") => "volume",
                _ => continue,
            };
            let Some(file) = input["asset"].as_str().and_then(|asset| files.get(asset)) else { continue };
            let sampler = &input["sampler"];
            let binding = serde_json::json!({
                "channel": input["channel"],
                "path": store.join(file),
                "kind": kind,
                "filter": sampler["filter"].as_str().unwrap_or("mipmap"),
                "wrap": sampler["wrap"].as_str().unwrap_or("repeat"),
                "vflip": sampler["vflip"].as_bool().unwrap_or(true),
            });
            if let Ok(channel) = serde_json::from_value(binding) {
                list.push(channel);
            }
        }
        if !list.is_empty() {
            found.insert(id.clone(), list);
        }
    }
    found
}
/// Ids of the shaders in the show that runs on the Pi, in show order.
pub fn pi_bank(root: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(root.join("engine/show/show.json")).unwrap_or_default();
    let show: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    let shaders = show["shaders"].as_array().into_iter().flatten();
    shaders
        .filter_map(|shader| Some(Path::new(shader["path"].as_str()?).file_stem()?.to_str()?.to_owned()))
        .collect()
}
