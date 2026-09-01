param(
  [string]$SourceDir = "gig\media\50",
  [int]$Fps = 60,
  [int]$LoopSeconds = 16,
  [switch]$Force
)

$ErrorActionPreference = "Stop"
$source = (Resolve-Path $SourceDir).Path
$outDir = Join-Path $source "loops"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

$slides = @(
  @{ File="AdobeStock_1241748710.jpeg"; Name="Chocolate Cake"; Fx=0.72; Fy=0.60 },
  @{ File="AdobeStock_1431780010.jpeg"; Name="Golden Party";   Fx=0.68; Fy=0.52 },
  @{ File="AdobeStock_1510561473.jpeg"; Name="Black & Gold";  Fx=0.30; Fy=0.50 },
  @{ File="AdobeStock_1592137293.jpeg"; Name="Candle Cake";   Fx=0.52; Fy=0.48 },
  @{ File="AdobeStock_2025938347.jpeg"; Name="Celebrate 50";  Fx=0.50; Fy=0.48 },
  @{ File="AdobeStock_2085997772.jpeg"; Name="Gold 50";       Fx=0.50; Fy=0.48 },
  @{ File="AdobeStock_697848202.jpeg";  Name="Golden Glow";   Fx=0.28; Fy=0.52 },
  @{ File="AdobeStock_864860919.jpeg";  Name="Birthday Cake"; Fx=0.50; Fy=0.50 },
  @{ File="AdobeStock_875807315.jpeg";  Name="Color Party";   Fx=0.50; Fy=0.48 },
  @{ File="AdobeStock_1981561688.mov";  Name="Gold Greeting"; Kind="video" }
)

function Invoke-Ffmpeg {
  param([string[]]$Arguments, [string]$What)
  Write-Host "== $What" -ForegroundColor Cyan
  & ffmpeg @Arguments
  if ($LASTEXITCODE -ne 0) { throw "ffmpeg failed while $What (exit $LASTEXITCODE)" }
}

$frames = $Fps * $LoopSeconds
$lastFrame = $frames - 1
$loopFiles = @()

for ($i = 0; $i -lt $slides.Count; $i++) {
  $slide = $slides[$i]
  $input = Join-Path $source $slide.File
  if (-not (Test-Path -LiteralPath $input)) { throw "Missing source image: $input" }
  $output = Join-Path $outDir ("50th-{0:d2}.mp4" -f ($i + 1))
  $loopFiles += $output
  if ((Test-Path -LiteralPath $output) -and -not $Force) {
    Write-Host "== already rendered: $(Split-Path $output -Leaf)" -ForegroundColor DarkGray
    continue
  }

  if ($slide.Kind -eq "video") {
    # Rotate the five-second stock animation at t=1, then dissolve its original
    # ending into its beginning. Cutting immediately after that dissolve makes
    # the last composition meet the first without a hard bokeh/text jump.
    $videoFilter = "[0:v]split=2[a][b];" +
      "[a]trim=start=1:end=5,setpts=PTS-STARTPTS[a1];" +
      "[b]trim=start=0:end=2,setpts=PTS-STARTPTS[b1];" +
      "[a1][b1]xfade=transition=fade:duration=1:offset=3," +
      "trim=duration=4,setpts=PTS-STARTPTS," +
      "minterpolate=fps=${Fps}:mi_mode=blend," +
      "scale=1920:1080:in_range=tv:out_range=tv,format=yuv420p[out]"
    Invoke-Ffmpeg @(
      "-y", "-hide_banner", "-loglevel", "warning", "-stats", "-i", $input,
      "-filter_complex", $videoFilter, "-map", "[out]", "-an",
      "-c:v", "libx264", "-preset", "medium", "-crf", "18",
      "-profile:v", "high", "-level", "4.2", "-pix_fmt", "yuv420p",
      "-r", "$Fps", "-g", "$($Fps * 2)", "-keyint_min", "$($Fps * 2)",
      "-sc_threshold", "0", "-movflags", "+faststart", $output
    ) "rendering $($i + 1)/$($slides.Count): $($slide.Name) seamless video loop"
    continue
  }

  # Begin and end at z=1, which shows the entire image edge-to-edge. Render the
  # move on an 8K working canvas to a 4K intermediate, then Lanczos-downsample.
  # zoompan quantizes crop coordinates to integers; supersampling turns each
  # integer step into a subpixel shift at 1080p instead of a visible jiggle.
  $fx = [string]::Format([Globalization.CultureInfo]::InvariantCulture, "{0:0.00}", $slide.Fx)
  $fy = [string]::Format([Globalization.CultureInfo]::InvariantCulture, "{0:0.00}", $slide.Fy)
  $filter = "scale=7680:4320:force_original_aspect_ratio=increase:in_range=full:out_range=tv,crop=7680:4320," +
    "zoompan=z='1+0.10*pow(sin(PI*on/$lastFrame),2)':" +
    "x='(iw-iw/zoom)*$fx':" +
    "y='(ih-ih/zoom)*$fy':" +
    "d=${frames}:s=3840x2160:fps=$Fps," +
    "scale=1920:1080:flags=lanczos,format=yuv420p"

  Invoke-Ffmpeg @(
    "-y", "-hide_banner", "-loglevel", "warning", "-stats",
    "-i", $input, "-vf", $filter,
    "-frames:v", "$frames", "-an", "-c:v", "libx264", "-preset", "medium",
    "-crf", "18", "-profile:v", "high", "-level", "4.2",
    "-g", "$($Fps * 2)", "-keyint_min", "$($Fps * 2)", "-sc_threshold", "0",
    "-movflags", "+faststart", $output
  ) "rendering $($i + 1)/$($slides.Count): $($slide.Name)"
}

