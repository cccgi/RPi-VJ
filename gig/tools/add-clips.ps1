# One command to take a folder of new source video all the way to playable
# pages on the Stream Deck. Run from the repo root on Windows.
#
#   .\gig\tools\add-clips.ps1 -Source "E:\__VJ\New Pack" -Name neon
#
# Phases (it runs all of them unless you pick one):
#   1 SCAN   find the seamless loop point of every clip        -> <Name>-loops.json
#   2 BUILD  cut at those points, scale, write to media/<Name>
#   3 BANK   add Stream Deck page(s) for them
#   4 PUSH   copy only that media folder to the Pi and restart
#
# Run -Scan on its own first when you want to read the table before committing
# to an encode. The scan modifies nothing.
param(
  [Parameter(Mandatory=$true)][string]$Source,
  [Parameter(Mandatory=$true)][string]$Name,      # folder + bank name, e.g. neon
  [int]$Height = 1080,                            # 0 = keep source resolution
  [double]$MinLength = 15,                        # promote short loops to whole multiples
  [string]$Bank = "",                             # defaults to $Name uppercased
  [int]$MaxLoop = 120,                            # longest loop to look for (s)
  [switch]$Scan, [switch]$Build, [switch]$BankOnly, [switch]$Push,
  [switch]$IncludeUncertain,                      # also cut clips flagged uncertain
  [switch]$Clean,                                 # delete AppleDouble/.DS_Store junk in -Source
  [switch]$CropQuarter,                           # crop 4-way symmetric clips to 1/4 (top-left)
  [switch]$ForceCropQuarter,                      # curated folder: crop every clip, even detector false-negatives
  [string]$Target = "rpi@rpi-vj.local"
)

$ErrorActionPreference = "Stop"
if (-not (Test-Path "gig\tools\find-loops.py")) {
  Write-Host "!! run this from the repo root (C:\Users\Admin\Projects\rpi-vj)" -ForegroundColor Red; exit 1
}
if (-not (Test-Path $Source)) { Write-Host "!! source not found: $Source" -ForegroundColor Red; exit 1 }
if ($Bank -eq "") { $Bank = $Name.ToUpper() }

$all = -not ($Scan -or $Build -or $BankOnly -or $Push)
$loops = "$Name-loops.json"
$dest  = "gig\media\$Name"

if ($Scan -or $all) {
  Write-Host "`n=== 1/4 SCAN  finding seamless loop points and symmetry ===" -ForegroundColor Cyan
  Write-Host "    nothing is modified by this step" -ForegroundColor DarkGray
  $scanArgs = @("gig\tools\find-loops.py", "--dir", $Source, "--out", $loops, "--max", $MaxLoop)
  if ($Clean) { $scanArgs += "--clean" }
  python -u @scanArgs
  if ($LASTEXITCODE -ne 0) { Write-Host "!! scan failed" -ForegroundColor Red; exit 1 }
  Write-Host "`n    Read the table above before continuing." -ForegroundColor Yellow
  Write-Host "    seam < 1.5 wraps invisibly. 'uncertain' clips are skipped" -ForegroundColor Yellow
  Write-Host "    unless you pass -IncludeUncertain." -ForegroundColor Yellow
  if ($Scan) { Write-Host "`n== scan only. Re-run without -Scan to build." -ForegroundColor Green; exit 0 }
}

if ($Build -or $all) {
  Write-Host "`n=== 2/4 BUILD  cutting and encoding ===" -ForegroundColor Cyan
  $args = @("gig\tools\loop-cut.py", "--src", $Source, "--dst", $dest,
            "--loops", $loops, "--min-length", $MinLength)
  if ($Height -gt 0) { $args += @("--height", $Height) }
  if ($IncludeUncertain) { $args += "--all" }
  if ($CropQuarter) { $args += "--crop-quarter" }
  if ($ForceCropQuarter) { $args += "--force-crop-quarter" }
  python @args
  if ($LASTEXITCODE -ne 0) { Write-Host "!! build failed" -ForegroundColor Red; exit 1 }
}

if ($BankOnly -or $all) {
  Write-Host "`n=== 3/4 BANK  adding Stream Deck page(s) ===" -ForegroundColor Cyan
  $bankArgs = @("gig\tools\build-clip-bank.py", "--dir", "media/$Name", "--bank", "$Bank")
  if ($CropQuarter) { $bankArgs += "--quad" }
  python @bankArgs
  if ($LASTEXITCODE -ne 0) { Write-Host "!! bank build failed" -ForegroundColor Red; exit 1 }
}

if ($Push -or $all) {
  Write-Host "`n=== 4/4 PUSH  copying to the Pi ===" -ForegroundColor Cyan
  .\gig\tools\sync.ps1 -Target $Target
  .\gig\tools\sync.ps1 -Target $Target -MediaDir $Name
}

Write-Host "`n== done. New pages are at the end of the bank list." -ForegroundColor Green
Write-Host "== to remove them again:" -ForegroundColor DarkGray
Write-Host "   python gig\tools\build-clip-bank.py --bank `"$Bank`" --remove" -ForegroundColor DarkGray
