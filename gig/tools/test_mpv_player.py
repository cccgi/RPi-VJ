#!/usr/bin/env python3
"""No-mpv unit checks for path containment and command sequencing."""
import tempfile
from pathlib import Path
from mpv_player import MpvError, MpvPlayer


class FakePlayer(MpvPlayer):
    def __init__(self, root):
        super().__init__(root, executable=__file__)
        self.commands = []

    def _command(self, *command):
        self.commands.append(command)
        if command[:2] == ("get_property", "path"):
            return str(Path(self.music_root) / "song.mp3")
        if command[:2] == ("get_property", "media-title"):
            return "Song"
        return False

    def status(self):
        return {"playing": True, "paused": False, "muted": False}


with tempfile.TemporaryDirectory() as td:
    root = Path(td) / "music"
    root.mkdir()
    (root / "song.mp3").write_bytes(b"test")
    player = FakePlayer(root)
    state = player.play("media/music/song.mp3")
    assert state["playing"] is True
    assert player.commands[0][0] == "loadfile"
    assert Path(player.commands[0][1]) == root / "song.mp3"
    assert player.commands[0][2] == "replace"
    assert player.commands[1] == ("set_property", "pause", False)
    assert player.seek_forward(120) == {"ok": True, "seeked": 120}
    assert player.commands[-1] == ("seek", 120, "relative+exact")
    try:
        player.seek_forward(45)
        raise AssertionError("unsupported seek interval was accepted")
    except MpvError:
        pass
    try:
        player._safe_path("../outside.mp3")
        raise AssertionError("path traversal was accepted")
    except MpvError:
        pass

print("PASS: mpv player containment and replace-only playback")