$master = Join-Path $outDir "50th-master-slideshow.mp4"
if (-not (Test-Path -LiteralPath $master) -or $Force) {
  $args = @("-y", "-hide_banner", "-loglevel", "warning", "-stats")
  $segmentSeconds = [int]($LoopSeconds / 2)
  $transitionSeconds = 1
  $stepSeconds = $segmentSeconds - $transitionSeconds
  # Use the slow zoom-in half of every source, then the zoom-out half of slide
  # 1 after the final dissolve to return the master to its opening full frame.
  foreach ($file in $loopFiles) {
    $duration = [double](ffprobe -v error -show_entries format=duration -of default=nw=1:nk=1 $file)
    if ($duration -lt $segmentSeconds) {
      $repeats = [Math]::Ceiling($segmentSeconds / $duration) - 1
      $args += @("-stream_loop", "$repeats", "-t", "$segmentSeconds", "-i", $file)
    } else {
      $args += @("-t", "$segmentSeconds", "-i", $file)
    }
  }
  # The closing repeat uses the second half of slide 1's cycle, so
  # the final frame returns to the exact framing seen at the master start.
  $args += @("-ss", "$segmentSeconds", "-t", "$segmentSeconds", "-i", $loopFiles[0])

  $parts = @()
  $previous = "[0:v]"
  for ($i = 1; $i -le $slides.Count; $i++) {
    $out = "[x$i]"
    $offset = $i * $stepSeconds
    $parts += "$previous[$i`:v]xfade=transition=fade:duration=${transitionSeconds}:offset=$offset$out"
    $previous = $out
  }
  $filterComplex = $parts -join ";"
  $args += @(
    "-filter_complex", $filterComplex, "-map", $previous,
    "-an", "-c:v", "libx264", "-preset", "medium", "-crf", "18",
    "-profile:v", "high", "-level", "4.2", "-pix_fmt", "yuv420p",
    "-r", "$Fps", "-g", "$($Fps * 2)", "-keyint_min", "$($Fps * 2)",
    "-sc_threshold", "0", "-movflags", "+faststart", $master
  )
  $masterSeconds = ($slides.Count * $stepSeconds) + $segmentSeconds
  Invoke-Ffmpeg $args "rendering seamless $masterSeconds-second master slideshow"
}

Write-Host "== finished: $outDir" -ForegroundColor Green
