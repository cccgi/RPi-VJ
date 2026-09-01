# Sync gig/ to the Pi, restore exec bits, restart the kiosk.
#   .\gig\tools\sync.ps1                 # code only -- media/ is SKIPPED
#   .\gig\tools\sync.ps1 -WithMedia      # include media/ (slow: clips + thumbs)
#
# media/ is excluded by default because the clip library is large and rarely
# changes, while code changes constantly. Copying it every time wasted minutes.
param(
  [string]$Target = "rpi@rpi-vj.local",
  [int]$Retries = 3,
  [switch]$WithMedia,
  # Copy ONE media subfolder (e.g. -MediaDir 4k) instead of the whole tree.
  # -WithMedia re-pushes every clip you already have on the Pi, so adding a
  # 2.4GB folder meant shipping 5.3GB of unchanged 720p files first. This
  # copies only what is missing, and verifies it afterwards.
  [string]$MediaDir = ""
)

# PowerShell does not escape @ with a backslash. Normalize the form that can
# result from copying Markdown-escaped commands (rpi\@host) instead of letting
# SSH attempt to authenticate as the literal user "rpi\".
if ($Target -match '\\@') {
  $originalTarget = $Target
  $Target = $Target -replace '\\@', '@'
  Write-Host "== normalized SSH target: $originalTarget -> $Target" -ForegroundColor DarkYellow
}

# mDNS on Windows commonly returns the Pi's link-local IPv6 address before its
# usable IPv4 address.  OpenSSH can then sit on that route while this script
# appears frozen (the old script also discarded SSH output and made SCP quiet).
# Force IPv4 and bound every connection attempt so a broken route fails visibly.
$SshOptions = @(
  "-4",
  "-o", "BatchMode=yes",
  "-o", "ConnectTimeout=10",
  "-o", "ConnectionAttempts=1",
  "-o", "ServerAliveInterval=5",
  "-o", "ServerAliveCountMax=2"
)

function Invoke-WithRetry {
  param([scriptblock]$Action, [string]$What)
  for ($i = 1; $i -le $Retries; $i++) {
    & $Action
    if ($LASTEXITCODE -eq 0) { return $true }
    if ($i -lt $Retries) {
      Write-Host "   $What failed (attempt $i/$Retries), retrying..." -ForegroundColor DarkYellow
      Start-Sleep -Seconds 2
    }
  }
  return $false
}

Write-Host "== checking SSH and preparing destination -> $Target" -ForegroundColor Cyan
if (-not (Invoke-WithRetry { ssh @SshOptions $Target "mkdir -p ~/RPi-VJ/gig" } "connect")) {
  Write-Host "!! SSH FAILED - try: ssh -4 $Target hostname" -ForegroundColor Red
  exit 1
}

if ($MediaDir -ne "") {
  $local = "gig\media\$MediaDir"
  if (-not (Test-Path $local)) { Write-Host "!! $local not found" -ForegroundColor Red; exit 1 }

  $localFiles = @(Get-ChildItem -Path $local -File)
  $localCount = $localFiles.Count
  $localMB = [int](($localFiles | Measure-Object -Property Length -Sum).Sum / 1MB)
  Write-Host "== copying media/$MediaDir : $localCount files, $localMB MB" -ForegroundColor Cyan
  Write-Host "   (interrupted runs resume by skipping files already present at the same size)" -ForegroundColor DarkGray

  if (-not (Invoke-WithRetry { ssh @SshOptions $Target "mkdir -p ~/RPi-VJ/gig/media/$MediaDir" } "prepare-media")) {
    Write-Host "!! could not prepare the remote media directory" -ForegroundColor Red
    exit 1
  }

  # File by file, so an interrupted run resumes instead of starting over.
  $n = 0
  foreach ($f in $localFiles) {
    $n++
    $remoteSize = ssh @SshOptions $Target "stat -c%s '~/RPi-VJ/gig/media/$MediaDir/$($f.Name)' 2>/dev/null || stat -c%s ~/RPi-VJ/gig/media/$MediaDir/'$($f.Name)' 2>/dev/null || echo 0"
    if ("$remoteSize".Trim() -eq "$($f.Length)") {
      Write-Host ("   [{0}/{1}] {2}  already there" -f $n, $localCount, $f.Name.Substring(0, [Math]::Min(44, $f.Name.Length))) -ForegroundColor DarkGray
      continue
    }
    Write-Host ("   [{0}/{1}] {2}  {3} MB ..." -f $n, $localCount, $f.Name.Substring(0, [Math]::Min(44, $f.Name.Length)), [int]($f.Length / 1MB))
    scp @SshOptions $f.FullName "${Target}:~/RPi-VJ/gig/media/$MediaDir/"
    if ($LASTEXITCODE -ne 0) { Write-Host "   !! failed, re-run to resume" -ForegroundColor Red }
  }

  $remote = ssh @SshOptions $Target "ls ~/RPi-VJ/gig/media/$MediaDir/*.mp4 2>/dev/null | wc -l"
  Write-Host "== on the Pi: $($remote.Trim()) files (expected $localCount)" -ForegroundColor Green
  Write-Host "== restarting kiosk" -ForegroundColor Cyan
  ssh @SshOptions $Target "systemctl --user restart vj-kiosk"
  exit 0
}

