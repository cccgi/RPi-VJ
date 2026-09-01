# Restart the kiosk without copying anything -- for when you have edited files
# directly over the SMB share. Fails loudly rather than silently no-opping.
param([string]$Target = "rpi@rpi-vj.local", [int]$Retries = 3)
for ($i = 1; $i -le $Retries; $i++) {
  ssh $Target "systemctl --user restart vj-kiosk"
  if ($LASTEXITCODE -eq 0) {
    Write-Host "== kiosk reloaded" -ForegroundColor Green
    exit 0
  }
  if ($i -lt $Retries) { Write-Host "   retry $i/$Retries..." -ForegroundColor DarkYellow; Start-Sleep 2 }
}
Write-Host "!! RELOAD FAILED - the Pi is still running the OLD code." -ForegroundColor Red
exit 1
