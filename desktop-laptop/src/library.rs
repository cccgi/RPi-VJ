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
    /// The author's own tags, lower case.
    pub tags: Vec<String>,
    /// The code looks like a raymarcher.
    pub raymarch: bool,
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
                tags: shader["tags"].as_array().into_iter().flatten().filter_map(|tag| Some(tag.as_str()?.to_lowercase())).collect(),
                raymarch: shader["features"]["raymarch"].as_bool().unwrap_or(false),
            })
        })
        .collect();
    entries.sort_by_key(|entry| entry.title().to_lowercase());
    Ok(entries)
}
/// Groups worked out from each shader's tags, code and author, largest subjects first,
/// then one per author with ten or more shaders. A shader can be in several.
pub fn groups(entries: &[Entry]) -> Vec<(String, std::collections::BTreeSet<usize>)> {
    const SUBJECTS: [(&str, &[&str]); 11] = [
        ("Fractals", &["fractal", "fractals", "mandelbrot", "julia", "menger", "mandelbulb", "kifs", "ifs"]),
        ("Raymarched 3D", &["raymarch", "raymarching", "sdf", "3d", "volumetric", "voxel", "dda"]),
        ("Tunnels", &["tunnel"]),
        ("Tiny code", &["golf", "short", "small"]),
        ("2D", &["2d"]),
        ("Abstract", &["abstract"]),
        ("Glow", &["glow"]),
        ("Space", &["space", "stars", "galaxy", "nebula"]),
        ("Nature", &["cloud", "clouds", "fire", "water", "terrain", "ocean", "sea"]),
        ("Spirals", &["spiral"]),
        ("Noise", &["noise", "procedural", "fbm"]),
    ];
    let mut found: Vec<(String, std::collections::BTreeSet<usize>)> = SUBJECTS.iter().map(|(name, _)| ((*name).to_owned(), Default::default())).collect();
    let mut other = std::collections::BTreeSet::new();
    let mut textured = std::collections::BTreeSet::new();
    let mut authors: std::collections::BTreeMap<&str, std::collections::BTreeSet<usize>> = Default::default();
    for (index, entry) in entries.iter().enumerate().filter(|(_, entry)| entry.runnable()) {
        let mut placed = false;
        for (subject, (name, words)) in SUBJECTS.iter().enumerate() {
            let fits = entry.tags.iter().any(|tag| words.contains(&tag.as_str()))
                || (*name == "Raymarched 3D" && entry.raymarch)
                || (*name == "Tiny code" && entry.name.to_lowercase().contains("chars"));
            if fits {
                found[subject].1.insert(index);
                placed = true;
            }
        }
        if !placed {
            other.insert(index);
        }
        if entry.class == "needs_texture" {
            textured.insert(index);
        }
        if !entry.author.is_empty() {
            authors.entry(&entry.author).or_default().insert(index);
        }
    }
    found.push(("Uses textures".into(), textured));
    found.push(("Everything else".into(), other));
    let mut by_author: Vec<_> = authors.into_iter().filter(|(_, set)| set.len() >= 10).collect();
    by_author.sort_by_key(|(_, set)| std::cmp::Reverse(set.len()));
    found.extend(by_author.into_iter().map(|(author, set)| (format!("By {author}"), set)));
    found.retain(|(_, set)| !set.is_empty());
    found
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
