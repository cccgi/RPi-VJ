# Music page (mpv/libmpv)

Music playback is intentionally outside Chromium. The existing localhost
server owns one audio-only `mpv` process (which uses libmpv internally) and
controls it through a Unix JSON IPC socket. This keeps audio decoding out of
the WebGL compositor and prevents multiple songs from consuming memory.

Place up to 24 files directly in `gig/media/music/`. Supported extensions are
MP3, FLAC, WAV, M4A, AAC, OGG, and Opus. Then rebuild the page:

```powershell
python gig/tools/build-music-bank.py
```

The page uses keys 1–24 for songs. Physical keys 25/26 remain page navigation.
Keys 27–30 seek forward by 30, 60, 120, and 180 seconds respectively. Key 31
is Play/Pause, and key 32 is Mute. Seeking uses mpv's `relative+exact` mode in
the same decoder; it does not reload the track. Selecting a song uses mpv's
`loadfile ... replace`, so exactly one music decoder is active.

Deploy code and the library separately:

```powershell
.\gig\tools\sync.ps1 -Target "rpi@rpi"
.\gig\tools\sync.ps1 -Target "rpi@rpi" -MediaDir music
```

On a fresh Pi, install mpv once:

```bash
sudo apt-get update && sudo apt-get install -y mpv
```

Health checks:

```bash
curl -s http://127.0.0.1:8080/api/music/status
journalctl --user -u vj-server -n 50 --no-pager
```
