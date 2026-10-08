#!/usr/bin/env python3
"""Stream Deck layout editor for RPi-VJ (workstation only).

Arrange the shader and clip keys of a show by dragging them, then send the show
to the Pi. Needs Python 3.10+ with tkinter and Pillow, and `ssh`/`scp` on PATH.

    python studio/deck-layout/deck_layout.py [show.json]

Default show: target/shader-bank/live/show.json. Key pictures are read from the
paths in the show, relative to it.

  drag a key onto another key      swap the two
  drag a key onto a page button    move it to that page's first free key
  drag a key onto the tray         take it off the deck (it stays in the show)
  drag a tray item onto a key      put it on the deck
  double-click a key               rename its caption
  right-click a shader key         switch it between tiled and endless canvas
                                   (marked with a ring; see below)
  Pull from Pi                     replace the local layout with the Pi's
  Sync to Pi                       save, upload and restart the show services

Keys 25, 26, 31 and 32 (bottom row: PREV, NEXT, RESET, BLACK) belong to the
controller and cannot be assigned.

Canvas mode decides what the X-Touch layer B move and spin knobs do to a shader.
Tiled (default) repeats the finished picture, mirrored; right for textures,
patterns and small centred graphics you want repeated. Endless canvas moves
the shader over its own unbounded picture with no tiles; right for shaders
built around the screen centre, such as tunnels and portals. It only works on
shaders converted from Shadertoy. The Pi password is asked for when needed and
kept in memory for the session only; it is never written to disk.
"""
import copy
import json
import os
import subprocess
import sys
import tempfile
import tkinter as tk
from pathlib import Path
from tkinter import messagebox, simpledialog

from PIL import Image, ImageTk

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_SHOW = ROOT / "target" / "shader-bank" / "live" / "show.json"
PI_HOST = os.environ.get("RPI_VJ_HOST", "rpi@rpi-vj3.local")
PI_SHOW = "/srv/rpi-vj-native/show.json"
COLS, ROWS, CELL, GAP = 8, 4, 104, 8
SYSTEM_KEYS = {24: "PREV", 25: "NEXT", 30: "RESET", 31: "BLACK"}
MAX_PAGES = 128
BG, PANEL, EMPTY, SYSTEM, TEXT, ACCENT = "#0d0e11", "#16181d", "#1b1d23", "#23262d", "#e8e8ea", "#2f9bd6"


def item_of(binding):
    """('shader'|'clip', index) for a trigger binding, else None."""
    command = binding.get("command") or {}
    if command.get("type") == "TriggerShader":
        return ("shader", command["shader"])
    if command.get("type") == "TriggerClip":
        return ("clip", command["clip"])
    return None


class Remote:
    """ssh/scp to the Pi. Tries key authentication first, then asks for the password."""

    def __init__(self, parent):
        self.parent = parent
        self.password = None

    def _run(self, argv, stdin=None):
        env = dict(os.environ)
        helper = None
        if self.password is not None:
            # The helper only echoes an environment variable of this process tree.
            suffix = ".cmd" if os.name == "nt" else ".sh"
            body = "@echo %RPI_VJ_ASKPASS%\r\n" if os.name == "nt" else "#!/bin/sh\necho \"$RPI_VJ_ASKPASS\"\n"
            handle, helper = tempfile.mkstemp(suffix=suffix)
            os.write(handle, body.encode())
            os.close(handle)
            os.chmod(helper, 0o700)
            env.update(SSH_ASKPASS=helper, SSH_ASKPASS_REQUIRE="force", RPI_VJ_ASKPASS=self.password, DISPLAY=":0")
        try:
            options = ["-o", "StrictHostKeyChecking=accept-new", "-o", "ConnectTimeout=10"]
            if self.password is None:
                options += ["-o", "BatchMode=yes"]
            feed = {"input": stdin} if stdin is not None else {"stdin": subprocess.DEVNULL}
            return subprocess.run([argv[0], *options, *argv[1:]], env=env, capture_output=True, text=True,
                                  timeout=120, **feed)
        finally:
            if helper:
                os.unlink(helper)

    def run(self, argv, stdin=None):
        result = self._run(argv, stdin)
        if result.returncode == 255 or "Permission denied" in result.stderr:
            if self.password is None:
                self.password = simpledialog.askstring("Pi password", f"Password for {PI_HOST}:", show="*", parent=self.parent)
                if self.password is None:
                    raise RuntimeError("cancelled")
                result = self._run(argv, stdin)
            if result.returncode == 255 or "Permission denied" in result.stderr:
                self.password = None
                raise RuntimeError(result.stderr.strip() or "cannot reach the Pi")
        if result.returncode != 0:
            raise RuntimeError((result.stderr or result.stdout).strip()[-600:])
        return result.stdout

    def sudo(self, script):
        """Run a root shell script on the Pi; sudo reads the password from stdin."""
        if self.password is None:
            self.password = simpledialog.askstring("Pi password", f"Password for sudo on {PI_HOST}:", show="*", parent=self.parent)
            if self.password is None:
                raise RuntimeError("cancelled")
        return self.run(["ssh", PI_HOST, "sudo -S -p '' sh -c " + json.dumps(script)], stdin=self.password + "\n")