if ($WithMedia) {
  Write-Host "== syncing gig/ INCLUDING media -> $Target" -ForegroundColor Cyan
  $ok = Invoke-WithRetry { scp @SshOptions -r gig "${Target}:~/RPi-VJ/" } "scp"
} else {
  $items = Get-ChildItem -Path gig -Exclude media | ForEach-Object { $_.FullName }
  Write-Host "== syncing gig/ (media skipped; -WithMedia to include) -> $Target" -ForegroundColor Cyan
  $ok = Invoke-WithRetry { scp @SshOptions -r @items "${Target}:~/RPi-VJ/gig/" } "scp"
}
if (-not $ok) {
  Write-Host "!! SCP FAILED - nothing was copied. Do not test; fix the connection." -ForegroundColor Red
  exit 1
}
Write-Host "== files copied" -ForegroundColor Green

if (-not (Invoke-WithRetry { ssh @SshOptions $Target "chmod +x ~/RPi-VJ/gig/deploy/*.sh ~/RPi-VJ/gig/tools/*.sh 2>/dev/null" } "chmod")) {
  Write-Host "!! failed to restore executable permissions" -ForegroundColor Red
  exit 1
}

# VERIFY the copy actually landed -- EVERY file, not just index.html.
#
# Verifying one file was not enough: a sync reported success with a matching
# index.html while the Pi still held old shaders and an old src/isf.js, and
# the resulting compile errors looked exactly like broken code. Hash the whole
# tree and name the files that differ, so a stale Pi can never again be
# mistaken for a bad fix.
Write-Host "== verifying every file (this is the check that catches stale code)" -ForegroundColor Cyan

$localFiles = Get-ChildItem -Path gig -Recurse -File |
  Where-Object { $_.FullName -notmatch '[\\/]media[\\/]' -and $_.FullName -notmatch '__pycache__' }

$localMap = @{}
$gigRoot = (Resolve-Path "gig").Path
foreach ($f in $localFiles) {
  $rel = $f.FullName.Substring($gigRoot.Length).TrimStart('\','/').Replace('\','/')
  $localMap[$rel] = (Get-FileHash -Algorithm SHA256 $f.FullName).Hash.ToLower()
}

$remoteRaw = ssh @SshOptions $Target "cd ~/RPi-VJ/gig 2>/dev/null && find . -type f -not -path './media/*' -not -path '*__pycache__*' -print0 | xargs -0 sha256sum 2>/dev/null"
if ($LASTEXITCODE -ne 0) {
  Write-Host "!! VERIFY FAILED - could not read remote hashes" -ForegroundColor Red
  exit 1
}
$remoteMap = @{}
foreach ($line in ($remoteRaw -split "`n")) {
  if ($line -match '^([0-9a-f]{64})\s+\.\/(.+)$') { $remoteMap[$Matches[2]] = $Matches[1] }
}

$bad = @()
foreach ($rel in $localMap.Keys) {
  if (-not $remoteMap.ContainsKey($rel))      { $bad += "MISSING  $rel" }
  elseif ($remoteMap[$rel] -ne $localMap[$rel]) { $bad += "STALE    $rel" }
}

if ($bad.Count -gt 0) {
  Write-Host "!! VERIFY FAILED - $($bad.Count) file(s) on the Pi do NOT match your copy:" -ForegroundColor Red
  $bad | Sort-Object | ForEach-Object { Write-Host "   $_" -ForegroundColor Red }
  Write-Host "   The Pi is running OLD code. Do not test until this is clean." -ForegroundColor Red
  exit 1
}
Write-Host "== verified: all $($localMap.Count) files on the Pi match your copy" -ForegroundColor Green

Write-Host "== restarting server + kiosk" -ForegroundColor Cyan
# vj-server runs collect_server.py; python does NOT reload edited files.
# Restarting only the kiosk leaves stale server code running (404 on /heartbeat).
$restartCommand = 'systemctl --user restart --no-block vj-server vj-kiosk && printf "   restart queued"; for i in $(seq 1 20); do if systemctl --user is-active --quiet vj-server vj-kiosk && curl -sf http://127.0.0.1:8080/gig/ >/dev/null; then echo; echo "   services ready"; exit 0; fi; printf "."; sleep 1; done; echo; echo "restart readiness timed out" >&2; systemctl --user --no-pager --lines=8 status vj-server vj-kiosk >&2; exit 1'
if (-not (Invoke-WithRetry { ssh @SshOptions $Target $restartCommand } "restart-and-health")) {
  Write-Host "!! RESTART FAILED - inspect: ssh -4 $Target 'systemctl --user status vj-server vj-kiosk'" -ForegroundColor Red
  exit 1
}
Write-Host "== kiosk restarted, running current code" -ForegroundColor Green
