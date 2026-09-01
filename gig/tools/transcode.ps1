# Transcode a clip library for the Pi, on the 3090 (ffmpeg must be on PATH).
#
# 2026-08-26: The shipped library is 28 clips at 1280x720 H.264 30fps and is CORRECT
# as built — re-encoding it to 1080p60 would make Pi playback harder, not easier
# (the Pi has no H.264 hardware decode, so every pixel is software-decoded).
# Default resolution is 720p @ 30fps.
#
#   .\gig\tools\transcode.ps1 -In "D:\VJ\raw" -Out ".\gig\media"
#   .\gig\tools\transcode.ps1 -In "D:\VJ\raw" -Out ".\gig\media" -Codec h264
#
# Pi 5 has NO H.264 hardware decode (HEVC only), but Chromium's HEVC support on
# Pi OS is unverified. Default 'both' writes hevc/ and h264/ so you can test the
# hardware path and still have a guaranteed fallback.
param(
  [Parameter(Mandatory=$true)][string]$In,
  [string]$Out = ".\gig\media",
  [ValidateSet("hevc","h264","both")][string]$Codec = "both",
  [int]$Height = 720,          # 720 is the safe default for software H.264
  [int]$Fps = 30,
  [int]$Crf = 21
)
if (-not (Get-Command ffmpeg -ErrorAction SilentlyContinue)) {
  Write-Host "!! ffmpeg not on PATH. Install it, or use: winget install Gyan.FFmpeg" -ForegroundColor Red; exit 1
}
$src = Get-ChildItem -Path $In -Recurse -Include *.mp4,*.mov,*.mkv,*.avi,*.m4v,*.webm,*.mxf
if (-not $src) { Write-Host "!! no video files under $In" -ForegroundColor Red; exit 1 }
Write-Host "== $($src.Count) source clips -> $Out" -ForegroundColor Cyan

$targets = @()
if ($Codec -eq "both") { $targets = @("hevc","h264") } else { $targets = @($Codec) }
foreach ($t in $targets) { New-Item -ItemType Directory -Force -Path (Join-Path $Out $t) | Out-Null }
New-Item -ItemType Directory -Force -Path (Join-Path $Out "thumbs") | Out-Null

$manifest = @()
$i = 0
foreach ($f in $src) {
  $i++
  $name = [System.IO.Path]::GetFileNameWithoutExtension($f.Name) -replace '[^A-Za-z0-9_-]','_'
  Write-Host ("[{0}/{1}] {2}" -f $i, $src.Count, $name) -ForegroundColor Yellow

  # -g/-keyint_min/-sc_threshold force a CLOSED GOP of 15 so loop points are
  # clean; -an strips audio entirely (clip audio must never reach the PA).
  $common = @("-y","-hide_banner","-loglevel","error","-i",$f.FullName,
              "-vf","scale=-2:$Height,fps=$Fps","-an",
              "-g","15","-keyint_min","15","-sc_threshold","0",
              "-pix_fmt","yuv420p","-movflags","+faststart")

  foreach ($t in $targets) {
    $dst = Join-Path (Join-Path $Out $t) "$name.mp4"
    if (Test-Path $dst) { Write-Host "    skip $t (exists)"; continue }
    if ($t -eq "hevc") {
      ffmpeg @common -c:v libx265 -preset medium -crf $Crf -tag:v hvc1 $dst
    } else {
      ffmpeg @common -c:v libx264 -preset medium -crf $Crf $dst
    }
    if ($LASTEXITCODE -ne 0) { Write-Host "    !! $t FAILED" -ForegroundColor Red }
  }

  # 96x96 thumbnail for the Stream Deck key icon
  $thumb = Join-Path (Join-Path $Out "thumbs") "$name.jpg"
  if (-not (Test-Path $thumb)) {
    ffmpeg -y -hide_banner -loglevel error -ss 1 -i $f.FullName -frames:v 1 `
      -vf "scale=96:96:force_original_aspect_ratio=increase,crop=96:96" -q:v 4 $thumb
  }
  $manifest += [pscustomobject]@{ name = $name; thumb = "media/thumbs/$name.jpg" }
}

$manifest | ConvertTo-Json -Depth 3 | Set-Content (Join-Path $Out "manifest.json")
Write-Host "`n== done. sizes:" -ForegroundColor Green
foreach ($t in $targets) {
  $p = Join-Path $Out $t
  $mb = [math]::Round(((Get-ChildItem $p -File | Measure-Object Length -Sum).Sum / 1MB), 1)
  Write-Host ("   {0,-5} {1,6} MB" -f $t, $mb)
}
Write-Host "`nNext:  .\gig\tools\sync.ps1     (copies gig/ including media/)" -ForegroundColor Cyan
