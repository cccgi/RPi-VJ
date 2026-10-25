//! The Pi's Stream Deck layout, edited in place: which shader or clip of the Pi's show
//! sits on which key of which page. The same job as `studio/deck-layout/deck_layout.py`,
//! inside the app. The show is kept as plain JSON so that nothing in it is lost.
use eframe::egui;
use serde_json::{json, Value};
use std::{collections::HashMap, path::PathBuf, sync::mpsc::Receiver};

const SHOW: &str = "target/shader-bank/live/show.json";
const PI_SHOW: &str = "/srv/rpi-vj-native/show.json";
/// Keys the controller keeps for itself on every page.
const SYSTEM: [(u64, &str); 4] = [(24, "PREV"), (25, "NEXT"), (30, "RESET"), (31, "BLACK")];
const KEY: f32 = 104.0;
const CANVASES: [&str; 3] = ["scroll", "sweep", "tile"];

/// What is being dragged: a key of the page in view, or something from the tray
/// (`true` for a shader, and its number in the show).
enum Drag {
    Key(u64),
    Item(bool, usize),
}
/// What can be asked of the Pi.
#[derive(Clone, Copy)]
pub enum Remote {
    Pull,
    Sync,
}
pub struct DeckEditor {
    path: PathBuf,
    show: Value,
    page: usize,
    /// Undo: the layout as last noted, the layouts to go back to, and those to go forward to.
    base: Value,
    undo: Vec<Value>,
    redo: Vec<Value>,
    dirty: bool,
    status: String,
    pictures: HashMap<String, Option<egui::TextureHandle>>,
    renaming: Option<(u64, String)>,
    page_name: Option<String>,
    /// Talking to the Pi: the password typed for this run (never stored), what is
    /// waiting for it or for a yes, and the job under way.
    password: Option<String>,
    typed: String,
    asking: Option<Remote>,
    confirming: bool,
    job: Option<(Remote, Receiver<Result<String, String>>)>,
}
fn item_of(binding: &Value) -> Option<(bool, usize)> {
    let command = &binding["command"];
    match command["type"].as_str()? {
        "TriggerShader" => Some((true, command["shader"].as_u64()? as usize)),
        "TriggerClip" => Some((false, command["clip"].as_u64()? as usize)),
        _ => None,
    }
}
/// What was dropped here, if it is of this kind (see `dropped` in `main.rs`).
fn dropped<T: std::any::Any + Send + Sync>(response: &egui::Response) -> Option<std::sync::Arc<T>> {
    if egui::DragAndDrop::has_payload_of_type::<T>(&response.ctx) {
        response.dnd_release_payload::<T>()
    } else {
        None
    }
}
/// Run `ssh` or `scp`. Without a password only key authentication is tried; with one,
/// it reaches ssh through a helper that echoes an environment variable of this process.
fn remote(program: &str, args: &[String], password: Option<&str>, input: Option<&str>) -> Result<String, String> {
    use std::io::Write;
    let mut command = std::process::Command::new(program);
    command.args(["-o", "StrictHostKeyChecking=accept-new", "-o", "ConnectTimeout=10"]);
    let helper = std::env::temp_dir().join(format!("rpi-vj-askpass-{}.cmd", std::process::id()));
    match password {
        Some(password) => {
            let body = if cfg!(windows) { "@echo %RPI_VJ_ASKPASS%\r\n" } else { "#!/bin/sh\necho \"$RPI_VJ_ASKPASS\"\n" };
            std::fs::write(&helper, body).map_err(|error| error.to_string())?;
            command.env("SSH_ASKPASS", &helper).env("SSH_ASKPASS_REQUIRE", "force").env("RPI_VJ_ASKPASS", password).env("DISPLAY", ":0");
        }
        None => {
            command.args(["-o", "BatchMode=yes"]);
        }
    }
    command.args(args).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    command.stdin(if input.is_some() { std::process::Stdio::piped() } else { std::process::Stdio::null() });
    let run = command.spawn().and_then(|mut child| {
        if let (Some(text), Some(mut stdin)) = (input, child.stdin.take()) {
            let _ = stdin.write_all(text.as_bytes());
        }
        child.wait_with_output()
    });
    let _ = std::fs::remove_file(&helper);
    let output = run.map_err(|error| format!("could not run {program}: {error}"))?;
    let errors = String::from_utf8_lossy(&output.stderr).into_owned();
    if output.status.code() == Some(255) || errors.contains("Permission denied") {
        return Err(format!("auth: {}", errors.trim()));
    }
    if !output.status.success() {
        return Err(errors.trim().chars().rev().take(600).collect::<String>().chars().rev().collect());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
impl DeckEditor {
    pub fn new(root: &std::path::Path) -> Self {
        let path = root.join(SHOW);
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let mut show: Value = serde_json::from_str(&text).unwrap_or_else(|_| json!({ "version": 2, "shaders": [], "clips": [], "pages": [] }));
        if show["pages"].as_array().is_none_or(Vec::is_empty) {
            show["pages"] = json!([{ "name": "PAGE 1", "bindings": [] }]);
        }
        Self {
            status: if text.is_empty() { format!("no show at {}", path.display()) } else { String::new() },
            path,
            base: show.clone(),
            undo: Vec::new(),
            redo: Vec::new(),
            show,
            page: 0,
            dirty: false,
            pictures: HashMap::new(),
            renaming: None,
            page_name: None,
            password: std::env::var("RPI_VJ_PI_PASSWORD").ok(),
            typed: String::new(),
            asking: None,
            confirming: false,
            job: None,
        }
    }
    fn host() -> String {
        std::env::var("RPI_VJ_HOST").unwrap_or_else(|_| "rpi@rpi-vj3.local".into())
    }
    fn pages(&self) -> usize {
        self.show["pages"].as_array().map_or(0, Vec::len)
    }
    fn bindings(&mut self, page: usize) -> &mut Vec<Value> {
        if !self.show["pages"][page]["bindings"].is_array() {
            self.show["pages"][page]["bindings"] = json!([]);
        }
        self.show["pages"][page]["bindings"].as_array_mut().expect("just made an array")
    }
    fn at(&self, page: usize, key: u64) -> Option<&Value> {
        self.show["pages"][page]["bindings"].as_array()?.iter().find(|binding| binding["key"].as_u64() == Some(key))
    }
    fn label_of(&self, (shader, index): (bool, usize)) -> String {
        let list = if shader { "shaders" } else { "clips" };
        self.show[list][index]["label"].as_str().unwrap_or("?").to_owned()
    }
    /// Shaders and clips of the show that are on no key.
    fn unplaced(&self) -> Vec<(bool, usize)> {
        let used: Vec<(bool, usize)> =
            self.show["pages"].as_array().into_iter().flatten().flat_map(|page| page["bindings"].as_array().into_iter().flatten()).filter_map(item_of).collect();
        let count = |list: &str| self.show[list].as_array().map_or(0, Vec::len);
        let all = (0..count("shaders")).map(|index| (true, index)).chain((0..count("clips")).map(|index| (false, index)));
        all.filter(|item| !used.contains(item)).collect()
    }
    fn free_key(&self, page: usize) -> Option<u64> {
        (0..32).find(|key| !SYSTEM.iter().any(|(system, _)| system == key) && self.at(page, *key).is_none())
    }
    fn changed(&mut self, what: impl Into<String>) {
        // The layout as it was before this change is a step to go back to.
        if self.base != self.show {
            self.undo.push(std::mem::replace(&mut self.base, self.show.clone()));
            self.redo.clear();
            if self.undo.len() > 200 {
                self.undo.remove(0);
            }
        }
        self.dirty = true;
        self.status = format!("{} (not saved)", what.into());
    }
    /// For `--deck-undo-test`: put a shader on a free key, undo, redo, undo, and say what
    /// the key held after each. Nothing is saved.
    pub fn undo_test(&mut self) -> String {
        let key = self.free_key(0).unwrap_or(26);
        let held = |deck: &Self| deck.at(0, key).map(|binding| binding["label"].as_str().unwrap_or("?").to_owned());
        let mut seen = vec![held(self)];
        self.put(0, key, (true, 0));
        self.changed("test");
        seen.push(held(self));
        for back in [true, false, true] {
            self.step(back);
            seen.push(held(self));
        }
        format!("key {key}: {seen:?}; undo {} redo {}", self.undo.len(), self.redo.len())
    }
    /// One step back through the changes to the layout, or forward again.
    fn step(&mut self, back: bool) {
        let Some(target) = (if back { self.undo.pop() } else { self.redo.pop() }) else { return };
        let now = std::mem::replace(&mut self.show, target);
        if back {
            self.redo.push(now);
        } else {
            self.undo.push(now);
        }
        self.base = self.show.clone();
        self.page = self.page.min(self.pages().saturating_sub(1));
        self.pictures.clear();
        self.dirty = true;
        self.status = format!("{} (not saved)", if back { "undone" } else { "redone" });
    }
    fn put(&mut self, page: usize, key: u64, item: (bool, usize)) {
        let label = self.label_of(item);
        let command = if item.0 { json!({ "type": "TriggerShader", "shader": item.1 }) } else { json!({ "type": "TriggerClip", "clip": item.1 }) };
        let color = if item.0 { json!([25, 75, 120]) } else { json!([35, 65, 95]) };
        self.bindings(page).retain(|binding| binding["key"].as_u64() != Some(key));
        self.bindings(page).push(json!({ "key": key, "label": label, "command": command, "color": color }));
    }
    /// The show's number for a library shader, by its id: the name of its file.
    fn shader_number(&self, id: &str) -> Option<usize> {
        self.show["shaders"].as_array()?.iter().position(|shader| {
            shader["path"].as_str().and_then(|path| std::path::Path::new(path).file_stem()).is_some_and(|stem| stem == id)
        })
    }
    fn save(&mut self) -> bool {
        // Keys in order, none on the controller's own keys.
        for page in self.show["pages"].as_array_mut().into_iter().flatten() {
            if let Some(bindings) = page["bindings"].as_array_mut() {
                bindings.retain(|binding| !SYSTEM.iter().any(|(key, _)| binding["key"].as_u64() == Some(*key)));
                bindings.sort_by_key(|binding| binding["key"].as_u64());
            }
        }
        let text = serde_json::to_string_pretty(&self.show).unwrap_or_default();
        let _ = std::fs::copy(&self.path, self.path.with_extension("json.bak"));
        match std::fs::write(&self.path, text) {
            Ok(()) => {
                self.dirty = false;
                self.status = format!("saved {}", self.path.display());
                true
            }
            Err(error) => {
                self.status = format!("could not save: {error}");
                false
            }
        }
    }
    /// Start talking to the Pi, asking for the password first where it is needed.
    pub fn start(&mut self, what: Remote) {
        if self.job.is_some() {
            return;
        }
        if matches!(what, Remote::Sync) && (self.password.is_none() || !self.save()) {
            // Installing the show on the Pi needs sudo, and sudo needs the password.
            self.asking = self.password.is_none().then_some(what);
            return;
        }
        let (host, password, path) = (Self::host(), self.password.clone(), self.path.display().to_string());
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let password = password.as_deref();
            let done = match what {
                Remote::Pull => remote("ssh", &[host, format!("cat {PI_SHOW}")], password, None),
                Remote::Sync => remote("scp", &[path, format!("{host}:/tmp/deck-layout-show.json")], password, None).and_then(|_| {
                    let script = format!(
                        "set -e; test ! -e /home/rpi/DEPLOY_IN_PROGRESS; \
                         python3 -c 'import json,sys; json.load(open(sys.argv[1]))' /tmp/deck-layout-show.json; \
                         cp -a {PI_SHOW} {PI_SHOW}.before-sync; \
                         systemctl stop rpi-vj-controller rpi-vj-native; \
                         install -m 644 -o vj -g vj /tmp/deck-layout-show.json {PI_SHOW}; \
                         systemctl start rpi-vj-native rpi-vj-controller; sleep 6; \
                         systemctl is-active rpi-vj-native rpi-vj-controller"
                    );
                    let quoted = serde_json::to_string(&script).unwrap_or_default();
                    let input = format!("{}\n", password.unwrap_or_default());
                    remote("ssh", &[host, format!("sudo -S -p '' sh -c {quoted}")], password, Some(&input))
                }),
            };
            let _ = sender.send(done);
        });
        self.status = match what {
            Remote::Pull => "downloading the Pi's layout…".into(),
            Remote::Sync => "uploading and restarting the show on the Pi…".into(),
        };
        self.job = Some((what, receiver));
    }
    fn finish(&mut self) {
        let Some((what, receiver)) = &self.job else { return };
        let Ok(done) = receiver.try_recv() else { return };
        let what = *what;
        self.job = None;
        match (what, done) {
            (_, Err(error)) if error.starts_with("auth:") => {
                // Key authentication did not do it, or the password was wrong: ask.
                self.password = None;
                self.asking = Some(what);
                self.status = "the Pi wants a password".into();
            }
            (_, Err(error)) => self.status = format!("failed: {error}"),
            (Remote::Pull, Ok(text)) => match serde_json::from_str::<Value>(&text) {
                Ok(show) if show["pages"].is_array() => {
                    self.show = show;
                    self.page = 0;
                    self.pictures.clear();
                    self.changed("pulled");
                    self.save();
                    self.status = "loaded the Pi's layout".into();
                }
                _ => self.status = "what the Pi sent is not a show".into(),
            },
            (Remote::Sync, Ok(said)) => {
                let active: Vec<&str> = said.split_whitespace().collect();
                self.status = if active == ["active", "active"] {
                    "synced: the Pi is running this layout".into()
                } else {
                    format!("uploaded, but a service is not active: {}", active.join(" "))
                };
            }
        }
    }
    fn picture(&mut self, ctx: &egui::Context, item: (bool, usize)) -> Option<egui::TextureId> {
        let list = if item.0 { "shaders" } else { "clips" };
        let relative = self.show[list][item.1]["thumbnail"].as_str()?.to_owned();
        if !self.pictures.contains_key(&relative) {
            let file = self.path.parent()?.join(&relative);
            let loaded = image::open(file).ok().map(|picture| {
                let picture = picture.to_rgb8();
                let image = egui::ColorImage::from_rgb([picture.width() as usize, picture.height() as usize], picture.as_raw());
                ctx.load_texture(format!("deck-{relative}"), image, egui::TextureOptions::LINEAR)
            });
            self.pictures.insert(relative.clone(), loaded);
        }
        self.pictures.get(&relative)?.as_ref().map(egui::TextureHandle::id)
    }
    /// One tile: a key of the page, or an item in the tray.
    fn tile(&mut self, ui: &mut egui::Ui, item: Option<(bool, usize)>, caption: &str, size: f32, system: bool) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, egui::Color32::from_gray(if system { 44 } else if item.is_some() { 36 } else { 24 }));
        if let Some(texture) = item.and_then(|item| self.picture(ui.ctx(), item)) {
            let area = egui::Rect::from_min_size(rect.min, egui::vec2(size, size * 0.78));
            painter.image(texture, area, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
        }
        let (at, align, text_size) = if item.is_some() {
            (rect.left_bottom() + egui::vec2(4.0, -3.0), egui::Align2::LEFT_BOTTOM, 10.5)
        } else {
            (rect.center(), egui::Align2::CENTER_CENTER, 13.0)
        };
        painter.text(at, align, caption, egui::FontId::proportional(text_size), egui::Color32::from_gray(if system { 150 } else { 225 }));
        let carried = response.dnd_hover_payload::<Drag>().is_some() || response.dnd_hover_payload::<crate::Dragged>().is_some();
        let stroke = if carried && !system { (2.0, egui::Color32::from_rgb(240, 200, 60)) } else { (1.0, egui::Color32::from_gray(64)) };
        painter.rect_stroke(rect, 4.0, stroke, egui::StrokeKind::Inside);
        response
    }
    /// The whole view. `id_of` gives a library shader's id, for shaders dragged in from the library.
    pub fn ui(&mut self, ui: &mut egui::Ui, id_of: &dyn Fn(usize) -> String) {
        self.finish();
        self.page = self.page.min(self.pages().saturating_sub(1));
        let page = self.page;
        ui.horizontal_wrapped(|ui| {
            ui.heading("Pi Stream Deck layout");
            if ui.button("Save").clicked() {
                self.save();
            }
            // Ctrl+Z and Ctrl+Y (or Ctrl+Shift+Z) belong to the layout while it is in view.
            let typing = ui.ctx().wants_keyboard_input();
            let pressed = |modifiers, key| !typing && ui.input_mut(|input| input.consume_key(modifiers, key));
            let redo = pressed(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Z) || pressed(egui::Modifiers::COMMAND, egui::Key::Y);
            let undo = !redo && pressed(egui::Modifiers::COMMAND, egui::Key::Z);
            if ui.add_enabled(!self.undo.is_empty(), egui::Button::new("Undo")).on_hover_text("Ctrl+Z").clicked() || undo {
                self.step(true);
            }
            if ui.add_enabled(!self.redo.is_empty(), egui::Button::new("Redo")).on_hover_text("Ctrl+Y").clicked() || redo {
                self.step(false);
            }
            let busy = self.job.is_some();
            if ui.add_enabled(!busy, egui::Button::new("Sync to Pi")).on_hover_text("Save, upload and restart the show on the Pi").clicked() {
                self.confirming = true;
            }
            if ui.add_enabled(!busy, egui::Button::new("Pull from Pi")).on_hover_text("Replace this layout with the one the Pi is running").clicked() {
                self.start(Remote::Pull);
            }
            if ui.button("Add page").clicked() && self.pages() < 128 {
                let name = format!("PAGE {}", self.pages() + 1);
                if let Some(pages) = self.show["pages"].as_array_mut() {
                    pages.push(json!({ "name": name, "bindings": [] }));
                }
                self.page = self.pages() - 1;
                self.changed("page added");
            }
            if ui.button("Rename page").clicked() {
                self.page_name = Some(self.show["pages"][page]["name"].as_str().unwrap_or("").to_owned());
            }
            if ui.add_enabled(self.pages() > 1, egui::Button::new("Delete page")).on_hover_text("Its keys go back to the tray").clicked() {
                if let Some(pages) = self.show["pages"].as_array_mut() {
                    pages.remove(page);
                }
                self.page = page.saturating_sub(1);
                self.changed("page deleted");
            }
            if busy {
                ui.spinner();
            }
            ui.weak(format!("{}{}", self.status, if self.dirty && self.status.is_empty() { "not saved" } else { "" }));
        });
        if let Some(name) = &mut self.page_name {
            let mut done = false;
            ui.horizontal(|ui| {
                ui.label("Page name");
                done = ui.text_edit_singleline(name).lost_focus() || ui.button("OK").clicked();
            });
            if done {
                let name = self.page_name.take().unwrap_or_default();
                self.show["pages"][page]["name"] = json!(name);
                self.changed("page renamed");
            }
        }
        // Pages: click to see one, drop a key on one to send it there.
        ui.horizontal_wrapped(|ui| {
            for index in 0..self.pages() {
                let name = self.show["pages"][index]["name"].as_str().unwrap_or("?").to_owned();
                let response = ui.selectable_label(index == page, name);
                if response.clicked() {
                    self.page = index;
                }
                if let Some(Drag::Key(key)) = dropped::<Drag>(&response).as_deref() {
                    if let (Some(free), true) = (self.free_key(index), index != page) {
                        if let Some(binding) = self.bindings(page).iter_mut().find(|binding| binding["key"].as_u64() == Some(*key)) {
                            binding["key"] = json!(free);
                        }
                        let moved: Vec<Value> = self.bindings(page).iter().filter(|binding| binding["key"].as_u64() == Some(free)).cloned().collect();
                        self.bindings(page).retain(|binding| binding["key"].as_u64() != Some(free));
                        self.bindings(index).extend(moved);
                        self.changed("moved to another page");
                    }
                }
            }
        });
        ui.weak("Drag a key onto another to swap them, onto a page above to send it there, onto the tray to take it off the deck. Double-click renames; right-click sets how the Pi's move knobs treat a shader. Library shaders that are in the Pi's show can be dropped on a key.");
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            for row in 0..4u64 {
                ui.horizontal(|ui| {
                    for column in 0..8u64 {
                        let key = row * 8 + column;
                        let reserved = SYSTEM.iter().find(|(system, _)| *system == key).map(|(_, name)| *name);
                        let bound = self.at(page, key).cloned();
                        let item = bound.as_ref().and_then(item_of);
                        let caption = match (&bound, reserved) {
                            (_, Some(name)) => name.to_owned(),
                            (Some(binding), _) => binding["label"].as_str().unwrap_or("").to_owned(),
                            _ => String::new(),
                        };
                        let response = self.tile(ui, item.filter(|_| reserved.is_none()), &caption, KEY, reserved.is_some());
                        if reserved.is_some() {
                            continue;
                        }
                        if let Some((true, shader)) = item {
                            let canvas = self.show["shaders"][shader]["canvas"].as_str().unwrap_or("scroll").to_owned();
                            if canvas != "scroll" {
                                ui.painter().text(response.rect.right_top() + egui::vec2(-4.0, 3.0), egui::Align2::RIGHT_TOP, &canvas, egui::FontId::proportional(10.0), egui::Color32::WHITE);
                            }
                        }
                        if bound.is_some() && response.dragged() {
                            response.dnd_set_drag_payload(Drag::Key(key));
                        }
                        if bound.is_some() && response.double_clicked() {
                            self.renaming = Some((key, caption.clone()));
                        }
                        match dropped::<Drag>(&response).as_deref() {
                            Some(Drag::Key(from)) if *from != key => {
                                // Swap: whatever is here goes where the dragged key was.
                                for binding in self.bindings(page).iter_mut() {
                                    let at = binding["key"].as_u64();
                                    if at == Some(*from) {
                                        binding["key"] = json!(key);
                                    } else if at == Some(key) {
                                        binding["key"] = json!(from);
                                    }
                                }
                                self.changed("keys moved");
                            }
                            Some(Drag::Item(shader, index)) => {
                                self.put(page, key, (*shader, *index));
                                self.changed("put on the deck");
                            }
                            _ => {}
                        }
                        if let Some(dragged) = dropped::<crate::Dragged>(&response) {
                            // From the library: only shaders the Pi's show already holds.
                            let (mut placed, mut missing, mut next) = (0, 0, Some(key));
                            for source in &dragged.0 {
                                let crate::Source::Shader(entry) = source else { continue };
                                match (self.shader_number(&id_of(*entry)), next) {
                                    (Some(number), Some(at)) => {
                                        self.put(page, at, (true, number));
                                        placed += 1;
                                        next = self.free_key(page);
                                    }
                                    (None, _) => missing += 1,
                                    _ => {}
                                }
                            }
                            let note = if missing > 0 { format!("; {missing} not in the Pi's show (they have to be qualified for the Pi first)") } else { String::new() };
                            self.changed(format!("{placed} put on the deck{note}"));
                        }
                        if let (Some(binding), Some((shader, index))) = (&bound, item) {
                            let _ = binding;
                            response.context_menu(|ui| {
                                if shader {
                                    let now = self.show["shaders"][index]["canvas"].as_str().unwrap_or("scroll").to_owned();
                                    for canvas in CANVASES {
                                        if ui.selectable_label(now == canvas, format!("Move knobs: {canvas}")).clicked() {
                                            if canvas == "scroll" {
                                                if let Some(entry) = self.show["shaders"][index].as_object_mut() {
                                                    entry.remove("canvas");
                                                }
                                            } else {
                                                self.show["shaders"][index]["canvas"] = json!(canvas);
                                            }
                                            self.changed(format!("move knobs: {canvas}"));
                                            ui.close_menu();
                                        }
                                    }
                                    ui.separator();
                                }
                                if ui.button("Take off the deck").clicked() {
                                    self.bindings(page).retain(|binding| binding["key"].as_u64() != Some(key));
                                    self.changed("taken off the deck");
                                    ui.close_menu();
                                }
                            });
                        }
                    }
                });
            }
            ui.add_space(6.0);
            // The tray: what the show holds that is on no key. Drop a key here to take it off.
            let waiting = self.unplaced();
            let tray = ui
                .group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.strong(format!("Tray: {} in the show, on no key", waiting.len()));
                    ui.horizontal_wrapped(|ui| {
                        for item in &waiting {
                            let caption = self.label_of(*item);
                            let response = self.tile(ui, Some(*item), &caption, 80.0, false);
                            if response.dragged() {
                                response.dnd_set_drag_payload(Drag::Item(item.0, item.1));
                            }
                        }
                    });
                })
                .response;
            if let Some(Drag::Key(key)) = dropped::<Drag>(&tray).as_deref() {
                self.bindings(page).retain(|binding| binding["key"].as_u64() != Some(*key));
                self.changed("taken off the deck");
            }
        });
        self.windows(ui.ctx(), page);
    }
    /// The small windows: a key's caption, the yes before a sync, the Pi's password.
    fn windows(&mut self, ctx: &egui::Context, page: usize) {
        if let Some((key, mut caption)) = self.renaming.take() {
            let mut keep = true;
            egui::Window::new("Key caption").collapsible(false).resizable(false).show(ctx, |ui| {
                ui.text_edit_singleline(&mut caption);
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        if let Some(binding) = self.bindings(page).iter_mut().find(|binding| binding["key"].as_u64() == Some(key)) {
                            binding["label"] = json!(caption.trim());
                        }
                        self.changed("caption changed");
                        keep = false;
                    }
                    keep &= !ui.button("Cancel").clicked();
                });
            });
            if keep {
                self.renaming = Some((key, caption));
            }
        }
        if self.confirming {
            egui::Window::new("Sync to Pi").collapsible(false).resizable(false).show(ctx, |ui| {
                ui.label("Upload this layout and restart the show on the Pi?");
                ui.label("The output goes black for a few seconds. The Pi keeps its previous show as show.json.before-sync.");
                ui.horizontal(|ui| {
                    if ui.button("Sync").clicked() {
                        self.confirming = false;
                        self.start(Remote::Sync);
                    }
                    if ui.button("Cancel").clicked() {
                        self.confirming = false;
                    }
                });
            });
        }
        if let Some(what) = self.asking {
            egui::Window::new("Pi password").collapsible(false).resizable(false).show(ctx, |ui| {
                ui.label(format!("Password for {} (kept for this run only):", Self::host()));
                let typed = ui.add(egui::TextEdit::singleline(&mut self.typed).password(true));
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() || (typed.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter))) {
                        self.password = Some(std::mem::take(&mut self.typed));
                        self.asking = None;
                        self.start(what);
                    }
                    if ui.button("Cancel").clicked() {
                        self.typed.clear();
                        self.asking = None;
                        self.status = "cancelled".into();
                    }
                });
            });
        }
    }
}