class Editor:
    def __init__(self, root, path):
        self.root, self.path = root, Path(path)
        self.show = json.loads(self.path.read_text(encoding="utf-8"))
        self.show.setdefault("pages", [])
        if not self.show["pages"]:
            self.show["pages"].append({"name": "PAGE 1", "bindings": []})
        self.page, self.dirty, self.images, self.drag = 0, False, {}, None
        self.remote = Remote(root)
        root.title(f"RPi-VJ deck layout - {self.path}")
        root.configure(bg=BG)
        root.protocol("WM_DELETE_WINDOW", self.close)

        bar = tk.Frame(root, bg=BG)
        bar.pack(fill="x", padx=10, pady=(10, 4))
        for label, action in (("Save", self.save), ("Sync to Pi", self.sync), ("Pull from Pi", self.pull),
                              ("Add page", self.add_page), ("Rename page", self.rename_page),
                              ("Delete page", self.delete_page)):
            tk.Button(bar, text=label, command=action, bg=PANEL, fg=TEXT, activebackground=ACCENT, relief="flat",
                      padx=10, pady=4).pack(side="left", padx=(0, 6))
        self.status = tk.Label(bar, text="", bg=BG, fg="#9aa0aa", anchor="e")
        self.status.pack(side="right", fill="x", expand=True)

        self.tabs = tk.Frame(root, bg=BG)
        self.tabs.pack(fill="x", padx=10, pady=4)
        body = tk.Frame(root, bg=BG)
        body.pack(fill="both", expand=True, padx=10, pady=(4, 10))
        width, height = COLS * (CELL + GAP) + GAP, ROWS * (CELL + GAP) + GAP
        self.grid = tk.Canvas(body, width=width, height=height, bg=BG, highlightthickness=0)
        self.grid.pack(side="left", anchor="n")
        side = tk.Frame(body, bg=BG)
        side.pack(side="left", fill="both", expand=True, padx=(12, 0))
        tk.Label(side, text="Not on the deck (drag onto a key)", bg=BG, fg="#9aa0aa", anchor="w").pack(fill="x")
        self.tray = tk.Listbox(side, bg=PANEL, fg=TEXT, selectbackground=ACCENT, relief="flat", width=34,
                               highlightthickness=0, activestyle="none", exportselection=False)
        self.tray.pack(fill="both", expand=True)
        self.tray_items = []
        for widget in (self.grid, self.tray):
            widget.bind("<ButtonPress-1>", self.press)
            widget.bind("<B1-Motion>", self.motion)
            widget.bind("<ButtonRelease-1>", self.release)
        self.grid.bind("<Double-Button-1>", self.rename_key)
        self.grid.bind("<Button-3>", self.toggle_canvas)
        self.ghost = None
        self.refresh()

    # --- model -----------------------------------------------------------------
    def bindings(self, page=None):
        return self.show["pages"][self.page if page is None else page]["bindings"]

    def at(self, key, page=None):
        return next((b for b in self.bindings(page) if b["key"] == key), None)

    def label_of(self, item):
        kind, index = item
        entries = self.show.get("shaders" if kind == "shader" else "clips", [])
        return entries[index]["label"] if index < len(entries) else "?"

    def unplaced(self):
        used = {item_of(b) for page in self.show["pages"] for b in page["bindings"]}
        items = [("shader", i) for i in range(len(self.show.get("shaders", [])))]
        items += [("clip", i) for i in range(len(self.show.get("clips", [])))]
        return [item for item in items if item not in used]

    def new_binding(self, item, key):
        kind, index = item
        command = {"type": "TriggerShader", "shader": index} if kind == "shader" else {"type": "TriggerClip", "clip": index}
        return {"key": key, "label": self.label_of(item), "command": command,
                "color": [25, 75, 120] if kind == "shader" else [35, 65, 95]}

    def free_key(self, page):
        taken = {b["key"] for b in self.bindings(page)} | set(SYSTEM_KEYS)
        return next((k for k in range(COLS * ROWS) if k not in taken), None)

    def changed(self, message):
        self.dirty = True
        self.refresh()
        self.say(message + " (not saved)")

    def say(self, message):
        self.status.config(text=message)
        self.root.update_idletasks()

    # --- drawing ---------------------------------------------------------------
    def picture(self, binding):
        item = item_of(binding)
        if not item:
            return None
        kind, index = item
        entries = self.show.get("shaders" if kind == "shader" else "clips", [])
        relative = entries[index].get("thumbnail") if index < len(entries) else None
        if not relative:
            return None
        if relative not in self.images:
            try:
                image = Image.open(self.path.parent / relative).convert("RGB").resize((CELL, CELL - 24), Image.LANCZOS)
                self.images[relative] = ImageTk.PhotoImage(image)
            except OSError:
                self.images[relative] = None
        return self.images[relative]

    def cell_xy(self, key):
        return GAP + (key % COLS) * (CELL + GAP), GAP + (key // COLS) * (CELL + GAP)

    def refresh(self):
        for child in self.tabs.winfo_children():
            child.destroy()
        self.tab_buttons = []
        for index, page in enumerate(self.show["pages"]):
            count = sum(1 for b in page["bindings"] if b["key"] not in SYSTEM_KEYS)
            button = tk.Label(self.tabs, text=f"{index + 1}  {page['name']}  ({count})", padx=10, pady=5,
                              bg=ACCENT if index == self.page else PANEL, fg=TEXT)
            button.pack(side="left", padx=(0, 4))
            button.bind("<Button-1>", lambda _event, i=index: self.go(i))
            self.tab_buttons.append(button)
        self.grid.delete("all")
        for key in range(COLS * ROWS):
            x, y = self.cell_xy(key)
            if key in SYSTEM_KEYS:
                self.grid.create_rectangle(x, y, x + CELL, y + CELL, fill=SYSTEM, outline="")
                self.grid.create_text(x + CELL / 2, y + CELL / 2, text=SYSTEM_KEYS[key], fill="#7d838f", font=("Segoe UI", 11, "bold"))
                continue
            binding = self.at(key)
            if not binding:
                self.grid.create_rectangle(x, y, x + CELL, y + CELL, fill=EMPTY, outline="")
                continue
            color = "#%02x%02x%02x" % tuple(binding.get("color", [35, 65, 95]))
            self.grid.create_rectangle(x, y, x + CELL, y + CELL, fill=color, outline="")
            picture = self.picture(binding)
            if picture:
                self.grid.create_image(x, y, image=picture, anchor="nw")
                self.grid.create_rectangle(x, y + CELL - 24, x + CELL, y + CELL, fill="#101216", outline="")
                self.grid.create_text(x + CELL / 2, y + CELL - 12, text=self.fit(binding["label"], 15), fill=TEXT, font=("Segoe UI", 9))
            item = item_of(binding)
            if item and item[0] == "shader" and self.show["shaders"][item[1]].get("infinite_canvas"):
                self.grid.create_oval(x + CELL - 22, y + 6, x + CELL - 6, y + 22, outline="white", width=3)
            else:
                self.grid.create_text(x + CELL / 2, y + CELL / 2, text=binding["label"], fill=TEXT, width=CELL - 10,
                                      font=("Segoe UI", 10, "bold"), justify="center")
        self.tray_items = self.unplaced()
        self.tray.delete(0, "end")
        for item in self.tray_items:
            self.tray.insert("end", f"{item[0]:<6} {self.label_of(item)}")

    @staticmethod
    def fit(text, limit):
        return text if len(text) <= limit else text[:limit - 1] + "…"

    def go(self, index):
        self.page = index
        self.refresh()

    # --- dragging --------------------------------------------------------------
    def key_at(self, x_root, y_root):
        x, y = x_root - self.grid.winfo_rootx(), y_root - self.grid.winfo_rooty()
        col, row = int((x - GAP / 2) // (CELL + GAP)), int((y - GAP / 2) // (CELL + GAP))
        return row * COLS + col if 0 <= col < COLS and 0 <= row < ROWS and x >= 0 and y >= 0 else None

    def over(self, widget, x_root, y_root):
        return (widget.winfo_rootx() <= x_root < widget.winfo_rootx() + widget.winfo_width()
                and widget.winfo_rooty() <= y_root < widget.winfo_rooty() + widget.winfo_height())

    def press(self, event):
        self.drag = None
        if event.widget is self.tray:
            index = self.tray.nearest(event.y)
            if 0 <= index < len(self.tray_items):
                self.drag = ("tray", self.tray_items[index], self.label_of(self.tray_items[index]))
        else:
            key = self.key_at(event.x_root, event.y_root)
            binding = self.at(key) if key is not None and key not in SYSTEM_KEYS else None
            if binding:
                self.drag = ("key", key, binding["label"])

    def motion(self, event):
        if not self.drag:
            return
        if self.ghost is None:
            self.ghost = tk.Toplevel(self.root)
            self.ghost.overrideredirect(True)
            self.ghost.attributes("-alpha", 0.8)
            tk.Label(self.ghost, text=self.fit(self.drag[2], 24), bg=ACCENT, fg="white", padx=8, pady=4).pack()
        self.ghost.geometry(f"+{event.x_root + 12}+{event.y_root + 12}")

    def release(self, event):
        drag, self.drag = self.drag, None
        if self.ghost is not None:
            self.ghost.destroy()
            self.ghost = None
            moved = True
        else:
            moved = False
        if not drag or not moved:
            return
        source, what, label = drag
        for index, button in enumerate(self.tab_buttons):
            if self.over(button, event.x_root, event.y_root):
                return self.drop_on_page(source, what, label, index)
        if self.over(self.tray, event.x_root, event.y_root):
            if source == "key":
                self.bindings().remove(self.at(what))
                self.changed(f"took {label} off the deck")
            return
        key = self.key_at(event.x_root, event.y_root)
        if key is None or key in SYSTEM_KEYS or not self.over(self.grid, event.x_root, event.y_root):
            return
        target = self.at(key)
        if source == "key":
            if key == what:
                return
            moving = self.at(what)
            if target:
                target["key"] = what
            moving["key"] = key
            self.changed(f"moved {label}")
        else:
            if target:
                self.bindings().remove(target)
            self.bindings().append(self.new_binding(what, key))
            self.changed(f"placed {label}")

    def drop_on_page(self, source, what, label, index):
        key = self.free_key(index)
        if key is None:
            return self.say(f"page {index + 1} is full")
        if source == "key":
            if index == self.page:
                return
            moving = self.at(what)
            self.bindings().remove(moving)
            moving["key"] = key
            self.bindings(index).append(moving)
        else:
            self.bindings(index).append(self.new_binding(what, key))
        self.changed(f"moved {label} to page {index + 1}")

    # --- pages and captions ------------------------------------------------------
    def rename_key(self, event):
        key = self.key_at(event.x_root, event.y_root)
        binding = self.at(key) if key is not None else None
        if binding:
            text = simpledialog.askstring("Caption", "Key caption:", initialvalue=binding["label"], parent=self.root)
            if text and text.strip():
                binding["label"] = text.strip()
                self.changed("renamed key")

    def toggle_canvas(self, event):
        key = self.key_at(event.x_root, event.y_root)
        item = item_of(self.at(key) or {}) if key is not None else None
        if not item or item[0] != "shader":
            return
        shader = self.show["shaders"][item[1]]
        if shader.get("infinite_canvas"):
            del shader["infinite_canvas"]
            self.changed(f"{shader['label']}: tiled")
        else:
            shader["infinite_canvas"] = True
            self.changed(f"{shader['label']}: endless canvas")

    def add_page(self):
        if len(self.show["pages"]) >= MAX_PAGES:
            return self.say("the engine allows 128 pages")
        self.show["pages"].insert(self.page + 1, {"name": f"PAGE {len(self.show['pages']) + 1}", "bindings": []})
        self.page += 1
        self.changed("added a page")

    def rename_page(self):
        page = self.show["pages"][self.page]
        text = simpledialog.askstring("Page name", "Page name:", initialvalue=page["name"], parent=self.root)
        if text and text.strip():
            page["name"] = text.strip()
            self.changed("renamed page")

    def delete_page(self):
        if len(self.show["pages"]) == 1:
            return self.say("a show needs at least one page")
        count = len(self.bindings())
        if count and not messagebox.askyesno("Delete page", f"Delete this page? Its {count} keys go back to the tray.", parent=self.root):
            return
        del self.show["pages"][self.page]
        self.page = min(self.page, len(self.show["pages"]) - 1)
        self.changed("deleted a page")

    # --- files and the Pi ----------------------------------------------------------
    def tidy(self):
        show = copy.deepcopy(self.show)
        for page in show["pages"]:
            page["bindings"] = sorted((b for b in page["bindings"] if b["key"] not in SYSTEM_KEYS), key=lambda b: b["key"])
        return show

    def save(self):
        if self.path.exists():
            self.path.with_suffix(".json.bak").write_bytes(self.path.read_bytes())
        self.path.write_text(json.dumps(self.tidy(), indent=1), encoding="utf-8")
        self.dirty = False
        self.say(f"saved {self.path.name}")
        return True

    def sync(self):
        if not messagebox.askyesno("Sync to Pi", "Upload this layout and restart the show on the Pi?\n"
                                   "The output goes black for a few seconds. The Pi keeps its previous show as show.json.before-sync.",
                                   parent=self.root):
            return
        self.save()
        try:
            self.say("uploading...")
            self.remote.run(["scp", str(self.path), f"{PI_HOST}:/tmp/deck-layout-show.json"])
            self.say("installing and restarting...")
            out = self.remote.sudo(
                "set -e; test ! -e /home/rpi/DEPLOY_IN_PROGRESS; "
                "python3 -c 'import json,sys; json.load(open(sys.argv[1]))' /tmp/deck-layout-show.json; "
                f"cp -a {PI_SHOW} {PI_SHOW}.before-sync; "
                "systemctl stop rpi-vj-controller rpi-vj-native; "
                f"install -m 644 -o vj -g vj /tmp/deck-layout-show.json {PI_SHOW}; "
                "systemctl start rpi-vj-native rpi-vj-controller; sleep 6; "
                "systemctl is-active rpi-vj-native rpi-vj-controller")
            if out.split() == ["active", "active"]:
                self.say("synced: the Pi is running the new layout")
            else:
                self.say("uploaded, but a service is not active: " + " ".join(out.split()))
        except (RuntimeError, subprocess.TimeoutExpired, OSError) as error:
            self.say("sync failed")
            messagebox.showerror("Sync failed", str(error), parent=self.root)

    def pull(self):
        if self.dirty and not messagebox.askyesno("Pull from Pi", "Discard unsaved changes and load the Pi's layout?", parent=self.root):
            return
        try:
            self.say("downloading...")
            text = self.remote.run(["ssh", PI_HOST, f"cat {PI_SHOW}"])
            show = json.loads(text)
        except (RuntimeError, subprocess.TimeoutExpired, OSError, ValueError) as error:
            self.say("pull failed")
            return messagebox.showerror("Pull failed", str(error), parent=self.root)
        self.show, self.page = show, 0
        self.images.clear()
        self.save()
        self.refresh()
        self.say("loaded the Pi's layout")

    def close(self):
        if self.dirty:
            answer = messagebox.askyesnocancel("Unsaved changes", "Save before closing?", parent=self.root)
            if answer is None or (answer and not self.save()):
                return
        self.root.destroy()


def main():
    path = Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_SHOW
    if not path.is_file():
        raise SystemExit(f"show not found: {path}")
    root = tk.Tk()
    Editor(root, path)
    root.mainloop()


if __name__ == "__main__":
    main()
